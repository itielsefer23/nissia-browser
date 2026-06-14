//! Session profile persistence.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use nissia_cdp::commands::Cookie;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProfile {
    pub version: u32,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub cookies: Vec<Cookie>,
    pub local_storage: HashMap<String, String>,
    pub session_storage: HashMap<String, String>,
    pub last_url: String,
}

impl SessionProfile {
    fn sessions_dir() -> PathBuf {
        let dir = crate::data_dir().join("sessions");
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn file_path(name: &str) -> PathBuf {
        Self::sessions_dir().join(format!("{name}.json"))
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::file_path(&self.name);
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    pub fn load(name: &str) -> std::io::Result<Self> {
        let path = Self::file_path(name);
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(std::io::Error::other)
    }

    pub fn delete(name: &str) -> std::io::Result<()> {
        let path = Self::file_path(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn list() -> std::io::Result<Vec<String>> {
        let dir = Self::sessions_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }
}
