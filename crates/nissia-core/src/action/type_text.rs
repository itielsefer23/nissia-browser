//! Type action: character-by-character key dispatch.

use nissia_cdp::commands::{DomResolveNode, InputDispatchKeyEvent, RuntimeCallFunctionOn};
use nissia_cdp::CdpTransport;

use crate::element_map::ElementMap;

pub async fn execute(
    transport: &CdpTransport,
    ref_id: &str,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let map = ElementMap::load().map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to load element map: {e}"))
    })?;

    let entry = map
        .get(ref_id)
        .ok_or_else(|| nissia_cdp::CdpTransportError::CommandFailed {
            method: "type".into(),
            code: -1,
            message: format!("Element {ref_id} not found. Run `nissia snap` first."),
        })?;

    // Focus the element
    let resolved = transport
        .send(&DomResolveNode {
            node_id: None,
            backend_node_id: Some(entry.backend_node_id),
            object_group: Some("nissia".to_string()),
        })
        .await?;

    if let Some(object_id) = &resolved.object.object_id {
        transport
            .send(&RuntimeCallFunctionOn {
                function_declaration: "function() { this.focus(); }".to_string(),
                object_id: Some(object_id.clone()),
                arguments: None,
                return_by_value: Some(true),
                await_promise: None,
            })
            .await?;
    }

    // Human-paced typing: small variable delay between keystrokes.
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0x9E37_79B9);
    for ch in text.chars() {
        let text_str = ch.to_string();

        transport
            .send(&InputDispatchKeyEvent {
                event_type: "keyDown".to_string(),
                key: Some(text_str.clone()),
                text: Some(text_str.clone()),
                unmodified_text: Some(text_str.clone()),
                code: None,
                windows_virtual_key_code: None,
                native_virtual_key_code: None,
            })
            .await?;

        transport
            .send(&InputDispatchKeyEvent {
                event_type: "keyUp".to_string(),
                key: Some(text_str),
                text: None,
                unmodified_text: None,
                code: None,
                windows_virtual_key_code: None,
                native_virtual_key_code: None,
            })
            .await?;

        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let delay = 40 + (seed >> 33) % 110; // ~40-150ms human typing
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }

    Ok(())
}
