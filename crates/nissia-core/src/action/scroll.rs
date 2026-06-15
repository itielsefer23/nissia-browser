//! Scroll action (human-paced: small steps with jitter, less bot-like).

use nissia_cdp::commands::RuntimeEvaluate;
use nissia_cdp::CdpTransport;

pub async fn execute(
    transport: &CdpTransport,
    direction: &str,
    amount: Option<i64>,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let pixels = amount.unwrap_or(500).abs().max(1);
    let (ax, ay): (i64, i64) = match direction {
        "down" => (0, 1),
        "up" => (0, -1),
        "right" => (1, 0),
        "left" => (-1, 0),
        _ => (0, 1),
    };

    // Break the scroll into small steps with variable pauses, so it looks like a
    // human flicking the wheel rather than one instant programmatic jump.
    let steps: i64 = 8.min(pixels).max(1);
    let per = pixels / steps;
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0x9E37_79B9);

    for i in 0..steps {
        let chunk = if i == steps - 1 { pixels - per * (steps - 1) } else { per };
        let dx = ax * chunk;
        let dy = ay * chunk;
        transport
            .send(&RuntimeEvaluate {
                expression: format!("window.scrollBy({dx}, {dy})"),
                return_by_value: Some(true),
                await_promise: None,
                context_id: None,
            })
            .await?;
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let delay = 45 + (seed >> 33) % 65; // ~45-110ms jitter
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }

    Ok(())
}
