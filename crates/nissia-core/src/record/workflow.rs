//! Workflow data structures for recording and replay.
//!
//! Workflows are stored in two scopes:
//! - **Project scope**: `.nissia/workflows/` in the current directory (shared via git)
//! - **User scope**: `~/.local/share/nissia/workflows/` (personal, machine-local)
//!
//! Load order: project scope first, then user scope (project overrides user).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub version: u32,
    pub name: String,
    /// How to replay this workflow. Agents should run this command directly.
    pub replay_command: String,
    pub created_at: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub seq: u32,
    pub command: String,
    pub args: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_hash: Option<String>,
    pub timestamp_ms: u64,
}

impl Workflow {
    /// Project-scope workflows directory: `.nissia/workflows/` in cwd.
    fn project_workflows_dir() -> Option<PathBuf> {
        let dir = PathBuf::from(".nissia/workflows");
        if dir.exists() || PathBuf::from(".nissia").exists() {
            std::fs::create_dir_all(&dir).ok();
            Some(dir)
        } else {
            None
        }
    }

    /// User-scope workflows directory: `~/.local/share/nissia/workflows/`.
    fn user_workflows_dir() -> PathBuf {
        let dir = crate::data_dir().join("workflows");
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    /// Save workflow. Uses project scope if `.nissia/` exists, otherwise user scope.
    pub fn save(&self) -> std::io::Result<PathBuf> {
        let dir = Self::project_workflows_dir().unwrap_or_else(Self::user_workflows_dir);
        let path = dir.join(format!("{}.json", self.name));
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    /// Load workflow. Checks project scope first, then user scope.
    pub fn load(name: &str) -> std::io::Result<Self> {
        let filename = format!("{name}.json");

        // Project scope first
        if let Some(dir) = Self::project_workflows_dir() {
            let path = dir.join(&filename);
            if path.exists() {
                let json = std::fs::read_to_string(path)?;
                return serde_json::from_str(&json).map_err(std::io::Error::other);
            }
        }

        // User scope fallback
        let path = Self::user_workflows_dir().join(&filename);
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(std::io::Error::other)
    }

    /// List workflows from both scopes.
    pub fn list() -> std::io::Result<Vec<(String, String)>> {
        let mut results: Vec<(String, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Project scope
        if let Some(dir) = Self::project_workflows_dir() {
            for name in Self::list_dir(&dir)? {
                seen.insert(name.clone());
                results.push((name, "project".to_string()));
            }
        }

        // User scope
        let user_dir = Self::user_workflows_dir();
        for name in Self::list_dir(&user_dir)? {
            if !seen.contains(&name) {
                results.push((name, "user".to_string()));
            }
        }

        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    fn list_dir(dir: &PathBuf) -> std::io::Result<Vec<String>> {
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

    /// Delete workflow. Checks project scope first, then user scope.
    pub fn delete(name: &str) -> std::io::Result<()> {
        let filename = format!("{name}.json");

        if let Some(dir) = Self::project_workflows_dir() {
            let path = dir.join(&filename);
            if path.exists() {
                return std::fs::remove_file(path);
            }
        }

        let path = Self::user_workflows_dir().join(&filename);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
