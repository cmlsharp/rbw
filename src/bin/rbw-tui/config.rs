use std::{env, fs, path::PathBuf};

use ratatui::style::Color;
use serde::Deserialize;

/// Optional YAML configuration file contents.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    pub generator: Option<GeneratorConfig>,
    pub palette: Option<PaletteConfig>,
}

/// Generator-specific config overrides.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GeneratorConfig {
    pub length: Option<u32>,
    pub mode: Option<String>,
    pub nonconfusables: Option<bool>,
}

/// User-overridable color palette values.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PaletteConfig {
    pub border: Option<String>,
    pub text: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
    pub help: Option<String>,
    pub selected_fg: Option<String>,
    pub selected_bg: Option<String>,
    pub danger: Option<String>,
}

/// Effective palette used by the UI after applying config overrides.
#[derive(Debug, Clone)]
pub struct Palette {
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub help: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub danger: Color,
}

impl AppConfig {
    /// Loads config from disk. Returns the config and an optional warning for parse errors.
    /// Missing file is silent (returns defaults). Malformed file returns defaults + warning.
    pub(super) fn load_or_default() -> (Self, Option<String>) {
        let path = config_home().join("bitwarden-tui").join("config.yaml");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return (Self::default(), None),
        };
        match yaml_serde::from_str::<Self>(&text) {
            Ok(config) => (config, None),
            Err(err) => (Self::default(), Some(format!("Config parse error: {err}"))),
        }
    }
}

fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME").map_or_else(|| {
            env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from)
                .join(".config")
        }, PathBuf::from)
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

fn parse_color(value: &str) -> Option<Color> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "yellow" => Some(Color::Yellow),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        other => parse_hex_color(other),
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            border: Color::Rgb(137, 180, 250),
            text: Color::Rgb(205, 214, 244),
            muted: Color::Rgb(166, 173, 200),
            accent: Color::Rgb(249, 226, 175),
            help: Color::Rgb(180, 190, 254),
            selected_fg: Color::Rgb(30, 30, 46),
            selected_bg: Color::Rgb(203, 166, 247),
            danger: Color::Rgb(243, 139, 168),
        }
    }
}

impl Palette {
    /// Builds the effective palette by overlaying config values onto the defaults.
    pub fn from_config(config: &AppConfig) -> Self {
        let mut palette = Self::default();
        if let Some(custom) = &config.palette {
            apply_color(&mut palette.border, &custom.border);
            apply_color(&mut palette.text, &custom.text);
            apply_color(&mut palette.muted, &custom.muted);
            apply_color(&mut palette.accent, &custom.accent);
            apply_color(&mut palette.help, &custom.help);
            apply_color(&mut palette.selected_fg, &custom.selected_fg);
            apply_color(&mut palette.selected_bg, &custom.selected_bg);
            apply_color(&mut palette.danger, &custom.danger);
        }
        palette
    }
}

fn apply_color(target: &mut Color, source: &Option<String>) {
    if let Some(color) = source.as_deref().and_then(parse_color) {
        *target = color;
    }
}
