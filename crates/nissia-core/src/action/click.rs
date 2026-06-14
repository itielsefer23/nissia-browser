//! Click action: resolve @eN reference, compute center coordinates, dispatch mouse events.

use nissia_cdp::commands::{DomGetBoxModel, InputDispatchMouseEvent};
use nissia_cdp::CdpTransport;

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

    // Mouse move
    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mouseMoved".to_string(),
            x: cx,
            y: cy,
            button: None,
            click_count: None,
        })
        .await?;

    // Mouse press
    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mousePressed".to_string(),
            x: cx,
            y: cy,
            button: Some("left".to_string()),
            click_count: Some(1),
        })
        .await?;

    // Mouse release
    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mouseReleased".to_string(),
            x: cx,
            y: cy,
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
