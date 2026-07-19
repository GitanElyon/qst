mod app;
mod config;
mod history;
mod logger;
mod ui;

use crate::{app::App, config::AppConfig, ui::draw};
use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use dirs::config_dir;
use crate::history::History;
use std::fs;
use std::io;
use std::env;
use std::path::{Path, PathBuf};
use log::{debug, info, warn};

enum CliAction {
    Interactive,
    GenerateConfig,
    Help,
    Version,
    ListPrograms,
    ListScripts,
    ClearHistory,
    ClearFavorites,
    LaunchProgram(String),
    LaunchScript(String),
}

struct CliOptions {
    config_path: Option<PathBuf>,
    prefill: Option<String>,
    shy: bool,
    no_fuzzy: bool,
    log_level: Option<String>,
    debug_overlay: bool,
    action: CliAction,
}

fn main() -> Result<()> {
    let options = parse_cli_options(env::args().skip(1))?;

    let load_result = AppConfig::load(options.config_path.as_deref());

    let log_level = options
        .log_level
        .as_deref()
        .map(logger::parse_log_level)
        .unwrap_or_else(|| {
            load_result
                .config
                .general
                .log_level
                .as_deref()
                .map(logger::parse_log_level)
                .unwrap_or(log::LevelFilter::Info)
        });

    logger::QstLogger::initialize(log_level, load_result.config.general.log_retention_days)?;
    logger::install_panic_hook();
    info!("qst v{} starting", env!("CARGO_PKG_VERSION"));

    info!("Configuration loaded");
    if let Some(warning) = &load_result.warning {
        warn!("Config warnings: {warning}");
    }

    match options.action {
        CliAction::Help => {
            print_help();
            return Ok(());
        }
        CliAction::Version => {
            println!("qst {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::GenerateConfig => {
            generate_default_config(options.config_path.as_deref())?;
            return Ok(());
        }
        CliAction::ClearHistory => {
            let mut history = History::load();
            history.clear_history();
            info!("Cleared qst history");
            println!("Cleared qst history.");
            return Ok(());
        }
        CliAction::ClearFavorites => {
            let mut history = History::load();
            history.clear_favorites();
            info!("Cleared qst favorite apps");
            println!("Cleared qst favorite apps.");
            return Ok(());
        }
        _ => {}
    }

    let mut app = App::new(load_result.config, load_result.warning, options.debug_overlay);
    app.hide_entries_until_typing = options.shy;
    app.fuzzy_matching_enabled = !options.no_fuzzy;

    if let Some(prefill) = options.prefill {
        app.set_search_query(prefill);
        app.update_filter();
    } else {
        app.update_filter();
    }

    match options.action {
        CliAction::Interactive => {
            info!("Entering interactive mode");
        }
        CliAction::ListPrograms => {
            info!("Listing programs");
            print_programs(&app);
            return Ok(());
        }
        CliAction::ListScripts => {
            info!("Listing scripts");
            print_scripts(&app);
            return Ok(());
        }
        CliAction::LaunchProgram(program_name) => {
            info!("Launching program by name: {program_name}");
            app.launch_program_by_name(&program_name)
                .map_err(anyhow::Error::msg)?;
            return Ok(());
        }
        CliAction::LaunchScript(script_name) => {
            info!("Launching script mode: {script_name}");
            app.launch_script_mode(&script_name)
                .map_err(anyhow::Error::msg)?;
        }
        CliAction::Help | CliAction::Version | CliAction::GenerateConfig | CliAction::ClearHistory | CliAction::ClearFavorites => unreachable!(),
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| draw(f, &mut app))?;
        debug!("Frame drawn");

        if let Event::Key(key) = event::read()? {
            debug!("Key event: {:?}", key);
            if key.kind == KeyEventKind::Press {
                app.total_events += 1;

                if matches_key(&key, app.config.general.jump_to_top_key.as_deref().unwrap_or("alt+up")) {
                    app.select_first();
                    continue;
                }
                if matches_key(&key, app.config.general.jump_to_bottom_key.as_deref().unwrap_or("alt+down")) {
                    app.select_last();
                    continue;
                }

                match key.code {
                    KeyCode::Esc => app.should_quit = true,
                    KeyCode::Enter => app.launch_selected(),
                    KeyCode::Up => app.move_selection(-1),
                    KeyCode::Down => app.move_selection(1),
                    KeyCode::Left => app.move_search_cursor_left(),
                    KeyCode::Right => app.move_search_cursor_right(),
                    _ if matches_key(&key, app.config.general.favorite_key.as_deref().unwrap_or("alt+f")) => {
                        app.toggle_favorite();
                    }
                    _ if matches_key(&key, app.config.general.debug_key.as_deref().unwrap_or("ctrl+d")) => {
                        app.toggle_debug();
                    }
                    KeyCode::Backspace => app.backspace_search_char(),
                    KeyCode::Char(c) => app.insert_search_char(c),
                    KeyCode::Tab => app.auto_complete(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            info!("User requested quit");
            break;
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    info!("qst shutting down");
    Ok(())
}

fn parse_cli_options(args: impl IntoIterator<Item = String>) -> Result<CliOptions> {
    let mut action = None;
    let mut config_path = None;
    let mut prefill = None;
    let mut shy = false;
    let mut no_fuzzy = false;
    let mut log_level = None;
    let mut debug_overlay = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let Some(path) = args.next() else {
                    return Err(anyhow!("--config requires a file path"));
                };
                config_path = Some(PathBuf::from(path));
            }
            "--prefill" => {
                let Some(value) = args.next() else {
                    return Err(anyhow!("--prefill requires a string value"));
                };
                prefill = Some(value);
            }
            "--shy" => shy = true,
            "--no-fuzzy" => no_fuzzy = true,
            "--gen-config" => set_cli_action(&mut action, CliAction::GenerateConfig)?,
            "-h" | "--help" => set_cli_action(&mut action, CliAction::Help)?,
            "-v" | "--version" => set_cli_action(&mut action, CliAction::Version)?,
            "--list-programs" => set_cli_action(&mut action, CliAction::ListPrograms)?,
            "--list-scripts" => set_cli_action(&mut action, CliAction::ListScripts)?,
            "--clear-history" => set_cli_action(&mut action, CliAction::ClearHistory)?,
            "--clear-favorites" => set_cli_action(&mut action, CliAction::ClearFavorites)?,
            "-p" | "--program" => {
                let Some(program_name) = args.next() else {
                    return Err(anyhow!("--program requires a program name"));
                };
                set_cli_action(&mut action, CliAction::LaunchProgram(program_name))?;
            }
            "-s" | "--script" => {
                let Some(script_name) = args.next() else {
                    return Err(anyhow!("--script requires a script name"));
                };
                set_cli_action(&mut action, CliAction::LaunchScript(script_name))?;
            }
            "--log-level" => {
                let Some(value) = args.next() else {
                    return Err(anyhow!("--log-level requires a value (debug, info, warn, error)"));
                };
                log_level = Some(value);
            }
            "--debug-overlay" => debug_overlay = true,
            _ => {}
        }
    }

    Ok(CliOptions {
        config_path,
        prefill,
        shy,
        no_fuzzy,
        log_level,
        debug_overlay,
        action: action.unwrap_or(CliAction::Interactive),
    })
}

fn set_cli_action(action: &mut Option<CliAction>, next: CliAction) -> Result<()> {
    if action.is_some() {
        return Err(anyhow!("only one qst action can be specified at a time"));
    }

    *action = Some(next);
    Ok(())
}

fn generate_default_config(config_path: Option<&Path>) -> Result<()> {
    let path = resolve_config_path(config_path)?;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
            return Err(anyhow!("Unable to create configuration directory: {:?}", parent));
        }
    }

    if path.exists() {
        return Err(anyhow!("Configuration file already exists at {:?}", path));
    }

    let default_config_struct = AppConfig::default();
    let serialized = toml::to_string_pretty(&default_config_struct)
        .map_err(|err| anyhow!("Error serializing default configuration: {}", err))?;

    fs::write(&path, serialized)
        .map_err(|err| anyhow!("Error writing configuration file: {}", err))?;

    println!("Successfully generated default configuration at {:?}", path);
    Ok(())
}

