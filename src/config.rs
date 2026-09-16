//! Configuration from the user config dir (config.toml) plus the LICHESS_TOKEN env var.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub token: Option<String>,
    pub theme: String,
    pub camouflage: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            token: None,
            theme: "claude".to_string(),
            camouflage: false,
        }
    }
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Config> {
        toml::from_str(text).context("invalid config.toml")
    }

    pub fn apply_env(&mut self, env_token: Option<String>) {
        if let Some(t) = env_token.filter(|t| !t.trim().is_empty()) {
            self.token = Some(t);
        }
    }

    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("terminal_chess").join("config.toml"))
    }

    /// Load from disk (missing file is fine) and then apply the environment.
    pub fn load() -> Result<Config> {
        let mut cfg = match Self::path().filter(|p| p.exists()) {
            Some(p) => {
                let text = std::fs::read_to_string(&p)
                    .with_context(|| format!("reading {}", p.display()))?;
                Self::from_toml(&text)?
            }
            None => Config::default(),
        };
        cfg.apply_env(std::env::var("LICHESS_TOKEN").ok());
        Ok(cfg)
    }
}
