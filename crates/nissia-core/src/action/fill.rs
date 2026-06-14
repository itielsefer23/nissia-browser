//! Fill action: focus element, clear value, set new value with proper event dispatch.

use nissia_cdp::commands::{CallArgument, DomResolveNode, RuntimeCallFunctionOn};
use nissia_cdp::CdpTransport;

use crate::element_map::ElementMap;

pub async fn execute(
    transport: &CdpTransport,
    ref_id: &str,
    value: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let map = ElementMap::load().map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to load element map: {e}"))
    })?;

    let entry = map
        .get(ref_id)
        .ok_or_else(|| nissia_cdp::CdpTransportError::CommandFailed {
            method: "fill".into(),
            code: -1,
            message: format!("Element {ref_id} not found. Run `nissia snap` first."),
        })?;

    // Resolve backend node to a remote object
    let resolved = transport
        .send(&DomResolveNode {
            node_id: None,
            backend_node_id: Some(entry.backend_node_id),
            object_group: Some("nissia".to_string()),
        })
        .await?;

    let object_id =
        resolved
            .object
            .object_id
            .ok_or_else(|| nissia_cdp::CdpTransportError::CommandFailed {
                method: "fill".into(),
                code: -1,
                message: "Could not resolve element to remote object".into(),
            })?;

    // Focus, clear, set value, and dispatch events
    let js = r#"
        function(newValue) {
            this.focus();
            this.value = '';
            this.value = newValue;
            this.dispatchEvent(new Event('input', { bubbles: true }));
            this.dispatchEvent(new Event('change', { bubbles: true }));
        }
    "#;

    transport
        .send(&RuntimeCallFunctionOn {
            function_declaration: js.to_string(),
            object_id: Some(object_id),
            arguments: Some(vec![CallArgument {
                value: Some(serde_json::Value::String(value.to_string())),
                object_id: None,
            }]),
            return_by_value: Some(true),
            await_promise: None,
        })
        .await?;

    Ok(())
}
