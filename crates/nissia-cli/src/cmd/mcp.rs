//! MCP (Model Context Protocol) server — JSON-RPC 2.0 over stdio.
//! Start with `nissia mcp` to expose nissia tools to Claude Code or any MCP client.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "nissia";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tool definitions exposed to the MCP client.
fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "snap",
                "description": "Extract interactable elements from the current page as @eN references. Always run before acting on elements. Re-run after navigation.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to navigate to (optional if already on a page)"
                        },
                        "focus": {
                            "type": "string",
                            "description": "CSS selector to limit extraction scope (e.g. 'main', '#content')"
                        },
                        "lang": {
                            "type": "string",
                            "description": "Accept-Language header value (e.g. en-US, ko, ja)",
                            "default": "en-US"
                        }
                    }
                }
            },
            {
                "name": "read",
                "description": "Read visible text content as structured markdown (headings, paragraphs, lists, tables). Use to understand page content without taking a screenshot. Complement to snap.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "URL to navigate to (optional if already on a page)"
                        },
                        "focus": {
                            "type": "string",
                            "description": "CSS selector to limit scope (e.g. 'main', '#content', '.pr-list')"
                        },
                        "max_lines": {
                            "type": "integer",
                            "description": "Maximum lines to return",
                            "default": 200
                        },
                        "lang": {
                            "type": "string",
                            "description": "Accept-Language header value",
                            "default": "en-US"
                        }
                    }
                }
            },
            {
                "name": "click",
                "description": "Click an element by @eN reference from snap output.",
                "inputSchema": {
                    "type": "object",
                    "required": ["ref"],
                    "properties": {
                        "ref": {
                            "type": "string",
                            "description": "Element reference from snap (e.g. @e1)"
                        },
                        "dry_run": { "type": "boolean", "default": false }
                    }
                }
            },
            {
                "name": "fill",
                "description": "Set an input field's value (clears existing). Use for <input>, <textarea>.",
                "inputSchema": {
                    "type": "object",
                    "required": ["ref", "value"],
                    "properties": {
                        "ref": { "type": "string", "description": "Element reference (e.g. @e2)" },
                        "value": { "type": "string", "description": "Value to set" },
                        "dry_run": { "type": "boolean", "default": false }
                    }
                }
            },
            {
                "name": "type",
                "description": "Type text character by character with key events. Use for autocomplete/search inputs.",
                "inputSchema": {
                    "type": "object",
                    "required": ["ref", "text"],
                    "properties": {
                        "ref": { "type": "string" },
                        "text": { "type": "string", "description": "Text to type" },
                        "dry_run": { "type": "boolean", "default": false }
                    }
                }
            },
            {
                "name": "select",
                "description": "Select an option in a <select> dropdown by value.",
                "inputSchema": {
                    "type": "object",
                    "required": ["ref", "value"],
                    "properties": {
                        "ref": { "type": "string" },
                        "value": { "type": "string", "description": "Option value to select" },
                        "dry_run": { "type": "boolean", "default": false }
                    }
                }
            },
            {
                "name": "scroll",
                "description": "Scroll the page in a direction.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "direction": {
                            "type": "string",
                            "enum": ["up", "down", "left", "right"],
                            "default": "down"
                        },
                        "amount": { "type": "integer", "description": "Pixels to scroll (default: 400)" },
                        "dry_run": { "type": "boolean", "default": false }
                    }
                }
            },
            {
                "name": "screenshot",
                "description": "Capture a PNG screenshot of the current page.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "Output file path" }
                    }
                }
            },
            {
                "name": "wait",
                "description": "Wait for navigation, a CSS selector to appear, or a timeout in ms.",
                "inputSchema": {
                    "type": "object",
                    "required": ["condition"],
                    "properties": {
                        "condition": {
                            "type": "string",
                            "description": "'navigation' | CSS selector | milliseconds (e.g. '2000')"
                        }
                    }
                }
            },
            {
                "name": "eval",
                "description": "Execute JavaScript on the current page and return the result. Use for extracting data that snap/read can't capture (e.g. product cards, dynamic content, complex DOM queries).",
                "inputSchema": {
                    "type": "object",
                    "required": ["expression"],
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "JavaScript expression to evaluate. Use JSON.stringify() for complex return values."
                        }
                    }
                }
            },
            {
                "name": "browser_launch",
                "description": "Launch Chrome with remote debugging. Uses persistent profile to keep cookies/login state.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "headless": { "type": "boolean", "default": false },
                        "background": { "type": "boolean", "default": true },
                        "profile": { "type": "string", "description": "Profile name (default: 'default'). Persistent profiles reduce bot detection." }
                    }
                }
            },
            {
                "name": "browser_stop",
                "description": "Stop the running Chrome instance managed by nissia.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "browser_status",
                "description": "Check if Chrome is running on the given port.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

/// After a mutation action, perform settle + re-snap and return combined output.
async fn mcp_action_with_snap(transport: &nissia_cdp::CdpTransport, action: &str) -> Result<String> {
    let emu = nissia_core::snap::EmulationOptions::default();
    if let Some(snap) = nissia_core::action::post_action_snap(transport, "en-US", &emu).await {
        Ok(format!("ok\n\n---\n\n{}", snap.output))
    } else {
        Ok(format!("{{\"status\":\"ok\",\"action\":\"{action}\"}}"))
    }
}

/// Call a tool and return text output.
async fn call_tool(name: &str, args: &Value, port: u16) -> Result<String> {
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    match name {
        "snap" => {
            let url = args.get("url").and_then(|v| v.as_str());
            let focus = args.get("focus").and_then(|v| v.as_str());
            let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("en-US");

            if let Some(f) = focus {
                crate::validate::css_selector(f)?;
            }

            let transport = nissia_cdp::connect(port).await?;
            transport.send(&nissia_cdp::commands::PageEnable {}).await?;
            let emu = nissia_core::snap::EmulationOptions::default();
            let result = nissia_core::snap::execute(&transport, url, focus, lang, &emu).await?;
            Ok(result.output)
        }
        "read" => {
            let url = args.get("url").and_then(|v| v.as_str());
            let focus = args.get("focus").and_then(|v| v.as_str());
            let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("en-US");
            let max_lines = args
                .get("max_lines")
                .and_then(|v| v.as_u64())
                .unwrap_or(200) as usize;

            if let Some(f) = focus {
                crate::validate::css_selector(f)?;
            }

            let transport = nissia_cdp::connect(port).await?;
            let emu = nissia_core::snap::EmulationOptions::default();
            let result =
                nissia_core::read::execute(&transport, url, focus, lang, max_lines, &emu).await?;
            Ok(result.output)
        }
        "click" => {
            let element_ref = args
                .get("ref")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: ref"))?;
            crate::validate::element_ref(element_ref)?;
            if dry_run {
                return Ok(format!("[dry-run] click {element_ref}"));
            }
            let transport = nissia_cdp::connect(port).await?;
            transport.send(&nissia_cdp::commands::PageEnable {}).await?;
            nissia_core::action::click::execute(&transport, element_ref).await?;
            mcp_action_with_snap(&transport, "click").await
        }
        "fill" => {
            let element_ref = args
                .get("ref")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: ref"))?;
            let value = args
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: value"))?;
            crate::validate::element_ref(element_ref)?;
            crate::validate::fill_value(value)?;
            if dry_run {
                return Ok(format!("[dry-run] fill {element_ref} {value:?}"));
            }
            let transport = nissia_cdp::connect(port).await?;
            transport.send(&nissia_cdp::commands::PageEnable {}).await?;
            nissia_core::action::fill::execute(&transport, element_ref, value).await?;
            mcp_action_with_snap(&transport, "fill").await
        }
        "type" => {
            let element_ref = args
                .get("ref")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: ref"))?;
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: text"))?;
            crate::validate::element_ref(element_ref)?;
            crate::validate::fill_value(text)?;
            if dry_run {
                return Ok(format!("[dry-run] type {element_ref} {text:?}"));
            }
            let transport = nissia_cdp::connect(port).await?;
            transport.send(&nissia_cdp::commands::PageEnable {}).await?;
            nissia_core::action::type_text::execute(&transport, element_ref, text).await?;
            mcp_action_with_snap(&transport, "type").await
        }
        "select" => {
            let element_ref = args
                .get("ref")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: ref"))?;
            let value = args
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: value"))?;
            crate::validate::element_ref(element_ref)?;
            if dry_run {
                return Ok(format!("[dry-run] select {element_ref} {value:?}"));
            }
            let transport = nissia_cdp::connect(port).await?;
            transport.send(&nissia_cdp::commands::PageEnable {}).await?;
            nissia_core::action::select::execute(&transport, element_ref, value).await?;
            mcp_action_with_snap(&transport, "select").await
        }
        "scroll" => {
            let direction = args
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("down");
            let amount = args.get("amount").and_then(|v| v.as_i64());
            if dry_run {
                return Ok(format!(
                    "[dry-run] scroll {direction} {}",
                    amount.unwrap_or(400)
                ));
            }
            let transport = nissia_cdp::connect(port).await?;
            transport.send(&nissia_cdp::commands::PageEnable {}).await?;
            nissia_core::action::scroll::execute(&transport, direction, amount).await?;
            mcp_action_with_snap(&transport, "scroll").await
        }
        "screenshot" => {
            let file = args.get("file").and_then(|v| v.as_str());
            let transport = nissia_cdp::connect(port).await?;
            let path = nissia_core::action::screenshot::execute(&transport, file).await?;
            Ok(json!({"status":"ok","action":"screenshot","path": path}).to_string())
        }
        "wait" => {
            let condition = args
                .get("condition")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: condition"))?;
            if condition != "navigation" && condition.parse::<u64>().is_err() {
                crate::validate::css_selector(condition)?;
            }
            let transport = nissia_cdp::connect(port).await?;
            let wait_condition = if condition == "navigation" {
                nissia_core::action::wait::WaitCondition::Navigation
            } else if let Ok(ms) = condition.parse::<u64>() {
                nissia_core::action::wait::WaitCondition::Timeout(ms)
            } else {
                nissia_core::action::wait::WaitCondition::Selector(condition)
            };
            nissia_core::action::wait::execute(&transport, wait_condition).await?;
            Ok(r#"{"status":"ok","action":"wait"}"#.to_string())
        }
        "eval" => {
            let expression = args
                .get("expression")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing required field: expression"))?;
            let transport = nissia_cdp::connect(port).await?;
            let result = transport
                .send(&nissia_cdp::commands::RuntimeEvaluate {
                    expression: expression.to_string(),
                    return_by_value: Some(true),
                    await_promise: Some(true),
                    context_id: None,
                })
                .await?;
            if let Some(exc) = result.exception_details {
                anyhow::bail!("JavaScript error: {:?}", exc);
            }
            let value = result.result.value.unwrap_or(serde_json::Value::Null);
            Ok(serde_json::to_string(&value)?)
        }
        "browser_launch" => {
            let headless = args
                .get("headless")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let background = args
                .get("background")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let pid_path = nissia_core::data_dir().join(format!("chrome-{port}.pid"));
            // Check if already running
            if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    let alive = std::process::Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if alive {
                        return Ok(
                            json!({"status":"already_running","port":port,"pid":pid}).to_string()
                        );
                    }
                    std::fs::remove_file(&pid_path).ok();
                }
            }
            let profile_name = args
                .get("profile")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let profile_dir = nissia_core::data_dir().join("profiles").join(profile_name);
            let browser = nissia_cdp::ManagedBrowser::launch(port, headless, &profile_dir)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let pid = browser.pid();
            std::fs::write(&pid_path, pid.to_string())?;
            if background {
                std::mem::forget(browser);
            }
            Ok(
                json!({"status":"launched","port":port,"pid":pid,"background":background})
                    .to_string(),
            )
        }
        "browser_stop" => {
            let pid_path = nissia_core::data_dir().join(format!("chrome-{port}.pid"));
            if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .ok();
                    std::fs::remove_file(&pid_path).ok();
                    return Ok(json!({"status":"stopped","port":port,"pid":pid}).to_string());
                }
            }
            Ok(json!({"status":"not_running","port":port}).to_string())
        }
        "browser_status" => {
            let pid_path = nissia_core::data_dir().join(format!("chrome-{port}.pid"));
            if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    let alive = std::process::Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if alive {
                        return Ok(json!({"running":true,"port":port,"pid":pid}).to_string());
                    }
                    std::fs::remove_file(&pid_path).ok();
                }
            }
            Ok(json!({"running":false,"port":port}).to_string())
        }
        _ => anyhow::bail!("Unknown tool: {name}"),
    }
}

/// Run the MCP server loop: read JSON-RPC from stdin, write responses to stdout.
pub async fn run(port: u16) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {e}")}
                });
                let mut out = stdout.lock();
                writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                out.flush()?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");

        // Notifications have no id and no response needed
        if id.is_null() && method.starts_with("notifications/") {
            continue;
        }

        let response = match method {
            "initialize" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION}
                    }
                })
            }
            "tools/list" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": tool_list()
                })
            }
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                match call_tool(tool_name, &args, port).await {
                    Ok(text) => {
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{"type": "text", "text": text}]
                            }
                        })
                    }
                    Err(e) => {
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{"type": "text", "text": format!("Error: {e}")}],
                                "isError": true
                            }
                        })
                    }
                }
            }
            "ping" => {
                json!({"jsonrpc": "2.0", "id": id, "result": {}})
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": format!("Method not found: {method}")}
                })
            }
        };

        let mut out = stdout.lock();
        writeln!(out, "{}", serde_json::to_string(&response)?)?;
        out.flush()?;
    }

    Ok(())
}
