//! Click action: resolve @eN reference, compute center coordinates, dispatch mouse events.

use nissia_cdp::commands::{DomGetBoxModel, InputDispatchMouseEvent, RuntimeEvaluate};
use nissia_cdp::CdpTransport;
use serde_json::Value;

use crate::element_map::ElementMap;

pub async fn execute(
    transport: &CdpTransport,
    ref_id: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let map = ElementMap::load().map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to load element map: {e}"))
    })?;

    let entry = map
        .get(ref_id)
        .ok_or_else(|| nissia_cdp::CdpTransportError::CommandFailed {
            method: "click".into(),
            code: -1,
            message: format!("Element {ref_id} not found. Run `nissia snap` first."),
        })?;

    // Get box model to find center coordinates
    let box_model = transport
        .send(&DomGetBoxModel {
            node_id: None,
            backend_node_id: Some(entry.backend_node_id),
        })
        .await?;

    let (cx, cy) = compute_center(&box_model.model.content);
    dispatch_human_click(transport, cx, cy).await?;
    Ok(())
}

/// Click the first VISIBLE element matching a CSS selector with a real (trusted)
/// mouse click. Unlike `execute`, this does NOT need the element to be in the
/// snapshot index — essential for calendar day cells, custom grids and widgets
/// that the accessibility snapshot does not surface. Scrolls the element into
/// view first.
///
/// Coordinates come from `getBoundingClientRect` of the visible match (viewport
/// CSS pixels), which is exactly the space `Input.dispatchMouseEvent` expects.
/// We deliberately do NOT use `DOM.querySelectorAll` + `getBoxModel` here: many
/// responsive sites (e.g. Google Flights) render a hidden duplicate of a widget
/// earlier in the DOM, and `querySelectorAll` returns that hidden node, so the
/// click lands nowhere. Picking the visible match in JS avoids that trap.
pub async fn execute_selector(
    transport: &CdpTransport,
    selector: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let sel_json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());

    // Resolve the visible match, scroll it into view, and return its viewport
    // center. Among multiple matches we pick the first that is actually rendered
    // (non-zero box and an offsetParent), so hidden responsive duplicates are
    // skipped.
    let js = format!(
        "(function(){{\
var els=Array.from(document.querySelectorAll({sel_json}));\
if(!els.length)return 'notfound';\
var vw=window.innerWidth,vh=window.innerHeight;\
function center(e){{var b=e.getBoundingClientRect();return {{e:e,b:b,cx:b.left+b.width/2,cy:b.top+b.height/2}};}}\
function hittable(o){{if(o.b.width===0&&o.b.height===0)return false;if(o.cx<0||o.cx>vw||o.cy<0||o.cy>vh)return false;var t=document.elementFromPoint(o.cx,o.cy);return !!t&&(t===o.e||o.e.contains(t)||t.contains(o.e));}}\
var cand=els.map(center);\
var chosen=cand.find(hittable);\
if(!chosen){{var first=els[0];first.scrollIntoView({{block:'center',inline:'center'}});chosen=center(first);if(!hittable(chosen)&&chosen.b.width===0&&chosen.b.height===0)return 'hidden';}}\
return JSON.stringify([chosen.cx,chosen.cy]);\
}})()"
    );

    // First call scrolls + measures; do it, then re-measure after a short settle
    // so the coordinates reflect the post-scroll position.
    let _ = transport
        .send(&RuntimeEvaluate {
            expression: js.clone(),
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(220)).await;
    let r = transport
        .send(&RuntimeEvaluate {
            expression: js,
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;

    let (cx, cy) = match r.result.value {
        Some(Value::String(ref s)) if s == "notfound" => {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "click".into(),
                code: -1,
                message: format!("No element matches selector: {selector}"),
            });
        }
        Some(Value::String(ref s)) if s == "hidden" => {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "click".into(),
                code: -1,
                message: format!("Element for selector {selector} is not visible"),
            });
        }
        Some(Value::String(ref xystr)) => {
            let xy: Vec<f64> = serde_json::from_str(xystr).unwrap_or_default();
            if xy.len() == 2 {
                (xy[0], xy[1])
            } else {
                return Err(nissia_cdp::CdpTransportError::CommandFailed {
                    method: "click".into(),
                    code: -1,
                    message: format!("Element for selector {selector} has no usable box"),
                });
            }
        }
        _ => {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "click".into(),
                code: -1,
                message: format!("Could not locate selector: {selector}"),
            });
        }
    };

    if std::env::var("NISSIA_DEBUG_CLICK").is_ok() {
        eprintln!("[debug] selector visible center = ({cx:.1}, {cy:.1})");
    }
    dispatch_human_click(transport, cx, cy).await?;
    Ok(())
}

/// Dispatch a trusted left click at viewport coordinates with small human-paced
/// pauses (move, settle, press, release).
async fn dispatch_human_click(
    transport: &CdpTransport,
    x: f64,
    y: f64,
) -> Result<(), nissia_cdp::CdpTransportError> {
    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mouseMoved".to_string(),
            x,
            y,
            button: None,
            click_count: None,
        })
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(90)).await;

    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mousePressed".to_string(),
            x,
            y,
            button: Some("left".to_string()),
            click_count: Some(1),
        })
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;

    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mouseReleased".to_string(),
            x,
            y,
            button: Some("left".to_string()),
            click_count: Some(1),
        })
        .await?;

    Ok(())
}

/// Compute center point from a content quad (8 values: 4 x,y pairs).
fn compute_center(quad: &[f64]) -> (f64, f64) {
    if quad.len() >= 8 {
        let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
        let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
        (x, y)
    } else if quad.len() >= 4 {
        // bounds: [x, y, w, h]
        (quad[0] + quad[2] / 2.0, quad[1] + quad[3] / 2.0)
    } else {
        (0.0, 0.0)
    }
}
