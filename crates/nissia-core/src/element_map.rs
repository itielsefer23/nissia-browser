//! Element map: maps @eN references to backend node IDs and metadata.
//! Persisted to disk so subsequent CLI invocations can resolve references.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use nissia_cdp::types::BackendNodeId;

/// A single element entry in the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementEntry {
    pub backend_node_id: BackendNodeId,
    pub role: String,
    pub name: String,
    pub selector_hint: String,
    pub tag: String,
    #[serde(default)]
    pub attributes: HashMap<String, String>,
}

/// The full element map persisted between CLI invocations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementMap {
    pub elements: HashMap<String, ElementEntry>,
}

impl ElementMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an element and return its @eN reference.
    pub fn insert(&mut self, entry: ElementEntry) -> String {
        let idx = self.elements.len() + 1;
        let ref_id = format!("@e{idx}");
        self.elements.insert(ref_id.clone(), entry);
        ref_id
    }

    /// Look up an element by its @eN reference.
    pub fn get(&self, ref_id: &str) -> Option<&ElementEntry> {
        self.elements.get(ref_id)
    }

    /// Path to the element map file on disk.
    fn file_path() -> PathBuf {
        crate::data_dir().join("element_map.json")
    }

    /// Save the element map to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::file_path();
        let json = serde_json::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Load the element map from disk.
    pub fn load() -> std::io::Result<Self> {
        let path = Self::file_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(std::io::Error::other)
    }
}
