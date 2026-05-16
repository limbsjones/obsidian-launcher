use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| {
        let home = home_dir();
        PathBuf::from(format!("{}/.cache", home.display()))
    })
}

fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| {
        let home = home_dir();
        PathBuf::from(format!("{}/.config", home.display()))
    })
}

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
            vault_path: home_dir().join("Obsidian Vault"),
            index_path: None,
            max_results: 50,
            hotkey: Some("Super+Space".to_string()),
        }
    }
}

impl Config {
    pub fn index_path(&self) -> PathBuf {
        self.index_path.clone().unwrap_or_else(|| {
            cache_dir()
                .join("obsidian-launcher")
                .join("index")
        })
    }

    pub fn load() -> Result<Self> {
        let config_path = config_dir()
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
        let config_dir = config_dir().join("obsidian-launcher");
        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}
