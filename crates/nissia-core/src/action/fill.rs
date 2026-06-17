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

    // Focus, clear, set value, and dispatch events — but FIRST refuse payment-card /
    // CVV fields (entering card details is prohibited; using saved payment is fine).
    let js = r#"
        function(newValue) {
            var ac = (this.getAttribute('autocomplete')||'').toLowerCase();
            var hay = ((this.name||'')+' '+(this.id||'')+' '+(this.getAttribute('placeholder')||'')+' '+(this.getAttribute('aria-label')||'')).toLowerCase();
            if (/cc-number|cc-csc|cc-exp/.test(ac) ||
                /(card.?number|cardnum|creditcard|n[uú]mero.{0,6}(tarjeta|cart[aã]o)|\bcvv\b|\bcvc\b|security.?code|c[oó]digo.{0,6}segur|\bcsc\b)/.test(hay)) {
                return '__NZ_FINANCIAL__';
            }
            this.focus();
            this.value = '';
            this.value = newValue;
            this.dispatchEvent(new Event('input', { bubbles: true }));
            this.dispatchEvent(new Event('change', { bubbles: true }));
            return 'ok';
        }
    "#;

    let r = transport
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

    if let Some(serde_json::Value::String(s)) = r.result.value {
        if s == "__NZ_FINANCIAL__" {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "fill".into(),
                code: -1,
                message: "refusing to fill a payment-card / CVV field — entering card details is not allowed; ask the user to enter it themselves (use only payment methods already saved)".into(),
            });
        }
    }

    Ok(())
}
