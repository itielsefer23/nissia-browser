//! Key action: press a single key as a real (trusted) keyboard event.
//!
//! Lets the agent submit searches (enter), move between fields (tab) or pick an
//! autocomplete suggestion (arrowdown then enter), exactly like a human would.

use nissia_cdp::commands::InputDispatchKeyEvent;
use nissia_cdp::CdpTransport;

/// Resolve a friendly key name to (key, code, virtualKeyCode, optional text).
fn map_key(key_name: &str) -> Option<(&'static str, &'static str, i64, Option<&'static str>)> {
    let m = match key_name.to_lowercase().as_str() {
        "enter" | "return" => ("Enter", "Enter", 13, None),
        "tab" => ("Tab", "Tab", 9, None),
        "escape" | "esc" => ("Escape", "Escape", 27, None),
        "backspace" => ("Backspace", "Backspace", 8, None),
        "delete" | "del" => ("Delete", "Delete", 46, None),
        "arrowdown" | "down" => ("ArrowDown", "ArrowDown", 40, None),
        "arrowup" | "up" => ("ArrowUp", "ArrowUp", 38, None),
        "arrowleft" | "left" => ("ArrowLeft", "ArrowLeft", 37, None),
        "arrowright" | "right" => ("ArrowRight", "ArrowRight", 39, None),
        "space" => (" ", "Space", 32, Some(" ")),
        "pagedown" => ("PageDown", "PageDown", 34, None),
        "pageup" => ("PageUp", "PageUp", 33, None),
        "home" => ("Home", "Home", 36, None),
        "end" => ("End", "End", 35, None),
        _ => return None,
    };
    Some(m)
}

pub async fn execute(
    transport: &CdpTransport,
    key_name: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let (key, code, vk, text) = map_key(key_name).ok_or_else(|| {
        nissia_cdp::CdpTransportError::CommandFailed {
            method: "key".into(),
            code: -1,
            message: format!(
                "unknown key {key_name:?} (enter|tab|escape|backspace|delete|arrowup|arrowdown|arrowleft|arrowright|space|pageup|pagedown|home|end)"
            ),
        }
    })?;

    let down = InputDispatchKeyEvent {
        event_type: "keyDown".to_string(),
        key: Some(key.to_string()),
        text: text.map(|t| t.to_string()),
        unmodified_text: text.map(|t| t.to_string()),
        code: Some(code.to_string()),
        windows_virtual_key_code: Some(vk),
        native_virtual_key_code: Some(vk),
    };
    transport.send(&down).await?;
    // Small human-paced gap between press and release.
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    let up = InputDispatchKeyEvent {
        event_type: "keyUp".to_string(),
        key: Some(key.to_string()),
        text: None,
        unmodified_text: None,
        code: Some(code.to_string()),
        windows_virtual_key_code: Some(vk),
        native_virtual_key_code: Some(vk),
    };
    transport.send(&up).await?;
    Ok(())
}
