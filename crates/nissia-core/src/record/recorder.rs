//! Records sequences of snap+action commands into a workflow.

use std::collections::HashMap;

use super::workflow::{Workflow, WorkflowStep};
use crate::element_map::ElementMap;

pub struct Recorder;

impl Recorder {
    /// Load recorder state from disk (for cross-invocation recording).
    pub fn load_state() -> std::io::Result<Option<RecorderState>> {
        let path = crate::data_dir().join("recording.json");
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(path)?;
        let state: RecorderState = serde_json::from_str(&json).map_err(std::io::Error::other)?;
        Ok(Some(state))
    }

    /// Save recorder state to disk.
    pub fn save_state(state: &RecorderState) -> std::io::Result<()> {
        let path = crate::data_dir().join("recording.json");
        let json = serde_json::to_string(state).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Clear recorder state from disk.
    pub fn clear_state() -> std::io::Result<()> {
        let path = crate::data_dir().join("recording.json");
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Record a command step.
    pub fn record_step(
        state: &mut RecorderState,
        command: &str,
        args: HashMap<String, String>,
        element_map: Option<&ElementMap>,
    ) {
        state.seq += 1;

        let selector_hint = args.get("ref").and_then(|ref_id| {
            element_map.and_then(|map| map.get(ref_id).map(|e| e.selector_hint.clone()))
        });

        let step = WorkflowStep {
            seq: state.seq,
            command: command.to_string(),
            args,
            selector_hint,
            snapshot_hash: None,
            timestamp_ms: state.elapsed_ms(),
        };

        state.steps.push(step);
    }

    /// Finalize recording into a workflow.
    pub fn finalize(state: RecorderState) -> Workflow {
        let replay_command = format!("nissia replay {}", state.name);
        Workflow {
            version: 1,
            name: state.name,
            replay_command,
            created_at: state.created_at.clone(),
            steps: state.steps,
        }
    }
}

/// Persistent recorder state across CLI invocations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecorderState {
    pub name: String,
    pub created_at: String,
    pub started_at_ms: u64,
    pub seq: u32,
    pub steps: Vec<WorkflowStep>,
}

impl RecorderState {
    pub fn new(name: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            name: name.to_string(),
            created_at: format!("{}", now.as_secs()),
            started_at_ms: now.as_millis() as u64,
            seq: 0,
            steps: Vec::new(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(self.started_at_ms)
    }
}
