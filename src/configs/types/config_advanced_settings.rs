//! Module defining the `ConfigAdvancedSettings` struct, which allows to save and reload
//! the application advanced settings.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
pub struct ConfigAdvancedSettings {
    pub scale_factor: f64,
}

impl ConfigAdvancedSettings {
    pub fn load() -> Self {
        if let Ok(advanced_settings) =
            confy::load::<ConfigAdvancedSettings>("sniffnet", "advanced_settings")
        {
            advanced_settings
        } else {
            confy::store(
                "sniffnet",
                "advanced_settings",
                ConfigAdvancedSettings::default(),
            )
            .unwrap_or(());
            ConfigAdvancedSettings::default()
        }
    }
}

impl Default for ConfigAdvancedSettings {
    fn default() -> Self {
        ConfigAdvancedSettings { scale_factor: 1.0 }
    }
}
