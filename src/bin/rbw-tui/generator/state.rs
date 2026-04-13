use crate::config::AppConfig;
use crate::form;
use crate::text_input::TextInput;

pub const GENERATOR_MODES: [&str; 4] = ["standard", "no-symbols", "diceware", "numeric"];

/// Effective generator defaults and current generator values.
#[derive(Debug, Clone)]
pub struct Settings {
    pub length: u32,
    pub mode: String,
    pub nonconfusables: bool,
}

/// Generator modal state.
#[derive(Debug, Clone)]
pub struct State {
    pub settings: Settings,
    pub selected_index: usize,
    pub editing_length: bool,
    pub length_buffer: TextInput,
    pub length_touched: bool,
    pub return_to_form: Option<form::State>,
}

impl State {
    /// Starts a fresh generator state from the persisted generator settings.
    pub fn from_settings(settings: Settings) -> Self {
        let length_buffer = TextInput::from_str(&settings.length.to_string());
        Self {
            settings,
            selected_index: 0,
            editing_length: false,
            length_buffer,
            length_touched: false,
            return_to_form: None,
        }
    }

    fn default_length(mode: &str) -> u32 {
        match mode {
            "diceware" => 6,
            "numeric" => 8,
            _ => 24,
        }
    }

    /// Moves the generator cursor down, wrapping around.
    pub fn move_down(&mut self) {
        if !self.editing_length {
            self.selected_index = if self.selected_index >= 2 {
                0
            } else {
                self.selected_index + 1
            };
        }
    }

    /// Moves the generator cursor up, wrapping around.
    pub fn move_up(&mut self) {
        if !self.editing_length {
            self.selected_index = if self.selected_index == 0 {
                2
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Activates the selected generator row.
    pub fn activate(&mut self) {
        if self.editing_length {
            self.commit_length();
            return;
        }
        if self.selected_index == 0 {
            let current = GENERATOR_MODES
                .iter()
                .position(|mode| *mode == self.settings.mode)
                .unwrap_or(0);
            let next = (current + 1) % GENERATOR_MODES.len();
            self.settings.mode = GENERATOR_MODES[next].to_string();
            if !self.length_touched {
                self.settings.length = Self::default_length(&self.settings.mode);
                self.length_buffer.set(&self.settings.length.to_string());
            }
        } else if self.selected_index == 2 {
            self.settings.nonconfusables = !self.settings.nonconfusables;
        }
    }

    /// Adjusts the length field by one step.
    pub fn adjust_length(&mut self, delta: i32) {
        self.length_touched = true;
        let current = self.settings.length as i32;
        self.settings.length = (current + delta).clamp(4, 128) as u32;
        self.length_buffer.set(&self.settings.length.to_string());
    }

    /// Commits the current inline length buffer into generator settings.
    pub fn commit_length(&mut self) {
        let parsed = self
            .length_buffer
            .as_str()
            .parse::<u32>()
            .ok()
            .unwrap_or(self.settings.length);
        self.length_touched = true;
        self.settings.length = parsed.clamp(4, 128);
        self.length_buffer.set(&self.settings.length.to_string());
        self.editing_length = false;
    }
}

impl Settings {
    /// Generates a password from the current settings.
    pub fn generate(&self) -> String {
        let ty = match self.mode.as_str() {
            "diceware" => rbw::pwgen::Type::Diceware,
            "numeric" => rbw::pwgen::Type::Numbers,
            "no-symbols" => rbw::pwgen::Type::NoSymbols,
            _ if self.nonconfusables => rbw::pwgen::Type::NonConfusables,
            _ => rbw::pwgen::Type::AllChars,
        };
        rbw::pwgen::pwgen(ty, self.length as usize)
    }

    /// Builds the effective generator defaults from the optional config file.
    pub fn from_config(config: &AppConfig) -> Self {
        let mut defaults = Self {
            length: 24,
            mode: "standard".to_string(),
            nonconfusables: false,
        };
        if let Some(generator) = &config.generator {
            if let Some(length) = generator.length {
                defaults.length = length;
            }
            if let Some(mode) = &generator.mode {
                defaults.mode = mode.clone();
            }
            if let Some(nonconfusables) = generator.nonconfusables {
                defaults.nonconfusables = nonconfusables;
            }
        }
        defaults
    }
}
