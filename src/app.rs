use crate::config::AppConfig;
use crate::history::History;
use dirs::config_dir;
use log::{debug, error, info, warn};
use freedesktop_desktop_entry::{Iter, default_paths, get_languages_from_env};
use ratatui::widgets::ListState;
use rustls::{ClientConfig, ClientConnection, Stream};
use rustls_graviola;
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

const REMOTE_LOADER_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/GitanElyon/awesome-qst/main/scripts/loader.sh";

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    AppSelection,
    FileSelection,
    ScriptResults,
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub exec_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptAction {
    CopyToClipboard,
    SetStatusMessage,
    ClearStatusMessage,
    SetSearchQuery,
    AppendToQuery,
    PrependToQuery,
    ReplaceLastToken,
    PopLastToken,
    PopLastChar,
    ClearQuery,
    RefreshResults,
    Execute,
    ExitApp,
    ResetPrompt,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptRowMeta {
    pub display: Option<String>,
    pub meta: Vec<String>,
    pub nonselectable: bool,
    pub permanent: bool,
    pub active: bool,
    pub center: bool,
    pub urgent: bool,
    pub fuzzy: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptMetaField {
    Name,
    Version,
    Author,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptStorageWriteAction {
    PFront,
    PBack,
    RmFront,
    RmBack,
    Purge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptStorageReadAction {
    All,
    FPeek,
    BPeek,
}

#[derive(Debug, Clone)]
pub struct ScriptItem {
    pub title: String,
    pub value: String,
    pub actions: Vec<ScriptAction>,
    pub meta: ScriptRowMeta,
}

#[derive(Debug, Clone)]
pub struct ScriptListing {
    pub id: String,
    pub file_id: String,
    pub trigger: Option<String>,
    pub metadata: Option<ScriptMetadata>,
}

#[derive(Debug, Clone)]
struct ScriptPlugin {
    id: String,
    file_id: String,
    path: PathBuf,
    trigger: Option<String>,
    interpreter: Option<&'static str>,
    metadata: Option<ScriptMetadata>,
}

pub struct App {
    pub search_query: String,
    pub search_cursor: usize,
    pub entries: Vec<AppEntry>,
    pub filtered_entries: Vec<AppEntry>,
    pub list_state: ListState,
    pub should_quit: bool,
    pub config: AppConfig,
    pub status_message: Option<String>,
    pub launch_args: Option<Vec<String>>,
    pub mode: AppMode,
    pub filtered_files: Vec<String>,
    pub history: History,
    pub script_title: Option<String>,
    pub script_meta: Option<ScriptMetadata>,
    pub script_items: Vec<ScriptItem>,
    pub qst_ascii: String,
    pub hide_entries_until_typing: bool,
    pub fuzzy_matching_enabled: bool,
    scripts: Vec<ScriptPlugin>,
}

impl App {
    const SCRIPT_TIMEOUT: Duration = Duration::from_secs(10);

    pub fn new(config: AppConfig, status_message: Option<String>) -> Self {
        Self::ensure_loader_script_installed();
        let (mut script_aliases, mut app_aliases) = Self::load_aliases();
        let history = History::load();
        let scripts = Self::load_scripts(&mut script_aliases);
        info!("Loaded {} scripts", scripts.len());
        
        let mut entries = scan_desktop_files(config.features.show_duplicates);
        info!("Loaded {} desktop entries", entries.len());
        
        if !config.features.show_duplicates {
            let alias_keys: Vec<String> = app_aliases.keys().map(|k| k.to_lowercase()).collect();
            entries.retain(|e| !alias_keys.contains(&e.name.to_lowercase()));
        }

        for (name, command) in app_aliases.drain() {
            entries.push(AppEntry {
                name,
                exec_args: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    format!(r#"{} "$@""#, command),
                    "--".to_string(),
                ],
            });
        }

        let qst_ascii = if let Some(path) = &config.qst_ascii.custom_path {
            let expanded_path = path.replace("~", std::env::var("HOME").unwrap_or_else(|_| String::new()).as_str());
            fs::read_to_string(expanded_path).unwrap_or_else(|_| include_str!("../assets/qst.txt").to_string())
        } else {
            include_str!("../assets/qst.txt").to_string()
        };

        let mut app = Self {
            search_query: String::new(),
            search_cursor: 0,
            filtered_entries: entries.clone(),
            entries,
            list_state: ListState::default().with_selected(Some(0)),
            should_quit: false,
            config,
            status_message,
            launch_args: None,
            mode: AppMode::AppSelection,
            filtered_files: Vec::new(),
            history,
            script_title: None,
            script_meta: None,
            script_items: Vec::new(),
            qst_ascii,
            hide_entries_until_typing: false,
            fuzzy_matching_enabled: true,
            scripts,
        };

        app.sort_entries();
        app.filtered_entries = app.entries.clone();
        app
    }

    pub fn script_listings(&self) -> Vec<ScriptListing> {
        self.scripts
            .iter()
            .map(|script| ScriptListing {
                id: script.id.clone(),
                file_id: script.file_id.clone(),
                trigger: script.trigger.clone(),
                metadata: script.metadata.clone(),
            })
            .collect()
    }

    pub fn launch_program_by_name(&mut self, program_name: &str) -> Result<(), String> {
        let program_name = program_name.trim();
        let query = program_name.to_lowercase();
        let Some(entry) = self
            .entries
            .iter()
            .filter_map(|entry| {
                Self::search_score(&query, &entry.name, self.fuzzy_matching_enabled)
                    .map(|score| (score, entry))
            })
            .max_by(|(score_a, entry_a), (score_b, entry_b)| {
                score_a
                    .cmp(score_b)
                    .then_with(|| entry_a.name.to_lowercase().cmp(&entry_b.name.to_lowercase()))
                    .then_with(|| entry_a.name.cmp(&entry_b.name))
            })
            .map(|(_, entry)| entry.clone())
        else {
            return Err(format!("No program found matching '{program_name}'"));
        };

        self.history.increment(&entry.name);

        let Some((cmd, args)) = entry.exec_args.split_first() else {
            return Err(format!("Program '{}' has no launch command", entry.name));
        };

        info!("Launching program: {} (cmd: {}, args: {:?})", entry.name, cmd, args);
        let launch_args = self.build_exec_args(args, None);
        self.spawn_command(cmd, launch_args, &entry.name)
    }

    pub fn launch_script_mode(&mut self, script_name: &str) -> Result<(), String> {
        let script_name = script_name.trim();
        let Some(script) = self.find_script(script_name).cloned() else {
            return Err(format!("No script found matching '{script_name}'"));
        };

        let query = script
            .trigger
            .clone()
            .or_else(|| Some(script.file_id.clone()))
            .unwrap_or_else(|| script.id.clone());

        self.set_search_query(query);
        self.update_filter();
        Ok(())
    }

    fn char_count(input: &str) -> usize {
        input.chars().count()
    }

    fn byte_index_at_char(input: &str, char_idx: usize) -> usize {
        if char_idx == 0 {
            return 0;
        }

        input
            .char_indices()
            .nth(char_idx)
            .map(|(idx, _)| idx)
            .unwrap_or_else(|| input.len())
    }

    pub fn move_search_cursor_left(&mut self) {
        if self.search_cursor > 0 {
            self.search_cursor -= 1;
        }
    }

    pub fn move_search_cursor_right(&mut self) {
        let len = Self::char_count(&self.search_query);
        if self.search_cursor < len {
            self.search_cursor += 1;
        }
    }

    pub fn insert_search_char(&mut self, ch: char) {
        let byte_idx = Self::byte_index_at_char(&self.search_query, self.search_cursor);
        self.search_query.insert(byte_idx, ch);
        self.search_cursor += 1;
        self.update_filter();
    }

    pub fn insert_search_text(&mut self, text: &str) {
        let byte_idx = Self::byte_index_at_char(&self.search_query, self.search_cursor);
        self.search_query.insert_str(byte_idx, text);
        self.search_cursor += Self::char_count(text);
        self.update_filter();
    }

    pub fn backspace_search_char(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let end = Self::byte_index_at_char(&self.search_query, self.search_cursor);
        let start = Self::byte_index_at_char(&self.search_query, self.search_cursor - 1);
        self.search_query.replace_range(start..end, "");
        self.search_cursor -= 1;
        self.update_filter();
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.search_cursor = Self::char_count(&self.search_query);
    }

    fn reset_search_prompt(&mut self) {
        if let Some(space_idx) = self.search_query.find(' ') {
            let new_query = format!("{} ", self.search_query[..space_idx].trim());
            self.set_search_query(new_query);
        }

        self.update_filter();
    }

    fn script_item_is_selectable(&self, index: usize) -> bool {
        self.script_items
            .get(index)
            .map(|item| !item.meta.nonselectable)
            .unwrap_or(false)
    }

    fn first_selectable_script_index(&self) -> Option<usize> {
        self.script_items
            .iter()
            .enumerate()
            .find_map(|(index, item)| if item.meta.nonselectable { None } else { Some(index) })
    }

    fn last_selectable_script_index(&self) -> Option<usize> {
        self.script_items
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, item)| if item.meta.nonselectable { None } else { Some(index) })
    }

    fn parse_meta_bool(value: &str) -> bool {
        !matches!(value.trim().to_ascii_lowercase().as_str(), "false" | "0" | "no" | "off")
    }

    fn parse_script_metadata_field(value: &str) -> Option<ScriptMetaField> {
        match value.trim().to_ascii_lowercase().as_str() {
            "name" => Some(ScriptMetaField::Name),
            "version" => Some(ScriptMetaField::Version),
            "author" => Some(ScriptMetaField::Author),
            "description" => Some(ScriptMetaField::Description),
            _ => None,
        }
    }

    fn parse_script_metadata(value: &str) -> Option<ScriptMetadata> {
        let mut parts = value.splitn(4, ',');
        let name = parts.next().unwrap_or_default().trim();
        let version = parts.next().unwrap_or_default().trim();
        let author = parts.next().unwrap_or_default().trim();
        let description = parts.next().unwrap_or_default().trim();

        if [name, version, author, description].iter().all(|part| part.is_empty()) {
            return None;
        }

        Some(ScriptMetadata {
            name: (!name.is_empty()).then(|| name.to_string()),
            version: (!version.is_empty()).then(|| version.to_string()),
            author: (!author.is_empty()).then(|| author.to_string()),
            description: (!description.is_empty()).then(|| description.to_string()),
        })
    }

    fn script_metadata_field<'a>(metadata: &'a ScriptMetadata, field: ScriptMetaField) -> Option<&'a str> {
        match field {
            ScriptMetaField::Name => metadata.name.as_deref(),
            ScriptMetaField::Version => metadata.version.as_deref(),
            ScriptMetaField::Author => metadata.author.as_deref(),
            ScriptMetaField::Description => metadata.description.as_deref(),
        }
    }

    fn parse_script_row_meta(text: &str) -> (String, ScriptRowMeta) {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.starts_with("/@meta") {
            return (trimmed.to_string(), ScriptRowMeta::default());
        }

        let Some(meta_index) = trimmed.find(" @meta:") else {
            return (trimmed.to_string(), ScriptRowMeta::default());
        };

        let visible = trimmed[..meta_index].trim_end().to_string();
        let suffix = &trimmed[meta_index + 1..];
        let meta = Self::parse_script_row_meta_suffix(suffix);
        (visible, meta)
    }

    fn parse_script_row_meta_suffix(suffix: &str) -> ScriptRowMeta {
        let mut meta = ScriptRowMeta::default();
        let cleaned = suffix.trim();
        let cleaned = cleaned.strip_prefix("@meta:").unwrap_or(cleaned);

        for raw_chunk in cleaned.split(" @meta:") {
            let chunk = raw_chunk.trim();
            if chunk.is_empty() {
                continue;
            }

            let (key, value) = match chunk.split_once('=') {
                Some((key, value)) => (key.trim(), value.trim()),
                None => (chunk, "true"),
            };

            match key {
                "display" => {
                    if !value.is_empty() {
                        meta.display = Some(value.to_string());
                    }
                }
                "meta" => {
                    meta.meta.extend(
                        value
                            .split(',')
                            .map(|entry| entry.trim())
                            .filter(|entry| !entry.is_empty())
                            .map(|entry| entry.to_string()),
                    );
                }
                "nonselectable" => meta.nonselectable = Self::parse_meta_bool(value),
                "permanent" => meta.permanent = Self::parse_meta_bool(value),
                "active" => meta.active = Self::parse_meta_bool(value),
                "center" => meta.center = Self::parse_meta_bool(value),
                "urgent" => meta.urgent = Self::parse_meta_bool(value),
                "fuzzy" => meta.fuzzy = Self::parse_meta_bool(value),
                _ => {}
            }
        }

        meta
    }

    fn apply_script_row_meta(target: &mut ScriptRowMeta, source: ScriptRowMeta) {
        if source.display.is_some() {
            target.display = source.display;
        }
        if !source.meta.is_empty() {
            target.meta.extend(source.meta);
        }
        target.nonselectable |= source.nonselectable;
        target.permanent |= source.permanent;
        target.active |= source.active;
        target.center |= source.center;
        target.urgent |= source.urgent;
        target.fuzzy |= source.fuzzy;
    }

    fn parse_script_row_text(text: &str) -> (String, ScriptRowMeta) {
        let (visible, meta) = Self::parse_script_row_meta(text);
        (visible, meta)
    }

    fn script_storage_root(script_id: &str) -> Option<PathBuf> {
        let mut dir = config_dir()?;
        dir.push("qst");
        dir.push("storage");
        dir.push(script_id);
        Some(dir)
    }

    fn script_storage_path(script_id: &str, file_name: &str) -> Option<PathBuf> {
        let file_name = file_name.trim();
        if file_name.is_empty() {
            return None;
        }

        let file_path = Path::new(file_name);
        if file_path.is_absolute()
            || file_path.components().any(|component| {
                matches!(component, Component::ParentDir | Component::Prefix(_) | Component::RootDir)
            })
        {
            return None;
        }

        let mut path = Self::script_storage_root(script_id)?;
        path.push(file_path);
        Some(path)
    }

    fn read_script_storage_lines(
        script_id: &str,
        file_name: &str,
        read_action: ScriptStorageReadAction,
    ) -> Vec<String> {
        let Some(path) = Self::script_storage_path(script_id, file_name) else {
            return Vec::new();
        };

        let Ok(content) = fs::read_to_string(&path) else {
            return Vec::new();
        };

        let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();

        match read_action {
            ScriptStorageReadAction::All => lines,
            ScriptStorageReadAction::FPeek => lines.into_iter().take(1).collect(),
            ScriptStorageReadAction::BPeek => lines.pop().into_iter().collect(),
        }
    }

    fn write_script_storage(
        script_id: &str,
        file_name: &str,
        action: ScriptStorageWriteAction,
        value: &str,
    ) {
        let Some(path) = Self::script_storage_path(script_id, file_name) else {
            return;
        };

        if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_err() {
                return;
            }
        }

        let mut lines: Vec<String> = fs::read_to_string(&path)
            .map(|content| content.lines().map(|line| line.to_string()).collect())
            .unwrap_or_default();

        match action {
            ScriptStorageWriteAction::PFront => lines.insert(0, value.to_string()),
            ScriptStorageWriteAction::PBack => lines.push(value.to_string()),
            ScriptStorageWriteAction::RmFront => {
                if !lines.is_empty() {
                    lines.remove(0);
                }
            }
            ScriptStorageWriteAction::RmBack => {
                lines.pop();
            }
            ScriptStorageWriteAction::Purge => {
                lines.retain(|line| line != value);
            }
        }

        let content = lines.join("\n");
        let _ = fs::write(&path, content);
    }

    fn delete_script_storage(script_id: &str, file_name: &str) {
        let Some(path) = Self::script_storage_path(script_id, file_name) else {
            return;
        };

        let _ = fs::remove_file(path);
    }

    fn script_item_search_text(item: &ScriptItem) -> String {
        let mut parts = vec![item.title.as_str(), item.value.as_str()];
        parts.extend(item.meta.meta.iter().map(String::as_str));
        parts.join(" ")
    }

    fn parse_script_actions(value: &str) -> Vec<ScriptAction> {
        let mut actions = Vec::new();

        for token in value.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }

            match token {
                "CopyToClipboard" => actions.push(ScriptAction::CopyToClipboard),
                "SetStatusMessage" => actions.push(ScriptAction::SetStatusMessage),
                "ClearStatusMessage" => actions.push(ScriptAction::ClearStatusMessage),
                "SetSearchQuery" => actions.push(ScriptAction::SetSearchQuery),
                "AppendToQuery" => actions.push(ScriptAction::AppendToQuery),
                "PrependToQuery" => actions.push(ScriptAction::PrependToQuery),
                "ReplaceLastToken" => actions.push(ScriptAction::ReplaceLastToken),
                "PopLastToken" => actions.push(ScriptAction::PopLastToken),
                "PopLastChar" => actions.push(ScriptAction::PopLastChar),
                "ClearQuery" => actions.push(ScriptAction::ClearQuery),
                "RefreshResults" => actions.push(ScriptAction::RefreshResults),
                "Execute" => actions.push(ScriptAction::Execute),
                "ExitApp" => actions.push(ScriptAction::ExitApp),
                "ResetPrompt" => actions.push(ScriptAction::ResetPrompt),
                "None" => actions.push(ScriptAction::None),
                _ => {}
            }
        }

        if actions.is_empty() {
            actions.push(ScriptAction::None);
        }

        actions
    }

    fn fuzzy_filter_script_items(
        items: Vec<ScriptItem>,
        query: &str,
        script_fuzzy: bool,
        fuzzy_matching_enabled: bool,
    ) -> Vec<ScriptItem> {
        if query.trim().is_empty() {
            return items;
        }

        let mut fuzzy_matches: Vec<(i64, usize, ScriptItem)> = Vec::new();
        let mut passthrough_items: Vec<(usize, ScriptItem)> = Vec::new();

        for (index, item) in items.into_iter().enumerate() {
            let fuzzy_enabled = fuzzy_matching_enabled && (script_fuzzy || item.meta.fuzzy);
            if fuzzy_enabled {
                if let Some(score) = Self::search_score(query, &Self::script_item_search_text(&item), true) {
                    fuzzy_matches.push((score, index, item));
                }
            } else {
                passthrough_items.push((index, item));
            }
        }

        fuzzy_matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        let mut filtered = Vec::with_capacity(fuzzy_matches.len() + passthrough_items.len());
        filtered.extend(fuzzy_matches.into_iter().map(|(_, _, item)| item));
        filtered.extend(passthrough_items.into_iter().map(|(_, item)| item));
        filtered
    }

    pub fn pop_last_query_token(&mut self) {
        let trimmed = self.search_query.trim_end();

        if trimmed.is_empty() {
            self.set_search_query(String::new());
            return;
        }

        if let Some(last_ws_idx) = trimmed.rfind(char::is_whitespace) {
            self.set_search_query(trimmed[..=last_ws_idx].to_string());
        } else {
            self.set_search_query(String::new());
        }
    }

    pub fn replace_last_query_token(&mut self, replacement: &str) {
        let trimmed = self.search_query.trim_end();

        if trimmed.is_empty() {
            self.set_search_query(replacement.to_string());
            return;
        }

        if let Some(last_ws_idx) = trimmed.rfind(char::is_whitespace) {
            self.set_search_query(format!("{}{}", &trimmed[..=last_ws_idx], replacement));
        } else {
            self.set_search_query(replacement.to_string());
        }
    }

    pub fn pop_last_query_char(&mut self) {
        let mut chars: Vec<char> = self.search_query.chars().collect();
        if chars.is_empty() {
            return;
        }
        chars.pop();
        self.set_search_query(chars.into_iter().collect());
    }

    pub fn sort_entries(&mut self) {
        let history = &self.history;
        let recent_first = self.config.features.recent_first;

        self.entries.sort_by(|a, b| {
            let fav_a = history.is_favorite(&a.name);
            let fav_b = history.is_favorite(&b.name);
            if fav_a != fav_b {
                return fav_b.cmp(&fav_a);
            }

            if recent_first {
                let count_a = history.get_count(&a.name);
                let count_b = history.get_count(&b.name);
                if count_a != count_b {
                    return count_b.cmp(&count_a);
                }
            }

            a.name.to_lowercase().cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    pub fn toggle_favorite(&mut self) {
        if self.mode == AppMode::AppSelection {
            if let Some(i) = self.list_state.selected() {
                if let Some(entry) = self.filtered_entries.get(i).cloned() {
                    let is_fav = self.history.is_favorite(&entry.name);
                    self.history.toggle_favorite(&entry.name);
                    debug!("Toggled favorite for {} (now: {})", entry.name, !is_fav);
                    self.sort_entries();
                    self.update_filter();
                }
            }
        }
    }


    pub fn update_filter(&mut self) {
        self.launch_args = None;
        self.mode = AppMode::AppSelection;
        self.filtered_files.clear();
        self.script_title = None;
        self.script_meta = None;
        self.script_items.clear();

        let query_slice_str = self.search_query.trim().to_string();
        let query_slice = query_slice_str.as_str();

        if self.try_run_script_query(query_slice) {
            let count = self.script_items.len();
            if count == 0 {
                self.list_state.select(None);
            } else {
                self.list_state.select(self.first_selectable_script_index());
            }
            return;
        }

        if self.config.features.enable_file_explorer && Self::looks_like_path_query(query_slice) {
            let files = self.list_completions(query_slice);
            self.filtered_entries.clear();
            self.filtered_files = files;
            self.mode = AppMode::FileSelection;
        }

        if self.mode != AppMode::FileSelection && query_slice.is_empty() {
            if self.hide_entries_until_typing {
                self.filtered_entries = Vec::new();
            } else {
                self.filtered_entries = self.entries.clone();
            }
        } else if self.mode != AppMode::FileSelection {
            let query = query_slice.to_lowercase();
            let mut matches: Vec<(i64, AppEntry)> = self
                .entries
                .iter()
                .filter_map(|e| {
                    Self::search_score(&query, &e.name, self.fuzzy_matching_enabled).map(|score| (score, e.clone()))
                })
                .collect();

            matches.sort_by(|a, b| b.0.cmp(&a.0));

            let matches: Vec<AppEntry> = matches.into_iter().map(|(_, e)| e).collect();

            if !matches.is_empty() {
                self.filtered_entries = matches;
            } else {
                let words: Vec<&str> = query_slice.split_whitespace().collect();
                let mut found = false;

                for i in (1..words.len()).rev() {
                    let sub_query = words[0..i].join(" ");
                    let sub_query_lower = sub_query.to_lowercase();

                    let mut sub_matches: Vec<(i64, AppEntry)> = self
                        .entries
                        .iter()
                        .filter_map(|e| {
                            Self::search_score(&sub_query_lower, &e.name, self.fuzzy_matching_enabled)
                                .map(|score| (score, e.clone()))
                        })
                        .collect();

                    sub_matches.sort_by(|a, b| b.0.cmp(&a.0));

                    let sub_matches: Vec<AppEntry> = sub_matches.into_iter().map(|(_, e)| e).collect();

                    if !sub_matches.is_empty() {
                        self.filtered_entries = sub_matches;
                        
                        if self.config.features.enable_launch_args {
                            let args: Vec<String> = words[i..].iter().map(|s| s.to_string()).collect();
                            if let Some(last_arg) = args.last() {
                                if !last_arg.starts_with('-') && Self::looks_like_path_query(last_arg) {
                                    let files = self.list_completions(last_arg);
                                    if !files.is_empty() && self.config.features.enable_file_explorer {
                                        self.filtered_files = files;
                                        self.mode = AppMode::FileSelection;
                                    }
                                }
                            }
                            self.launch_args = Some(args);
                        }
                        
                        found = true;
                        break;
                    }
                }

                if !found {
                    self.filtered_entries = Vec::new();
                }
            }
        }
        
        let count = match self.mode {
            AppMode::AppSelection => self.filtered_entries.len(),
            AppMode::FileSelection => self.filtered_files.len(),
            AppMode::ScriptResults => self.script_items.len(),
        };

        debug!("Filter updated: mode={:?}, count={}", self.mode, count);

        if count == 0 {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = match self.mode {
            AppMode::AppSelection => self.filtered_entries.len(),
            AppMode::FileSelection => self.filtered_files.len(),
            AppMode::ScriptResults => self.script_items.len(),
        };

        if len == 0 {
            return;
        }

        if self.mode == AppMode::ScriptResults {
            let start = match self.list_state.selected() {
                Some(i) => i,
                None => {
                    if delta >= 0 {
                        len - 1
                    } else {
                        0
                    }
                }
            };

            let mut current = start;
            for _ in 0..len {
                current = ((current as i32 + delta).rem_euclid(len as i32)) as usize;
                if self.script_item_is_selectable(current) {
                    self.list_state.select(Some(current));
                    return;
                }
            }

            self.list_state.select(None);
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                let new_i = (i as i32 + delta).rem_euclid(len as i32);
                new_i as usize
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn select_first(&mut self) {
        let len = match self.mode {
            AppMode::AppSelection => self.filtered_entries.len(),
            AppMode::FileSelection => self.filtered_files.len(),
            AppMode::ScriptResults => self.script_items.len(),
        };

        if len > 0 {
            if self.mode == AppMode::ScriptResults {
                self.list_state.select(self.first_selectable_script_index());
            } else {
                self.list_state.select(Some(0));
            }
        }
    }

    pub fn select_last(&mut self) {
        let len = match self.mode {
            AppMode::AppSelection => self.filtered_entries.len(),
            AppMode::FileSelection => self.filtered_files.len(),
            AppMode::ScriptResults => self.script_items.len(),
        };

        if len > 0 {
            if self.mode == AppMode::ScriptResults {
                self.list_state.select(self.last_selectable_script_index());
            } else {
                self.list_state.select(Some(len - 1));
            }
        }
    }

    pub fn auto_complete(&mut self) {
        if !self.config.features.enable_auto_complete {
            return;
        }
        if self.mode == AppMode::FileSelection {
            if let Some(i) = self.list_state.selected() {
                if let Some(selected_file) = self.filtered_files.get(i) {
                    let mut new_path = selected_file.clone();

                    let expanded_path = self.expand_path(&new_path);
                    if Path::new(&expanded_path).is_dir()
                        && !new_path.ends_with('/')
                        && !new_path.ends_with("/.")
                    {
                        new_path.push('/');
                    }

                    if let Some(last_space_idx) = self.search_query.rfind(' ') {
                        let (prefix, _) = self.search_query.split_at(last_space_idx + 1);
                        self.set_search_query(format!("{}{}", prefix, new_path));
                    } else {
                        self.set_search_query(new_path);
                    }
                    self.update_filter();
                }
            }
        }
    }

    pub fn launch_selected(&mut self) {
        if self.mode == AppMode::ScriptResults {
            if let Some(i) = self.list_state.selected() {
                if let Some(item) = self.script_items.get(i).cloned() {
                    if item.meta.nonselectable {
                        return;
                    }
                    self.apply_script_actions(&item);
                }
            }
            return;
        }

        if let Some(i) = self.list_state.selected() {
            if self.mode == AppMode::FileSelection && self.filtered_entries.is_empty() {
                if let Some(selected_file) = self.filtered_files.get(i).cloned() {
                    self.open_file(&selected_file);
                }
                return;
            }

            let app_entry = if self.mode == AppMode::FileSelection {
                self.filtered_entries.first().cloned()
            } else {
                self.filtered_entries.get(i).cloned()
            };

            if let Some(entry) = app_entry {
                self.history.increment(&entry.name);
                if let Some((cmd, args)) = entry.exec_args.split_first() {
                    let selected_file = if self.mode == AppMode::FileSelection {
                        self.filtered_files.get(i).map(String::as_str)
                    } else {
                        None
                    };
                    let final_args = self.build_exec_args(args, selected_file);
                    let _ = self.spawn_command(cmd, final_args, &entry.name);
                }
            }
        }
    }

    fn build_exec_args(&self, args: &[String], selected_file: Option<&str>) -> Vec<String> {
        let mut final_args = Vec::new();
        let launch_placeholders = ["%f", "%F", "%u", "%U"];

        if self.config.features.enable_launch_args {
            if let Some(launch_args) = &self.launch_args {
                let mut current_launch_args = launch_args.clone();

                if let Some(selected_file) = selected_file {
                    if let Some(last) = current_launch_args.last_mut() {
                        *last = selected_file.to_string();
                    }
                }

                let expanded_launch_args: Vec<String> = current_launch_args
                    .iter()
                    .map(|arg| self.expand_path(arg))
                    .collect();

                let mut replaced = false;
                for arg in args {
                    if launch_placeholders.contains(&arg.as_str()) {
                        final_args.extend(expanded_launch_args.clone());
                        replaced = true;
                    } else {
                        final_args.push(arg.clone());
                    }
                }

                if !replaced {
                    final_args.extend(expanded_launch_args);
                }

                return final_args;
            }
        }

        for arg in args {
            if !launch_placeholders.contains(&arg.as_str()) {
                final_args.push(arg.clone());
            }
        }

        final_args
    }

    fn spawn_command(&mut self, cmd: &str, args: Vec<String>, entry_name: &str) -> Result<(), String> {
        let mut command = Command::new(cmd);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(()) as io::Result<()>
            });
        }

        match command.spawn() {
            Ok(_) => {
                info!("Launched {}", entry_name);
                self.should_quit = true;
                self.status_message = None;
                Ok(())
            }
            Err(err) => {
                error!("Failed to launch {}: {}", entry_name, err);
                self.status_message =
                    Some(format!("Failed to launch {}: {}", entry_name, err));
                Err(err.to_string())
            }
        }
    }

    fn open_file(&mut self, path_str: &str) {
        let expanded = self.expand_path(path_str);
        let path = Path::new(&expanded);

        let is_executable = if let Ok(metadata) = fs::metadata(path) {
            metadata.permissions().mode() & 0o111 != 0
        } else {
            false
        };

        let mut command = if is_executable && !path.is_dir() {
            Command::new(path)
        } else {
            let mut cmd = Command::new("xdg-open");
            cmd.arg(path);
            cmd
        };

        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(()) as io::Result<()>
            });
        }

        match command.spawn() {
            Ok(_) => {
                info!("Opened {}", path_str);
                self.should_quit = true;
                self.status_message = None;
            }
            Err(err) => {
                error!("Failed to open {}: {}", path_str, err);
                self.status_message = Some(format!("Failed to open {}: {}", path_str, err));
            }
        }
    }

    fn looks_like_path_query(query: &str) -> bool {
        query.starts_with("/")
            || query.starts_with("~/")
            || query.starts_with("./")
            || query.starts_with("../")
    }

    fn expand_path(&self, path: &str) -> String {
        if path == "~" {
            return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
        }

        if let Some(rest) = path.strip_prefix("~/") {
            let home = std::env::var("HOME").unwrap_or_default();
            if home.is_empty() {
                return path.to_string();
            }
            return format!("{}/{}", home, rest);
        }

        path.to_string()
    }

    fn list_completions(&self, query_path: &str) -> Vec<String> {
        let expanded_input = self.expand_path(query_path);
        let input_path = Path::new(&expanded_input);
        let query_root = query_path
            .rsplit_once('/')
            .map(|(head, _)| format!("{}/", head))
            .unwrap_or_default();
        let is_directory_query = expanded_input.ends_with('/') || input_path.is_dir();
        let show_current_dir_entry = query_path.ends_with('/');

        let (dir_path, prefix, display_root) = if is_directory_query {
            let root = if query_path.ends_with('/') {
                query_path.to_string()
            } else {
                format!("{}/", query_path)
            };
            (input_path.to_path_buf(), String::new(), root)
        } else {
            (
                input_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf(),
                input_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
                query_root,
            )
        };

        let mut results: Vec<String> = match fs::read_dir(&dir_path) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    let name = entry.file_name();
                    let name = name.to_str()?.to_string();
                    if !prefix.is_empty() && !name.starts_with(&prefix) {
                        return None;
                    }

                    let mut relative = format!("{}{}", display_root, name);

                    if entry.path().is_dir() {
                        relative.push('/');
                    }
                    Some(relative)
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        if self.config.features.dirs_first {
            results.sort_by(|a, b| {
                let a_is_dir = a.ends_with('/');
                let b_is_dir = b.ends_with('/');
                b_is_dir.cmp(&a_is_dir).then_with(|| a.cmp(b))
            });
        } else {
            results.sort();
        }

        if show_current_dir_entry {
            results.insert(0, format!("{}.", display_root));
        }

        results
    }

    fn scripts_dir() -> Option<PathBuf> {
        let mut dir = config_dir()?;
        dir.push("qst");
        dir.push("scripts");
        Some(dir)
    }

    fn ensure_loader_script_installed() {
        let Some(dir) = Self::scripts_dir() else {
            return;
        };

        let loader_path = dir.join("loader.sh");
        if loader_path.exists() {
            Self::ensure_executable(&loader_path);
            return;
        }

        if fs::create_dir_all(&dir).is_err() {
            return;
        }

        let loader_script = Self::fetch_remote_loader_script().unwrap_or_else(|| {
            "#!/bin/sh\necho \"qst! meta \"\ncat \"$@\"\n".to_string()
        });

        if fs::write(&loader_path, loader_script).is_ok() {
            Self::ensure_executable(&loader_path);
        }
    }

    fn fetch_remote_loader_script() -> Option<String> {
        rustls_graviola::default_provider()
            .install_default()
            .ok()?;

        let url = REMOTE_LOADER_SCRIPT_URL.strip_prefix("https://")?;
        let (host, rest) = url.split_once('/')?;
        let path = format!("/{rest}");

        let addr = (host, 443).to_socket_addrs().ok()?.next()?;
        let mut tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).ok()?;
        tcp.set_read_timeout(Some(Duration::from_secs(5))).ok()?;

        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = Arc::new(
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        );

        let server_name = rustls::pki_types::ServerName::try_from(host).ok()?;
        let mut tls_conn = ClientConnection::new(config, server_name).ok()?;
        let mut tls_stream = Stream::new(&mut tls_conn, &mut tcp);

        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
        );
        tls_stream.write_all(request.as_bytes()).ok()?;
        tls_stream.flush().ok()?;

        let mut response = Vec::new();
        tls_stream.read_to_end(&mut response).ok()?;
        let response = String::from_utf8(response).ok()?;

        let status = response.lines().next()?;
        let code = status.split_whitespace().nth(1)?;
        if !code.starts_with('2') {
            return None;
        }

        let body = response.split("\r\n\r\n").nth(1)?;
        let body = body.trim().to_string();
        if body.is_empty() { None } else { Some(body) }
    }

    fn ensure_executable(path: &Path) {
        let Ok(metadata) = fs::metadata(path) else {
            return;
        };

        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 != 0o111 {
            permissions.set_mode(mode | 0o111);
            let _ = fs::set_permissions(path, permissions);
        }
    }

    fn script_interpreter_for_extension(ext: &str) -> Option<&'static str> {
        match ext {
            "sh" => Some("sh"),
            "bash" => Some("bash"),
            "zsh" => Some("zsh"),
            "fish" => Some("fish"),
            "py" => Some("python3"),
            "pl" => Some("perl"),
            "rb" => Some("ruby"),
            "js" => Some("node"),
            "lua" => Some("lua"),
            _ => None,
        }
    }

    fn normalize_alias_key(key: &str) -> String {
        let normalized = key.trim();
        if normalized.is_empty() {
            return String::new();
        }

        let Some((base, ext)) = normalized.rsplit_once('.') else {
            return normalized.to_string();
        };

        if Self::script_interpreter_for_extension(&ext.to_ascii_lowercase()).is_some() {
            return base.to_string();
        }

        normalized.to_string()
    }

    fn read_script_metadata_from_source(path: &Path) -> Option<ScriptMetadata> {
        let contents = fs::read_to_string(path).ok()?;

        contents.lines().find_map(|line| {
            let line = line.trim_start();
            let metadata_line = line.strip_prefix("echo \"qst! meta ")?;
            let metadata_line = metadata_line.strip_suffix('"').unwrap_or(metadata_line);
            Self::parse_script_metadata(metadata_line)
        })
    }

    fn load_scripts(aliases: &mut HashMap<String, String>) -> Vec<ScriptPlugin> {
        let mut scripts = Vec::new();
        
        let Some(dir) = Self::scripts_dir() else {
            return scripts;
        };

        let Ok(entries) = fs::read_dir(&dir) else {
            return scripts;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };

            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase());
            let interpreter = extension
                .as_deref()
                .and_then(Self::script_interpreter_for_extension);
            let is_executable = meta.permissions().mode() & 0o111 != 0;

            if !is_executable && interpreter.is_none() {
                continue;
            }

            let id_source = path
                .file_stem()
                .or_else(|| path.file_name())
                .and_then(|value| value.to_str());
            let Some(stem) = id_source else {
                continue;
            };
            let id = stem.to_string();
            let file_id = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(stem)
                .to_string();
            let trigger = aliases.remove(stem).or_else(|| aliases.remove(&file_id));
            let metadata = Self::read_script_metadata_from_source(&path);

            scripts.push(ScriptPlugin {
                id,
                file_id,
                path,
                trigger,
                interpreter,
                metadata,
            });
        }

        scripts.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.file_id.cmp(&b.file_id)));
        scripts
    }

    fn load_aliases() -> (HashMap<String, String>, HashMap<String, String>) {
        let mut script_aliases = HashMap::new();
        let mut app_aliases = HashMap::new();

        let Some(mut config_dir_path) = config_dir() else {
            return (script_aliases, app_aliases);
        };
        config_dir_path.push("qst");

        let alias_path = if config_dir_path.join("alias.toml").exists() {
            config_dir_path.join("alias.toml")
        } else if config_dir_path.join("Alias.toml").exists() {
            config_dir_path.join("Alias.toml")
        } else {
            return (script_aliases, app_aliases);
        };

        let Ok(contents) = fs::read_to_string(alias_path) else {
            return (script_aliases, app_aliases);
        };
        let Ok(value) = toml::from_str::<toml::Value>(&contents) else {
            return (script_aliases, app_aliases);
        };
        
        if let Some(table) = value.as_table() {
            let has_scripts = table.contains_key("scripts");
            let has_apps = table.contains_key("apps");

            if !has_scripts && !has_apps {
                // Backwards compatibility: treat everything as script aliases
                Self::collect_aliases_from_table(table, "", &mut script_aliases);
            } else {
                if let Some(scripts_table) = table.get("scripts").and_then(|v| v.as_table()) {
                    Self::collect_aliases_from_table(scripts_table, "", &mut script_aliases);
                }
                if let Some(apps_table) = table.get("apps").and_then(|v| v.as_table()) {
                    Self::collect_aliases_from_table(apps_table, "", &mut app_aliases);
                }
            }
        }

        (script_aliases, app_aliases)
    }

    fn collect_aliases_from_table(
        table: &toml::map::Map<String, toml::Value>,
        prefix: &str,
        aliases: &mut HashMap<String, String>,
    ) {
        for (key, value) in table {
            let full_key = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", prefix, key)
            };

            match value {
                toml::Value::String(trigger) => {
                    let normalized = Self::normalize_alias_key(&full_key);
                    if !normalized.is_empty() {
                        aliases.insert(normalized, trigger.trim().to_string());
                    }
                }
                toml::Value::Table(child) => {
                    Self::collect_aliases_from_table(child, &full_key, aliases);
                }
                _ => {}
            }
        }
    }

    fn try_run_script_query(&mut self, query: &str) -> bool {
        if query.is_empty() || self.scripts.is_empty() {
            return false;
        }

        let mut matched: Option<(ScriptPlugin, String)> = None;

        let mut aliases: Vec<&ScriptPlugin> = self
            .scripts
            .iter()
            .filter(|script| script.trigger.as_ref().is_some_and(|t| !t.is_empty()))
            .collect();
        aliases.sort_by(|a, b| {
            b.trigger
                .as_ref()
                .map(|t| t.len())
                .unwrap_or(0)
                .cmp(&a.trigger.as_ref().map(|t| t.len()).unwrap_or(0))
        });

        for script in aliases {
            let trigger = script.trigger.as_ref().expect("filtered non-empty trigger");
            if let Some(rest) = query.strip_prefix(trigger) {
                matched = Some((script.clone(), rest.trim_start().to_string()));
                break;
            }
        }

        if matched.is_none() {
            for script in &self.scripts {
                if query == script.file_id {
                    matched = Some((script.clone(), String::new()));
                    break;
                }

                if let Some(rest) = query.strip_prefix(&format!("{} ", script.file_id)) {
                    matched = Some((script.clone(), rest.trim_start().to_string()));
                    break;
                }
            }
        }

        if matched.is_none() {
            let mut stem_counts: HashMap<&str, usize> = HashMap::new();
            for script in &self.scripts {
                *stem_counts.entry(script.id.as_str()).or_insert(0) += 1;
            }

            for script in &self.scripts {
                if query == script.id {
                    if stem_counts.get(script.id.as_str()).copied().unwrap_or(0) > 1 {
                        continue;
                    }
                    matched = Some((script.clone(), String::new()));
                    break;
                }

                if let Some(rest) = query.strip_prefix(&format!("{} ", script.id)) {
                    if stem_counts.get(script.id.as_str()).copied().unwrap_or(0) > 1 {
                        continue;
                    }
                    matched = Some((script.clone(), rest.trim_start().to_string()));
                    break;
                }
            }
        }

        let Some((script, payload)) = matched else {
            return false;
        };

        self.filtered_entries.clear();
        self.filtered_files.clear();
        self.mode = AppMode::ScriptResults;

        info!("Running script: {} (payload: {})", script.id, payload);
        match self.run_script(&script, &payload) {
            Ok((title, message, meta, items)) => {
                debug!("Script {} returned {} items", script.id, items.len());
                self.script_meta = meta.clone();
                self.script_title = title
                    .or_else(|| meta.and_then(|meta| meta.name).map(|name| format!(" {} ", name)))
                    .or_else(|| Some(format!(" {} ", script.id)));
                self.script_items = items;
                self.status_message = message;
            }
            Err(err) => {
                error!("Script {} failed: {}", script.id, err);
                self.script_meta = None;
                self.script_title = Some(format!(" {} ", script.id));
                self.script_items = vec![ScriptItem {
                    title: format!("Script error: {}", err),
                    value: String::new(),
                    actions: vec![ScriptAction::None],
                    meta: ScriptRowMeta::default(),
                }];
            }
        }

        true
    }

    fn find_script(&self, selector: &str) -> Option<&ScriptPlugin> {
        let selector = selector.trim();
        if selector.is_empty() {
            return None;
        }

        self.scripts.iter().find(|script| {
            script.id.eq_ignore_ascii_case(selector)
                || script.file_id.eq_ignore_ascii_case(selector)
                || script
                    .trigger
                    .as_deref()
                    .is_some_and(|trigger| trigger.eq_ignore_ascii_case(selector))
                || script
                    .metadata
                    .as_ref()
                    .and_then(|meta| meta.name.as_deref())
                    .is_some_and(|name| name.eq_ignore_ascii_case(selector))
        })
    }

            fn search_score(query: &str, target: &str, fuzzy_matching_enabled: bool) -> Option<i64> {
                let query = query.trim();
                if query.is_empty() {
                    return Some(0);
                }

                if fuzzy_matching_enabled {
                    return fuzzy_score(query, target);
                }

                let query_lower = query.to_lowercase();
                let target_lower = target.to_lowercase();
                if target_lower.contains(&query_lower) {
                    Some((query_lower.len() as i64).saturating_mul(10) - target_lower.len() as i64)
                } else {
                    None
                }
            }

    fn run_script(&self, script: &ScriptPlugin, payload: &str) -> Result<(Option<String>, Option<String>, Option<ScriptMetadata>, Vec<ScriptItem>), String> {
        self.run_script_with_timeout(script, payload, Self::SCRIPT_TIMEOUT)
    }

    fn run_script_with_timeout(
        &self,
        script: &ScriptPlugin,
        payload: &str,
        timeout: Duration,
    ) -> Result<(Option<String>, Option<String>, Option<ScriptMetadata>, Vec<ScriptItem>), String> {
        let mut command = if let Some(interpreter) = script.interpreter {
            let mut command = Command::new(interpreter);
            command.arg(&script.path);
            command
        } else {
            Command::new(&script.path)
        };

        let mut child = command
            .arg(payload)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| err.to_string())?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture script stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "failed to capture script stderr".to_string())?;

        let stdout_handle = thread::spawn(move || Self::read_pipe(stdout));
        let stderr_handle = thread::spawn(move || Self::read_pipe(stderr));

        let start = Instant::now();
        let status = loop {
            match child.try_wait().map_err(|err| err.to_string())? {
                Some(status) => break status,
                None => {
                    if start.elapsed() >= timeout {
                        warn!("Script {} timed out after {:?}", script.id, timeout);
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = stdout_handle.join();
                        let _ = stderr_handle.join();
                        return Err(format!("script timed out after {:?}", timeout));
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            }
        };

        let (stdout, stderr) = Self::collect_script_output(stdout_handle, stderr_handle)?;

        if !status.success() {
            let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
            if stderr.is_empty() {
                warn!("Script {} exited with code {:?}", script.id, status.code());
                return Err(format!("exit code {:?}", status.code()));
            }
            warn!("Script {} stderr: {}", script.id, stderr);
            return Err(stderr);
        }

        let stdout = String::from_utf8_lossy(&stdout);
        Ok(Self::parse_script_output(&stdout, payload, &script.id, self.fuzzy_matching_enabled))
    }

    fn read_pipe<R>(mut pipe: R) -> Vec<u8>
    where
        R: Read + Send + 'static,
    {
        let mut output = Vec::new();
        let _ = pipe.read_to_end(&mut output);
        output
    }

    fn collect_script_output(
        stdout_handle: thread::JoinHandle<Vec<u8>>,
        stderr_handle: thread::JoinHandle<Vec<u8>>,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        let stdout = stdout_handle
            .join()
            .map_err(|_| "failed to read script stdout".to_string())?;
        let stderr = stderr_handle
            .join()
            .map_err(|_| "failed to read script stderr".to_string())?;

        Ok((stdout, stderr))
    }

    fn parse_script_output(
        output: &str,
        query: &str,
        script_id: &str,
        fuzzy_matching_enabled: bool,
    ) -> (Option<String>, Option<String>, Option<ScriptMetadata>, Vec<ScriptItem>) {
        let mut title: Option<String> = None;
        let mut message: Option<String> = None;
        let mut meta: Option<ScriptMetadata> = None;
        let mut items = Vec::new();
        let mut default_actions = vec![ScriptAction::None];
        let mut next_item_actions: Option<Vec<ScriptAction>> = None;
        let mut script_fuzzy = false;

        for raw in output.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(directive) = line.strip_prefix("qst! ") {
                if let Some(value) = directive.strip_prefix("meta ") {
                    let value = value.trim();
                    if let Some(field) = Self::parse_script_metadata_field(value) {
                        if let Some(current_meta) = meta.as_ref() {
                            if let Some(field_value) = Self::script_metadata_field(current_meta, field)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                            {
                                message = Some(field_value.to_string());
                            }
                        }
                        continue;
                    }

                    if let Some(parsed_meta) = Self::parse_script_metadata(value) {
                        if title.is_none() {
                            if let Some(name) = parsed_meta.name.as_deref().filter(|value| !value.is_empty()) {
                                title = Some(format!(" {} ", name));
                            }
                        }
                        meta = Some(parsed_meta);
                        continue;
                    }
                }
                if let Some(value) = directive.strip_prefix("title ") {
                    let (visible, meta) = Self::parse_script_row_text(value.trim());
                    if meta.fuzzy {
                        script_fuzzy = true;
                    }
                    title = Some(format!(" {} ", visible));
                    continue;
                }
                if let Some(value) = directive.strip_prefix("message ") {
                    let (visible, meta) = Self::parse_script_row_text(value.trim());
                    if meta.fuzzy {
                        script_fuzzy = true;
                    }
                    message = Some(visible);
                    continue;
                }
                if directive == "clear_message" {
                    message = None;
                    continue;
                }
                if let Some(value) = directive.strip_prefix("action ") {
                    default_actions = Self::parse_script_actions(value.trim());
                    continue;
                }
                if let Some(value) = directive.strip_prefix("default_item_action ") {
                    default_actions = Self::parse_script_actions(value.trim());
                    continue;
                }
                if let Some(value) = directive.strip_prefix("item_action ") {
                    next_item_actions = Some(Self::parse_script_actions(value.trim()));
                    continue;
                }
                if directive == "clear" {
                    items.clear();
                    continue;
                }
                if let Some(value) = directive.strip_prefix("write ") {
                    let mut parts = value.splitn(3, '|');
                    let file_name = parts.next().unwrap_or_default().trim();
                    let action = parts
                        .next()
                        .map(|action| action.trim())
                        .and_then(Self::parse_script_storage_write_action);
                    let stored_value = parts.next().unwrap_or_default().trim();
                    if let Some(action) = action {
                        Self::write_script_storage(script_id, file_name, action, stored_value);
                    }
                    continue;
                }
                if let Some(value) = directive.strip_prefix("read ") {
                    let mut parts = value.splitn(2, '|');
                    let file_name = parts.next().unwrap_or_default().trim();
                    let read_action = parts
                        .next()
                        .map(|action| action.trim())
                        .and_then(Self::parse_script_storage_read_action)
                        .unwrap_or(ScriptStorageReadAction::All);
                    let read_items = Self::read_script_storage_lines(script_id, file_name, read_action);
                    Self::append_storage_rows(
                        &mut items,
                        read_items,
                        &mut next_item_actions,
                        &default_actions,
                    );
                    continue;
                }
                if let Some(value) = directive.strip_prefix("delete ") {
                    Self::delete_script_storage(script_id, value.trim());
                    continue;
                }
                if let Some(value) = directive.strip_prefix("single ") {
                    let mut parts = value.splitn(2, '|');
                    let query = parts.next().unwrap_or_default().trim();
                    let (result_text, result_meta) = Self::parse_script_row_text(parts.next().unwrap_or_default().trim());
                    let label = if query.is_empty() {
                        result_text.clone()
                    } else {
                        format!("{} = {}", query, result_text)
                    };
                    let actions = next_item_actions
                        .take()
                        .unwrap_or_else(|| default_actions.clone());
                    items.clear();
                    items.push(ScriptItem {
                        title: label,
                        value: result_text,
                        actions,
                        meta: result_meta,
                    });
                    continue;
                }
                if let Some(value) = directive.strip_prefix("item ") {
                    let mut parts = value.splitn(3, '|');
                    let (item_title, title_meta) = Self::parse_script_row_text(parts.next().unwrap_or_default().trim());
                    let value_part = parts.next().unwrap_or(item_title.as_str()).trim();
                    let (item_value, value_meta) = Self::parse_script_row_text(value_part);
                    let (explicit_action, action_meta) = parts
                        .next()
                        .and_then(|s| {
                            let (action_text, meta) = Self::parse_script_row_text(s.trim());
                            let action_text = action_text.trim();
                            if action_text.is_empty() {
                                None
                            } else {
                                Some((Self::parse_script_actions(action_text), meta))
                            }
                        })
                        .unwrap_or((Vec::new(), ScriptRowMeta::default()));
                    if !item_title.is_empty() {
                        let mut meta = ScriptRowMeta::default();
                        Self::apply_script_row_meta(&mut meta, title_meta);
                        Self::apply_script_row_meta(&mut meta, value_meta);
                        Self::apply_script_row_meta(&mut meta, action_meta);
                        let actions = if explicit_action.is_empty() {
                            next_item_actions
                                .take()
                                .unwrap_or_else(|| default_actions.clone())
                        } else {
                            explicit_action
                        };
                        items.push(ScriptItem {
                            title: item_title,
                            value: item_value,
                            actions,
                            meta,
                        });
                    }
                    continue;
                }
                if let Some(value) = directive.strip_prefix("log ") {
                    crate::logger::plugin_log(script_id, value.trim());
                    continue;
                }
                continue;
            }

            let mut parts = line.splitn(2, '|');
            let (item_title, title_meta) = Self::parse_script_row_text(parts.next().unwrap_or_default().trim());
            if item_title.is_empty() {
                continue;
            }
            let raw_value = parts.next().unwrap_or(item_title.as_str()).trim();
            let (item_value, value_meta) = Self::parse_script_row_text(raw_value);
            let mut meta = ScriptRowMeta::default();
            Self::apply_script_row_meta(&mut meta, title_meta);
            Self::apply_script_row_meta(&mut meta, value_meta);
            let actions = next_item_actions
                .take()
                .unwrap_or_else(|| default_actions.clone());
            items.push(ScriptItem {
                title: item_title,
                value: item_value,
                actions,
                meta,
            });
        }

        let items = Self::fuzzy_filter_script_items(items, query, script_fuzzy, fuzzy_matching_enabled);
        let mut items = items;
        items.sort_by(|a, b| b.meta.urgent.cmp(&a.meta.urgent));

        (title, message, meta, items)
    }

    fn append_storage_rows(
        items: &mut Vec<ScriptItem>,
        lines: Vec<String>,
        next_item_actions: &mut Option<Vec<ScriptAction>>,
        default_actions: &[ScriptAction],
    ) {
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let actions = next_item_actions
                .take()
                .unwrap_or_else(|| default_actions.to_vec());
            items.push(ScriptItem {
                title: line.clone(),
                value: line,
                actions,
                meta: ScriptRowMeta::default(),
            });
        }
    }

    fn parse_script_storage_write_action(value: &str) -> Option<ScriptStorageWriteAction> {
        match value {
            "pfront" => Some(ScriptStorageWriteAction::PFront),
            "pback" => Some(ScriptStorageWriteAction::PBack),
            "rmfront" => Some(ScriptStorageWriteAction::RmFront),
            "rmback" => Some(ScriptStorageWriteAction::RmBack),
            "purge" => Some(ScriptStorageWriteAction::Purge),
            _ => None,
        }
    }

    fn parse_script_storage_read_action(value: &str) -> Option<ScriptStorageReadAction> {
        match value {
            "all" => Some(ScriptStorageReadAction::All),
            "fpeek" => Some(ScriptStorageReadAction::FPeek),
            "bpeek" => Some(ScriptStorageReadAction::BPeek),
            _ => None,
        }
    }

    fn spawn_shell_command(command_text: &str) -> Result<std::process::Child, String> {
        let mut command = Command::new("sh");
        command
            .arg("-lc")
            .arg(command_text)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(()) as io::Result<()>
            });
        }

        command.spawn().map_err(|err| err.to_string())
    }

    fn apply_script_actions(&mut self, item: &ScriptItem) {
        let mut pending_execute: Option<std::process::Child> = None;

        for action in &item.actions {
            let should_continue = match action {
                ScriptAction::None => true,
                ScriptAction::SetSearchQuery => {
                    self.set_search_query(item.value.clone());
                    self.update_filter();
                    true
                }
                ScriptAction::AppendToQuery => {
                    self.insert_search_text(&item.value);
                    true
                }
                ScriptAction::PrependToQuery => {
                    self.set_search_query(format!("{}{}", item.value, self.search_query));
                    self.update_filter();
                    true
                }
                ScriptAction::ReplaceLastToken => {
                    self.replace_last_query_token(&item.value);
                    self.update_filter();
                    true
                }
                ScriptAction::PopLastToken => {
                    self.pop_last_query_token();
                    self.update_filter();
                    true
                }
                ScriptAction::PopLastChar => {
                    self.pop_last_query_char();
                    self.update_filter();
                    true
                }
                ScriptAction::ClearQuery => {
                    self.set_search_query(String::new());
                    self.update_filter();
                    true
                }
                ScriptAction::RefreshResults => {
                    if let Some(mut child) = pending_execute.take() {
                        let _ = child.wait();
                    }
                    self.update_filter();
                    true
                }
                ScriptAction::CopyToClipboard => match self.copy_to_clipboard(&item.value) {
                    Ok(()) => {
                        debug!("Copied value to clipboard");
                        self.status_message = Some("Copied to clipboard".to_string());
                        true
                    }
                    Err(err) => {
                        error!("Clipboard failed: {}", err);
                        self.status_message = Some(format!("Clipboard failed: {}", err));
                        false
                    }
                },
                ScriptAction::SetStatusMessage => {
                    self.status_message = Some(item.value.clone());
                    true
                }
                ScriptAction::ClearStatusMessage => {
                    self.status_message = None;
                    true
                }
                ScriptAction::Execute => match Self::spawn_shell_command(&item.value) {
                    Ok(child) => {
                        info!("Executing shell command: {}", item.value);
                        pending_execute = Some(child);
                        self.status_message = None;
                        true
                    }
                    Err(err) => {
                        error!("Failed to execute command ({}): {}", item.value, err);
                        self.status_message = Some(format!("Failed to execute command: {}", err));
                        false
                    }
                },
                ScriptAction::ExitApp => {
                    self.should_quit = true;
                    self.status_message = None;
                    true
                }
                ScriptAction::ResetPrompt => {
                    if let Some(mut child) = pending_execute.take() {
                        let _ = child.wait();
                    }

                    self.reset_search_prompt();
                    true
                }
            };

            if !should_continue {
                break;
            }
        }
    }

    fn copy_to_clipboard(&self, value: &str) -> Result<(), String> {
        let clipboard_command = self
            .config
            .general
            .clipboard_command
            .clone()
            .unwrap_or_else(|| "wl-copy".to_string());

        let mut command = Command::new("sh");
        command.arg("-lc").arg(format!("{}", clipboard_command));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
                Ok(()) as io::Result<()>
            });
        }

        let mut child = command.spawn().map_err(|err| err.to_string())?;
        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            stdin
                .write_all(value.as_bytes())
                .map_err(|err| err.to_string())?;
        }

        drop(child);
        Ok(())
    }
}

