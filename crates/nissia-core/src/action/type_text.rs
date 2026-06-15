//! Type action: character-by-character key dispatch.

use nissia_cdp::commands::{
    DomResolveNode, InputDispatchKeyEvent, RuntimeCallFunctionOn, RuntimeEvaluate,
};
use nissia_cdp::CdpTransport;
use serde_json::Value;

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

    type_chars(transport, text).await
}

/// Type into the first element matching a CSS selector (focus + clear + type),
/// without needing an `@eN` snapshot ref. Makes `batch` flows self-contained for
/// form fields (search boxes, autocomplete inputs).
pub async fn execute_selector(
    transport: &CdpTransport,
    selector: &str,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let sel_json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    // Pick the VISIBLE match via an elementFromPoint hit-test (responsive sites
    // often render a hidden duplicate of the field earlier in the DOM; focusing
    // that one types into nowhere). Same reliable picker as `click --sel`.
    let focus_js = format!(
        "(function(){{\
var els=Array.from(document.querySelectorAll({sel_json}));\
if(!els.length)return 'notfound';\
var vw=window.innerWidth,vh=window.innerHeight;\
function hit(e){{var r=e.getBoundingClientRect();if(r.width===0&&r.height===0)return false;var cx=r.left+r.width/2,cy=r.top+r.height/2;if(cx<0||cx>vw||cy<0||cy>vh)return false;var t=document.elementFromPoint(cx,cy);return !!t&&(t===e||e.contains(t)||t.contains(e));}}\
var act=document.activeElement;\
var el=(act&&els.indexOf(act)>=0)?act:(els.find(hit)||els.find(function(e){{var r=e.getBoundingClientRect();return r.width>0&&r.height>0&&e.offsetParent!==null;}})||els[0]);\
if(el!==document.activeElement){{el.focus();}}try{{if(el.select)el.select();}}catch(e){{}}return 'ok';\
}})()"
    );
    let r = transport
        .send(&RuntimeEvaluate {
            expression: focus_js,
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    if let Some(Value::String(s)) = r.result.value {
        if s == "notfound" {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "type".into(),
                code: -1,
                message: format!("No element matches selector: {selector}"),
            });
        }
    }
    type_chars(transport, text).await
}

/// Human-paced character typing into the currently focused element.
async fn type_chars(
    transport: &CdpTransport,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
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