fn resolve_config_path(config_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(config_path) = config_path {
        return Ok(config_path.to_path_buf());
    }

    let Some(mut path) = config_dir() else {
        return Err(anyhow!("Could not determine configuration directory"));
    };

    path.push("qst");
    path.push("config.toml");
    Ok(path)
}

fn print_help() {
    println!("Qst - An Application Launcher");
    println!("Usage: qst [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --config <path>         Use a config file from a custom path");
    println!("  --gen-config            Generate a default config file at ~/.config/qst/config.toml");
    println!("                          (Fails if file already exists)");
    println!("  --clear-history         Clear qst's app history");
    println!("  --clear-favorites       Clear qst's favorite apps");
    println!("  --prefill <string>      Launch qst with an initial search string");
    println!("  --shy                   Hide entries until you start typing");
    println!("  --no-fuzzy              Launch without fuzzy finding");
    println!("  -p, --program <name>    Launch a program directly using fuzzy matching");
    println!("  -s, --script <script>   Open that script by default when qst starts");
    println!("  --list-programs         Print all launchable programs");
    println!("  --list-scripts          Print all scripts and their metadata");
    println!("  -v, --version           Print version information");
    println!("  --debug-overlay         Start with the debug overlay visible");
    println!("  --log-level <level>     Set log level: debug, info, warn, error (default: info)");
    println!("  -h, --help              Print this help message");
}