fn scan_desktop_files(show_duplicates: bool) -> Vec<AppEntry> {
    let locales = get_languages_from_env();
    let locale_slice = locales.as_slice();

    let mut entries: Vec<AppEntry> = Iter::new(default_paths())
        .entries(Some(locale_slice))
        .filter(|entry| !entry.no_display() && !entry.hidden())
        .filter_map(|entry| {
            let exec_args = entry.parse_exec().ok()?;
            let name = entry
                .full_name(locale_slice)
                .or_else(|| entry.name(locale_slice))
                .map(|cow| cow.into_owned())
                .unwrap_or_else(|| entry.appid.clone());

            Some(AppEntry { name, exec_args })
        })
        .collect();

    entries.sort_by(|a, b| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    
    if !show_duplicates {
        entries.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());
    }
    
    entries
}


pub(crate) fn fuzzy_score(query: &str, target: &str) -> Option<i64> {
    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    if query_chars.is_empty() {
        return Some(0);
    }

    let mut score = 0;
    let mut pattern_idx = 0;
    let mut prev_match_idx = -100;

    for (idx, &t_char) in target_chars.iter().enumerate() {
        if pattern_idx < query_chars.len() {
            let q_char = query_chars[pattern_idx];
            if t_char.eq_ignore_ascii_case(&q_char) {
                let mut char_score = 10;

                if idx as i64 == prev_match_idx + 1 {
                    char_score += 40;
                }

                if idx == 0
                    || target_chars[idx - 1].is_whitespace()
                    || ['_', '-', '.', '/'].contains(&target_chars[idx - 1])
                {
                    char_score += 20;
                }

                if t_char.is_uppercase() {
                    char_score += 10;
                }

                score += char_score;
                prev_match_idx = idx as i64;
                pattern_idx += 1;
            }
        }
    }

    if pattern_idx == query_chars.len() {
        score -= target_chars.len() as i64 - query_chars.len() as i64;
        return Some(score);
    }
    None
}

