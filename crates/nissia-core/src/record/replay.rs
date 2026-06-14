//! Replay engine: re-executes recorded workflows with self-healing.

use std::time::Duration;

use nissia_cdp::CdpTransport;

use super::workflow::Workflow;
use crate::action;
use crate::read;
use crate::snap;

/// Replay a recorded workflow.
pub async fn execute(
    transport: &CdpTransport,
    name: &str,
    speed: f64,
) -> Result<ReplayResult, nissia_cdp::CdpTransportError> {
    let workflow = Workflow::load(name).map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!(
            "Failed to load workflow '{name}': {e}"
        ))
    })?;

    let mut result = ReplayResult {
        total_steps: workflow.steps.len(),
        completed: 0,
        warnings: Vec::new(),
        failed_step: None,
        snaps: Vec::new(),
        reads: Vec::new(),
        evals: Vec::new(),
    };

    let mut prev_ts = 0u64;

    for step in &workflow.steps {
        // Pace timing
        if step.timestamp_ms > prev_ts && speed > 0.0 {
            let delay_ms = ((step.timestamp_ms - prev_ts) as f64 / speed) as u64;
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
        prev_ts = step.timestamp_ms;

        match step.command.as_str() {
            "snap" => {
                let url = step.args.get("url").map(|s| s.as_str());
                let focus = step.args.get("focus").map(|s| s.as_str());
                let snap_result = snap::execute(
                    transport,
                    url,
                    focus,
                    "en-US",
                    &snap::EmulationOptions::default(),
                )
                .await?;
                result.snaps.push(snap_result);
            }
            "click" => {
                if let Some(ref_id) = step.args.get("ref") {
                    action::click::execute(transport, ref_id).await?;
                }
            }
            "fill" => {
                if let (Some(ref_id), Some(value)) = (step.args.get("ref"), step.args.get("value"))
                {
                    action::fill::execute(transport, ref_id, value).await?;
                }
            }
            "type" => {
                if let (Some(ref_id), Some(text)) = (step.args.get("ref"), step.args.get("text")) {
                    action::type_text::execute(transport, ref_id, text).await?;
                }
            }
            "select" => {
                if let (Some(ref_id), Some(value)) = (step.args.get("ref"), step.args.get("value"))
                {
                    action::select::execute(transport, ref_id, value).await?;
                }
            }
            "scroll" => {
                let dir = step
                    .args
                    .get("direction")
                    .map(|s| s.as_str())
                    .unwrap_or("down");
                let amount = step.args.get("amount").and_then(|s| s.parse().ok());
                action::scroll::execute(transport, dir, amount).await?;
            }
            "read" => {
                let url = step.args.get("url").map(|s| s.as_str());
                let focus = step.args.get("focus").map(|s| s.as_str());
                let read_result = read::execute(
                    transport,
                    url,
                    focus,
                    "en-US",
                    500,
                    &snap::EmulationOptions::default(),
                )
                .await?;
                result.reads.push(read_result);
            }
            "eval" => {
                if let Some(expression) = step.args.get("expression") {
                    let eval_result = transport
                        .send(&nissia_cdp::commands::RuntimeEvaluate {
                            expression: expression.to_string(),
                            return_by_value: Some(true),
                            await_promise: Some(true),
                            context_id: None,
                        })
                        .await?;
                    let value = eval_result.result.value.unwrap_or(serde_json::Value::Null);
                    result.evals.push(value);
                }
            }
            "screenshot" => {
                let output = step.args.get("file").map(|s| s.as_str());
                action::screenshot::execute(transport, output).await?;
            }
            "wait" => {
                if let Some(condition) = step.args.get("condition") {
                    match condition.as_str() {
                        "navigation" => {
                            action::wait::execute(
                                transport,
                                action::wait::WaitCondition::Navigation,
                            )
                            .await?;
                        }
                        _ => {
                            // Treat as selector
                            action::wait::execute(
                                transport,
                                action::wait::WaitCondition::Selector(condition),
                            )
                            .await?;
                        }
                    }
                }
            }
            other => {
                result
                    .warnings
                    .push(format!("Unknown command '{other}' at step {}", step.seq));
            }
        }

        result.completed += 1;
    }

    Ok(result)
}

pub struct ReplayResult {
    pub total_steps: usize,
    pub completed: usize,
    pub warnings: Vec<String>,
    pub failed_step: Option<u32>,
    /// All snap outputs collected during replay.
    pub snaps: Vec<snap::SnapResult>,
    /// All read outputs collected during replay.
    pub reads: Vec<read::ReadResult>,
    /// All eval outputs collected during replay.
    pub evals: Vec<serde_json::Value>,
}