fn print_programs(app: &App) {
    println!("Programs ({}):", app.entries.len());
    for entry in &app.entries {
        println!("- {} | {}", entry.name, entry.exec_args.join(" "));
    }
}

fn print_scripts(app: &App) {
    let scripts = app.script_listings();
    println!("Scripts ({}):", scripts.len());

    for script in scripts {
        println!("- {} ({})", script.file_id, script.id);

        if let Some(trigger) = &script.trigger {
            println!("  Trigger: {}", trigger);
        }

        if let Some(metadata) = &script.metadata {
            if let Some(name) = &metadata.name {
                println!("  Name: {}", name);
            }
            if let Some(version) = &metadata.version {
                println!("  Version: {}", version);
            }
            if let Some(author) = &metadata.author {
                println!("  Author: {}", author);
            }
            if let Some(description) = &metadata.description {
                println!("  Description: {}", description);
            }
        } else {
            println!("  Metadata: unavailable");
        }

        println!();
    }
}

fn matches_key(key: &event::KeyEvent, config_str: &str) -> bool {
    let Some((required_modifiers, required_code)) = parse_key_binding(config_str) else {
        return false;
    };

    key.code == required_code && key.modifiers.contains(required_modifiers)
}

fn parse_key_binding(config_str: &str) -> Option<(KeyModifiers, KeyCode)> {
    let mut required_modifiers = KeyModifiers::empty();
    let mut required_code = None;

    for part in config_str.to_lowercase().split('+').map(str::trim).filter(|part| !part.is_empty()) {
        match part {
            "ctrl" | "control" => required_modifiers.insert(KeyModifiers::CONTROL),
            "alt" | "option" => required_modifiers.insert(KeyModifiers::ALT),
            "shift" => required_modifiers.insert(KeyModifiers::SHIFT),
            "super" | "cmd" | "win" | "meta" => required_modifiers.insert(KeyModifiers::SUPER),
            "enter" | "return" => required_code = Some(KeyCode::Enter),
            "esc" | "escape" => required_code = Some(KeyCode::Esc),
            "backspace" => required_code = Some(KeyCode::Backspace),
            "tab" => required_code = Some(KeyCode::Tab),
            "space" => required_code = Some(KeyCode::Char(' ')),
            "up" => required_code = Some(KeyCode::Up),
            "down" => required_code = Some(KeyCode::Down),
            "left" => required_code = Some(KeyCode::Left),
            "right" => required_code = Some(KeyCode::Right),
            s if s.len() == 1 => {
                if let Some(ch) = s.chars().next() {
                    required_code = Some(KeyCode::Char(ch));
                } else {
                    return None;
                }
            }
            s if s.starts_with('f') && s.len() > 1 => {
                if let Ok(n) = s[1..].parse::<u8>() {
                    required_code = Some(KeyCode::F(n));
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    required_code.map(|code| (required_modifiers, code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_key_rejects_malformed_bindings() {
        let key = event::KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT);

        assert!(matches_key(&key, "alt+f"));
        assert!(!matches_key(&key, "alt+"));
        assert!(!matches_key(&key, "bogus"));
    }
}
