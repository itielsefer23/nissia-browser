//! History navigation: go back / forward, like the browser's back button.
//!
//! Lets the agent return to the previous page (e.g. a search-results page) to pick
//! another link, instead of re-typing the whole search from scratch. The previous
//! page is usually served from the back/forward cache, so this is instant and free
//! (no re-load, no re-type) — the human, token-cheap way to open several results.

use nissia_cdp::commands::RuntimeEvaluate;
use nissia_cdp::CdpTransport;

pub async fn go(
    transport: &CdpTransport,
    forward: bool,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let js = if forward {
        "history.forward()"
    } else {
        "history.back()"
    };
    transport
        .send(&RuntimeEvaluate {
            expression: js.to_string(),
            return_by_value: Some(true),
            await_promise: None,
            context_id: None,
        })
        .await?;
    // Brief settle, then wait for the (usually cached) page to be ready again.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    crate::snap::wait_dom_ready(transport, 6000).await;
    Ok(())
}
