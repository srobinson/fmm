
use std::path::PathBuf;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: String,
    pub paging: PagingMode,
    pub line_numbers: bool,
    pub tab_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PagingMode {
    Always,
    QuitIfOneScreen,
    Never,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "Monokai Extended".to_string(),
            paging: PagingMode::QuitIfOneScreen,
            line_numbers: true,
            tab_width: 4,
        }
    }
}

impl Default for PagingMode {
    fn default() -> Self {
        Self::QuitIfOneScreen
    }
}

pub fn load_config(path: &PathBuf) -> Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}