#[allow(dead_code)]
fn fuzzy_match(query: &str, target: &str) -> bool {
    fuzzy_score(query, target).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};

    struct TempDirCleanup(PathBuf);

    impl Drop for TempDirCleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_app() -> App {
        App {
            search_query: String::new(),
            search_cursor: 0,
            entries: Vec::new(),
            filtered_entries: Vec::new(),
            list_state: ListState::default(),
            should_quit: false,
            config: AppConfig::default(),
            status_message: None,
            launch_args: None,
            mode: AppMode::AppSelection,
            filtered_files: Vec::new(),
            history: History::default(),
            script_title: None,
            script_meta: None,
            script_items: Vec::new(),
            qst_ascii: String::new(),
            hide_entries_until_typing: false,
            fuzzy_matching_enabled: true,
            scripts: Vec::new(),
        }
    }

    fn unique_temp_path() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("qst-file-completions-{}-{}", std::process::id(), suffix))
    }

    #[test]
    fn parse_script_row_text_strips_fuzzy_and_center_meta() {
        let (visible, meta) = App::parse_script_row_text(
            "Clipboard entry @meta:fuzzy=true @meta:center=true",
        );

        assert_eq!(visible, "Clipboard entry");
        assert!(meta.fuzzy);
        assert!(meta.center);
    }

    #[test]
    fn apply_script_row_meta_merges_center_flag() {
        let mut target = ScriptRowMeta::default();

        App::apply_script_row_meta(
            &mut target,
            ScriptRowMeta {
                center: true,
                ..ScriptRowMeta::default()
            },
        );

        assert!(target.center);
    }

    #[test]
    fn fuzzy_filter_keeps_non_fuzzy_rows_and_matches_fuzzy_rows() {
        let fuzzy_item = ScriptItem {
            title: "Clipboard history".to_string(),
            value: "Clipboard history".to_string(),
            actions: vec![ScriptAction::None],
            meta: ScriptRowMeta {
                fuzzy: true,
                ..ScriptRowMeta::default()
            },
        };
        let plain_item = ScriptItem {
            title: "Terminal".to_string(),
            value: "Terminal".to_string(),
            actions: vec![ScriptAction::None],
            meta: ScriptRowMeta::default(),
        };

        let filtered = App::fuzzy_filter_script_items(vec![plain_item.clone(), fuzzy_item.clone()], "clb", false, true);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].title, fuzzy_item.title);
        assert_eq!(filtered[1].title, plain_item.title);
    }

    #[test]
    fn parse_script_output_ignores_log_directive_and_continues_parsing() {
        let output = "qst! title Test\nqst! log some debug info\nHello|world\n";
        let (_title, _message, _meta, items) = App::parse_script_output(output, "", "test_logger", true);
        assert_eq!(items.len(), 1, "log directive should not suppress item parsing");
        assert_eq!(items[0].title, "Hello");
        assert_eq!(items[0].value, "world");
    }

    #[test]
    fn parse_script_actions_supports_comma_separated_composition() {
        assert_eq!(
            App::parse_script_actions("CopyToClipboard,ExitApp"),
            vec![ScriptAction::CopyToClipboard, ScriptAction::ExitApp]
        );

        assert_eq!(
            App::parse_script_actions("Execute,RefreshResults"),
            vec![ScriptAction::Execute, ScriptAction::RefreshResults]
        );

        assert_eq!(
            App::parse_script_actions("Execute,ResetPrompt"),
            vec![ScriptAction::Execute, ScriptAction::ResetPrompt]
        );
    }

    #[test]
    fn parse_script_output_parses_script_metadata_header() {
        let output = "qst! meta My Awesome script, 1.0.0, John Doe, This script does awesome things!\nqst! title My Awesome script\n";

        let (title, message, meta, items) = App::parse_script_output(output, "", "sample", true);

        assert_eq!(title.as_deref(), Some(" My Awesome script "));
        assert!(message.is_none());
        assert!(items.is_empty());

        let meta = meta.expect("metadata should be captured");
        assert_eq!(meta.name.as_deref(), Some("My Awesome script"));
        assert_eq!(meta.version.as_deref(), Some("1.0.0"));
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
        assert_eq!(meta.description.as_deref(), Some("This script does awesome things!"));
    }

    #[test]
    fn parse_script_output_supports_script_metadata_field_selection() {
        let output = "qst! meta My Awesome script, 1.0.0, John Doe, This script does awesome things!\nqst! meta author\n";

        let (title, message, meta, items) = App::parse_script_output(output, "", "sample", true);

        assert_eq!(title.as_deref(), Some(" My Awesome script "));
        assert_eq!(message.as_deref(), Some("John Doe"));
        assert!(items.is_empty());

        let meta = meta.expect("metadata should be captured");
        assert_eq!(meta.name.as_deref(), Some("My Awesome script"));
        assert_eq!(meta.version.as_deref(), Some("1.0.0"));
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
        assert_eq!(meta.description.as_deref(), Some("This script does awesome things!"));
    }

    #[test]
    fn list_completions_prepends_current_directory_entry_at_trailing_slash() {
        let root = unique_temp_path();
        let _cleanup = TempDirCleanup(root.clone());

        let projects = root.join("Projects");
        let nested_dir = projects.join("docs");
        let nested_file = projects.join("alpha.txt");

        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(&nested_file, "alpha").unwrap();

        let app = test_app();
        let query = format!("{}/", projects.display());
        let dot_entry = format!("{}.", query);
        let completions = app.list_completions(&query);

        assert_eq!(completions.first().map(String::as_str), Some(dot_entry.as_str()));
        assert!(completions.iter().any(|entry| entry == &format!("{}docs/", query)));
        assert!(completions.iter().any(|entry| entry == &format!("{}alpha.txt", query)));

        let filtered = app.list_completions(&format!("{}a", query));
        assert_eq!(filtered, vec![format!("{}alpha.txt", query)]);
    }

    #[test]
    fn run_script_times_out_for_hanging_scripts() {
        let root = unique_temp_path();
        let _cleanup = TempDirCleanup(root.clone());

        fs::create_dir_all(&root).unwrap();
        let script_path = root.join("hang.sh");
        fs::write(&script_path, "#!/bin/sh\nsleep 1\n").unwrap();
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

        let app = test_app();
        let script = ScriptPlugin {
            id: "hang".to_string(),
            file_id: "hang.sh".to_string(),
            path: script_path,
            trigger: None,
            interpreter: None,
            metadata: None,
        };

        let err = app
            .run_script_with_timeout(&script, "", Duration::from_millis(50))
            .expect_err("script should time out");

        assert!(err.contains("timed out"));
    }

    #[test]
    fn script_storage_path_rejects_absolute_and_parent_paths() {
        assert!(App::script_storage_path("sample", "/tmp/data").is_none());
        assert!(App::script_storage_path("sample", "../data").is_none());
        assert!(App::script_storage_path("sample", "nested/data").is_some());
    }

    #[test]
    fn read_script_metadata_from_source_parses_header() {
        let root = unique_temp_path();
        let _cleanup = TempDirCleanup(root.clone());

        fs::create_dir_all(&root).unwrap();
        let script_path = root.join("meta.sh");
        fs::write(
            &script_path,
            "#!/bin/sh\necho \"qst! meta Sample Name, 1.2.3, Tester, Describes the script\"\n",
        )
        .unwrap();

        let metadata = App::read_script_metadata_from_source(&script_path).expect("metadata should be parsed");

        assert_eq!(metadata.name.as_deref(), Some("Sample Name"));
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.author.as_deref(), Some("Tester"));
        assert_eq!(metadata.description.as_deref(), Some("Describes the script"));
    }

    #[test]
    fn ensure_executable_adds_execute_bits_without_rewriting_content() {
        let root = unique_temp_path();
        let _cleanup = TempDirCleanup(root.clone());

        fs::create_dir_all(&root).unwrap();
        let script_path = root.join("loader.sh");
        fs::write(&script_path, "#!/bin/sh\necho loader\n").unwrap();

        App::ensure_executable(&script_path);

        let content = fs::read_to_string(&script_path).unwrap();
        let mode = fs::metadata(&script_path).unwrap().permissions().mode();

        assert_eq!(content, "#!/bin/sh\necho loader\n");
        assert_eq!(mode & 0o111, 0o111);
    }
}

