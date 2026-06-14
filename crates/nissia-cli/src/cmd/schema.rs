//! Schema introspection — returns JSON Schema for nissia commands.
//! Lets agents discover exact parameter types, required fields, and output shapes
//! at runtime without parsing static documentation.

use serde_json::{json, Value};

/// Return the full schema registry or the schema for a specific command.
pub fn run(command: Option<&str>, fmt: &str) {
    let schema = match command {
        Some(name) => match command_schema(name) {
            Some(s) => s,
            None => {
                if fmt == "json" {
                    eprintln!(
                        "{}",
                        serde_json::json!({"error":{"code":"NOT_FOUND","message":format!("Unknown command: {name}")}})
                    );
                } else {
                    eprintln!("error: unknown command '{name}'. Run `nissia schema` for full list.");
                }
                std::process::exit(1);
            }
        },
        None => full_schema(),
    };

    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}

fn full_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "nissia CLI schema",
        "version": env!("CARGO_PKG_VERSION"),
        "commands": {
            "snap":       command_schema("snap").unwrap(),
            "read":       command_schema("read").unwrap(),
            "click":      command_schema("click").unwrap(),
            "fill":       command_schema("fill").unwrap(),
            "type":       command_schema("type").unwrap(),
            "select":     command_schema("select").unwrap(),
            "scroll":     command_schema("scroll").unwrap(),
            "screenshot": command_schema("screenshot").unwrap(),
            "wait":       command_schema("wait").unwrap(),
            "session":    command_schema("session").unwrap(),
            "record":     command_schema("record").unwrap(),
            "replay":     command_schema("replay").unwrap(),
            "browser":    command_schema("browser").unwrap(),
        }
    })
}

