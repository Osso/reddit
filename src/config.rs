use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct TokenCache {
    pub refresh_token: Option<String>,
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("reddit")
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

fn token_cache_path() -> PathBuf {
    config_dir().join("token.json")
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config at {path:?}. Run 'reddit config' first"))?;
    toml::from_str(&content).context("Failed to parse config")
}

pub fn save_config(config: &Config) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let content = toml::to_string_pretty(config)?;
    fs::write(config_path(), content)?;
    Ok(())
}

pub fn load_token_cache() -> TokenCache {
    let path = token_cache_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_token_cache(cache: &TokenCache) {
    let path = token_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(path, json);
    }
}
