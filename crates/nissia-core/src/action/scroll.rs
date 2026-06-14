//! Scroll action.

use nissia_cdp::commands::RuntimeEvaluate;
use nissia_cdp::CdpTransport;

pub async fn execute(
    transport: &CdpTransport,
    direction: &str,
    amount: Option<i64>,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let pixels = amount.unwrap_or(500);
    let (dx, dy) = match direction {
        "down" => (0, pixels),
        "up" => (0, -pixels),
        "right" => (pixels, 0),
        "left" => (-pixels, 0),
        _ => (0, pixels),
    };

    transport
        .send(&RuntimeEvaluate {
            expression: format!("window.scrollBy({dx}, {dy})"),
            return_by_value: Some(true),
            await_promise: None,
            context_id: None,
        })
        .await?;

    Ok(())
}