fn command_schema(name: &str) -> Option<Value> {
    let schema = match name {
        "read" => json!({
            "description": "Read visible text content as structured markdown (headings, paragraphs, lists, tables). Use when you need to understand page content — not just interactable elements.",
            "input": {
                "type": "object",
                "properties": {
                    "url":       {"type": "string", "description": "URL to navigate to (optional if already on page)"},
                    "focus":     {"type": "string", "description": "CSS selector to limit scope (e.g. 'main', '#content')"},
                    "lang":      {"type": "string", "description": "Accept-Language header (e.g. en-US, ko)", "default": "en-US"},
                    "max_lines": {"type": "integer", "description": "Maximum output lines", "default": 200},
                    "output":    {"type": "string", "enum": ["text", "json"], "default": "text"}
                }
            },
            "output": {
                "text": "Compact markdown: # headings, - lists, | table rows, plain paragraphs",
                "json": {
                    "type": "object",
                    "properties": {
                        "content":    {"type": "string"},
                        "line_count": {"type": "integer"}
                    }
                }
            }
        }),
        "snap" => json!({
            "description": "Extract interactable elements from the current page as @eN refs.",
            "input": {
                "type": "object",
                "properties": {
                    "url":   {"type": "string", "description": "URL to navigate to (optional if already on page)"},
                    "focus": {"type": "string", "description": "CSS selector to limit scope (e.g. 'main', '#content')"},
                    "lang":  {"type": "string", "description": "Accept-Language header (e.g. en-US, ko)", "default": "en-US"},
                    "output":{"type": "string", "enum": ["text", "json", "ndjson"], "default": "text"}
                }
            },
            "output": {
                "text": "One element per line: @eN [role] \"label\" attr=val ...",
                "json": {
                    "type": "object",
                    "properties": {
                        "count":    {"type": "integer"},
                        "elements": {
                            "type": "object",
                            "additionalProperties": {
                                "type": "object",
                                "properties": {
                                    "backend_node_id":  {"type": "integer"},
                                    "role":             {"type": "string"},
                                    "name":             {"type": "string"},
                                    "tag":              {"type": "string"},
                                    "selector_hint":    {"type": "string"},
                                    "attributes":       {"type": "object"}
                                }
                            }
                        }
                    }
                },
                "ndjson": "One JSON object per element (stream-friendly)"
            },
            "notes": [
                "Always snap before acting — element refs are invalidated after navigation",
                "Refs are stable within a single page load (backendNodeId-based)"
            ]
        }),
        "click" => json!({
            "description": "Click an element by @eN reference.",
            "input": {
                "type": "object",
                "required": ["ref"],
                "properties": {
                    "ref":     {"type": "string", "pattern": "^@e[0-9]+$", "description": "@eN reference from snap"},
                    "dry_run": {"type": "boolean", "default": false}
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "click"}}},
                "text": "ok"
            }
        }),
        "fill" => json!({
            "description": "Set an input field's value (clears existing). Use for <input>, <textarea>.",
            "input": {
                "type": "object",
                "required": ["ref", "value"],
                "properties": {
                    "ref":     {"type": "string", "pattern": "^@e[0-9]+$"},
                    "value":   {"type": "string", "maxLength": 10000},
                    "dry_run": {"type": "boolean", "default": false}
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "fill"}}},
                "text": "ok"
            }
        }),
        "type" => json!({
            "description": "Type text character by character. Use for autocomplete/search inputs.",
            "input": {
                "type": "object",
                "required": ["ref", "text"],
                "properties": {
                    "ref":     {"type": "string", "pattern": "^@e[0-9]+$"},
                    "text":    {"type": "string", "maxLength": 10000},
                    "dry_run": {"type": "boolean", "default": false}
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "type"}}},
                "text": "ok"
            }
        }),
        "select" => json!({
            "description": "Select an option in a <select> dropdown by value.",
            "input": {
                "type": "object",
                "required": ["ref", "value"],
                "properties": {
                    "ref":     {"type": "string", "pattern": "^@e[0-9]+$"},
                    "value":   {"type": "string", "description": "Option value attribute"},
                    "dry_run": {"type": "boolean", "default": false}
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "select"}}},
                "text": "ok"
            }
        }),
        "scroll" => json!({
            "description": "Scroll the page in a direction.",
            "input": {
                "type": "object",
                "properties": {
                    "direction": {"type": "string", "enum": ["up", "down", "left", "right"], "default": "down"},
                    "amount":    {"type": "integer", "description": "Pixels", "default": 400},
                    "dry_run":   {"type": "boolean", "default": false}
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "scroll"}}},
                "text": "ok"
            }
        }),
        "screenshot" => json!({
            "description": "Capture a PNG screenshot of the current page.",
            "input": {
                "type": "object",
                "properties": {
                    "file": {"type": "string", "description": "Output file path (omit for auto-named file)"}
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "screenshot"}, "path": {"type": "string"}}},
                "text": "Saved to <path>"
            }
        }),
        "wait" => json!({
            "description": "Wait for navigation, a CSS selector to appear, or a timeout.",
            "input": {
                "type": "object",
                "required": ["condition"],
                "properties": {
                    "condition": {
                        "type": "string",
                        "description": "'navigation' | CSS selector | milliseconds as string (e.g. '2000')"
                    }
                }
            },
            "output": {
                "json": {"type": "object", "properties": {"status": {"const": "ok"}, "action": {"const": "wait"}}},
                "text": "ok"
            }
        }),
        "session" => json!({
            "description": "Persist and restore browser session (cookies + localStorage + sessionStorage).",
            "subcommands": {
                "save":   {"required": ["name"], "description": "Save current session"},
                "load":   {"required": ["name"], "description": "Restore saved session"},
                "list":   {"description": "List saved sessions"},
                "delete": {"required": ["name"], "description": "Delete a session"}
            },
            "input": {
                "name": {"type": "string", "description": "Session name"}
            }
        }),
        "record" => json!({
            "description": "Record a sequence of commands for replay.",
            "subcommands": {
                "start":  {"description": "Start recording (optional name)"},
                "stop":   {"description": "Stop recording and save"},
                "list":   {"description": "List recorded workflows"},
                "delete": {"required": ["name"], "description": "Delete a workflow"}
            }
        }),
        "replay" => json!({
            "description": "Replay a recorded workflow without LLM calls.",
            "input": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name":  {"type": "string"},
                    "speed": {"type": "number", "description": "Speed multiplier (0=instant, 1.0=original)", "default": 0}
                }
            }
        }),
        "browser" => json!({
            "description": "Manage the Chrome browser lifecycle.",
            "subcommands": {
                "launch": {
                    "description": "Launch Chrome with remote debugging",
                    "input": {
                        "headless":    {"type": "boolean", "default": false},
                        "background":  {"type": "boolean", "default": false, "description": "Detach immediately (use for agent workflows)"}
                    }
                },
                "stop":   {"description": "Stop the running Chrome instance"},
                "status": {"description": "Check if Chrome is running"}
            },
            "output": {
                "json": {
                    "launch": {"status": "launched|already_running", "port": "integer", "pid": "integer"},
                    "stop":   {"status": "stopped|not_running", "port": "integer"},
                    "status": {"running": "boolean", "port": "integer", "pid": "integer|null"}
                }
            }
        }),
        _ => return None,
    };
    Some(schema)
}
