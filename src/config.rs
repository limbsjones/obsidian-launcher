use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub vault_path: PathBuf,
    pub index_path: Option<PathBuf>,
    pub max_results: usize,
    pub hotkey: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vault_path: dirs::home_dir()
                .unwrap_or_default()
                .join("Obsidian Vault"),
            index_path: None,
            max_results: 50,
            hotkey: Some("Super+Space".to_string()),
        }
    }
}

impl Config {
    pub fn index_path(&self) -> PathBuf {
        self.index_path.clone().unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_default()
                .join("obsidian-launcher")
                .join("index")
        })
    }

    pub fn load() -> Result<Self> {
        let config_path = dirs::config_dir()
            .unwrap_or_default()
            .join("obsidian-launcher")
            .join("config.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .unwrap_or_default()
            .join("obsidian-launcher");
        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}
