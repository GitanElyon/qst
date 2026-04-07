use dirs::config_dir;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders},
};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use std::fs;

pub struct ConfigLoadResult {
    pub config: AppConfig,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub features: FeaturesConfig,
    pub window: SectionConfig,
    pub outer_box: SectionConfig,
    pub qst_ascii: QstAsciiConfig,
    pub input: SectionConfig,
    #[serde(alias = "results")]
    pub list: ResultsConfig,
    pub entry: EntryConfig,
    pub entry_selected: SectionConfig,
    pub meta: MetaConfig,
    pub text: TextConfig,
}

impl AppConfig {
    pub fn load() -> ConfigLoadResult {
        let default = Self::default();
        let mut warning = None;
        let config = match config_dir() {
            Some(mut dir) => {
                dir.push("qst");
                if fs::create_dir_all(&dir).is_err() {
                    warning = Some("Unable to create ~/.config/qst, using defaults".into());
                    default
                } else {
                    let config_path = dir.join("config.toml");
                    if config_path.exists() {
                        match fs::read_to_string(&config_path) {
                            Ok(contents) => match toml::from_str::<AppConfig>(&contents) {
                                Ok(parsed) => parsed,
                                Err(err) => {
                                    warning = Some(format!(
                                        "Invalid config ({}). Falling back to defaults.",
                                        err
                                    ));
                                    default
                                }
                            },
                            Err(err) => {
                                warning = Some(format!(
                                    "Failed to read config ({}). Using defaults.",
                                    err
                                ));
                                default
                            }
                        }
                    } else {
                        if let Ok(serialized) = toml::to_string_pretty(&default) {
                            let _ = fs::write(&config_path, serialized);
                        }
                        default
                    }
                }
            }
            None => {
                warning = Some("Could not locate configuration directory. Using defaults.".into());
                default
            }
        };
        ConfigLoadResult { config, warning }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        include!("../assets/defaults.rs")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ResultsConfig {
    #[serde(flatten)]
    pub section: SectionConfig,

    #[serde(alias = "applications-title")]
    pub apps_title: Option<String>,
    #[serde(alias = "directories-title")]
    pub files_title: Option<String>,
}

impl Default for ResultsConfig {
    fn default() -> Self {
        Self {
            section: SectionConfig::default(),
            apps_title: None,
            files_title: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct QstAsciiConfig {
    #[serde(flatten)]
    pub section: SectionConfig,
    #[serde(default, deserialize_with = "deserialize_color_stops")]
    pub gradient_colors: Vec<String>,
    pub gradient_angle: u16,
    pub alignment: Option<TextAlignment>,
    pub padding: PaddingConfig,
    pub custom_path: Option<String>,
}

impl Default for QstAsciiConfig {
    fn default() -> Self {
        Self {
            section: SectionConfig {
               visible: Some(true),
                ..SectionConfig::default()
            },
            gradient_colors: vec![String::from("#6464ff"), String::from("#c864ff")],
            gradient_angle: 90,
            alignment: Some(TextAlignment::Center),
            padding: PaddingConfig::default(),
            custom_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct EntryConfig {
    #[serde(default, deserialize_with = "deserialize_color_stops")]
    pub fg: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_color_stops")]
    pub bg: Vec<String>,
    pub gradient_angle: u16,
}

impl EntryConfig {
    pub fn base_style(&self, fallback: Style) -> Style {
        let mut style = fallback;
        if let Some(color) = self.fg.first().and_then(|v| parse_color(v)) {
            style = style.fg(color);
        }
        if let Some(color) = self.bg.first().and_then(|v| parse_color(v)) {
            style = style.bg(color);
        }
        style
    }
}

impl Default for EntryConfig {
    fn default() -> Self {
        Self {
            fg: Vec::new(),
            bg: Vec::new(),
            gradient_angle: 90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct MetaConfig {
    pub active: SectionConfig,
    pub urgent: SectionConfig,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            active: SectionConfig::default(),
            urgent: SectionConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct PaddingConfig {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GeneralConfig {
    pub rounded_corners: bool,
    pub show_borders: bool,
    pub highlight_symbol: Option<String>,
    pub favorite_symbol: Option<String>,
    pub favorite_key: Option<String>,
    pub jump_to_top_key: Option<String>,
    pub jump_to_bottom_key: Option<String>,
    pub clipboard_command: Option<String>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            rounded_corners: true,
            show_borders: true,
            highlight_symbol: Some(String::from(">> ")),
            favorite_symbol: Some(String::from("★ ")),
            favorite_key: Some(String::from("alt+f")),
            jump_to_top_key: Some(String::from("alt+up")),
            jump_to_bottom_key: Some(String::from("alt+down")),
            clipboard_command: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct FeaturesConfig {
    pub enable_file_explorer: bool,
    pub enable_launch_args: bool,
    pub enable_auto_complete: bool,
    pub dirs_first: bool,
    pub show_duplicates: bool,
    pub recent_first: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            enable_file_explorer: true,
            enable_launch_args: true,
            enable_auto_complete: true,
            dirs_first: true,
            show_duplicates: false,
            recent_first: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SectionConfig {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_color_stops")]
    pub fg: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_color_stops")]
    pub bg: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_color_stops")]
    pub border_color: Vec<String>,
    #[serde(alias = "border-gradient-angle")]
    pub border_angle: u16,
    pub gradient_angle: u16,
    pub full_width_highlight: Option<bool>,
    pub rounded: Option<bool>,
    pub borders: Option<bool>,
    #[serde(alias = "visable")]
    pub visible: Option<bool>,
    pub title_alignment: Option<TextAlignment>,
}

impl SectionConfig {
    pub fn is_visible(&self) -> bool {
        self.visible.unwrap_or(true)
    }

    pub fn style(&self) -> Style {
        let mut style = Style::default();
        if let Some(color) = self.fg.first().and_then(|v| parse_color(v)) {
            style = style.fg(color);
        }
        if let Some(color) = self.bg.first().and_then(|v| parse_color(v)) {
            style = style.bg(color);
        }
        style
    }

    pub fn border_offset(&self, general: &GeneralConfig) -> u16 {
        if self.draws_borders(general) { 1 } else { 0 }
    }

    pub fn draws_borders(&self, general: &GeneralConfig) -> bool {
        self.borders.unwrap_or(general.show_borders)
    }

    pub fn block_with_title<'a>(&self, general: &GeneralConfig, title: &'a str) -> Block<'a> {
        let mut block = Block::default().title(title);

        block = block.title_alignment(self.title_alignment.unwrap_or(TextAlignment::Left).into());

        if self.draws_borders(general) {
            block = block.borders(Borders::ALL);
            let rounded = self.rounded.unwrap_or(general.rounded_corners);
            block = block.border_type(if rounded {
                BorderType::Rounded
            } else {
                BorderType::Plain
            });

            if let Some(color) = self.border_color.first().and_then(|v| parse_color(v)) {
                block = block.border_style(Style::default().fg(color));
            }
        }

        block.style(self.style())
    }

    pub fn block<'a>(&self, general: &GeneralConfig, fallback_title: &'a str) -> Block<'a> {
        let mut block = Block::default().title(
            self.title
                .clone()
                .unwrap_or_else(|| fallback_title.to_string()),
        );

        block = block.title_alignment(self.title_alignment.unwrap_or(TextAlignment::Left).into());

        if self.draws_borders(general) {
            block = block.borders(Borders::ALL);
            let rounded = self.rounded.unwrap_or(general.rounded_corners);
            block = block.border_type(if rounded {
                BorderType::Rounded
            } else {
                BorderType::Plain
            });

            if let Some(color) = self.border_color.first().and_then(|v| parse_color(v)) {
                block = block.border_style(Style::default().fg(color));
            }
        }

        block.style(self.style())
    }
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            title: None,
            fg: Vec::new(),
            bg: Vec::new(),
            border_color: Vec::new(),
            border_angle: 90,
            gradient_angle: 90,
            full_width_highlight: None,
            rounded: None,
            borders: None,
            visible: None,
            title_alignment: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TextConfig {
    #[serde(flatten)]
    pub section: SectionConfig,
    pub alignment: Option<TextAlignment>,
}

impl TextConfig {
    pub fn style(&self) -> Style {
        self.section.style()
    }

    pub fn alignment(&self) -> TextAlignment {
        self.alignment.unwrap_or(TextAlignment::Left)
    }

    pub fn is_visible(&self) -> bool {
        self.section.is_visible()
    }
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            section: SectionConfig {
                fg: vec![String::from("#f2f5f7")],
                ..SectionConfig::default()
            },
            alignment: Some(TextAlignment::Left),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl From<TextAlignment> for Alignment {
    fn from(value: TextAlignment) -> Self {
        match value {
            TextAlignment::Left => Alignment::Left,
            TextAlignment::Center => Alignment::Center,
            TextAlignment::Right => Alignment::Right,
        }
    }
}

pub fn parse_color(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            let apply_alpha = |channel: u8| -> u8 {
                let value = (channel as u16 * a as u16) / 255;
                value as u8
            };
            return Some(Color::Rgb(apply_alpha(r), apply_alpha(g), apply_alpha(b)));
        }
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "dark-grey" => Some(Color::DarkGray),
        "lightred" | "light-red" => Some(Color::LightRed),
        "lightgreen" | "light-green" => Some(Color::LightGreen),
        "lightblue" | "light-blue" => Some(Color::LightBlue),
        "lightmagenta" | "light-magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light-cyan" => Some(Color::LightCyan),
        "lightyellow" | "light-yellow" => Some(Color::LightYellow),
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ColorStopsInput {
    Single(String),
    Multiple(Vec<String>),
}

fn deserialize_color_stops<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<ColorStopsInput>::deserialize(deserializer)?;
    Ok(match value {
        None => Vec::new(),
        Some(ColorStopsInput::Single(single)) => vec![single],
        Some(ColorStopsInput::Multiple(list)) => list,
    })
}

