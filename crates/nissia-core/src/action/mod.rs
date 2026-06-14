pub mod click;
pub mod fill;
pub mod screenshot;
pub mod scroll;
pub mod select;
pub mod type_text;
pub mod wait;

use nissia_cdp::CdpTransport;
use std::time::Duration;

/// After a mutation action (click, fill, type, select, scroll), wait for the page
/// to settle and then take a fresh snap. Returns None if snap fails (the action
/// itself already succeeded).
pub async fn post_action_snap(
    transport: &CdpTransport,
    lang: &str,
    emu: &crate::snap::EmulationOptions,
) -> Option<crate::snap::SnapResult> {
    // Subscribe to events before checking for navigation
    let mut rx = transport.subscribe_events();

    // Check if a navigation happened within 500ms
    let navigated = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            match rx.recv().await {
                Ok(event) if event.method == "Page.frameNavigated" => return true,
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);

    if navigated {
        // Navigation detected — wait for page load (3s timeout)
        let _ = transport
            .wait_for_event("Page.loadEventFired", Duration::from_secs(3))
            .await;
    } else {
        // No navigation — wait 300ms for SPA DOM mutations to settle
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // Take a fresh snap of the current page
    crate::snap::execute(transport, None, None, lang, emu)
        .await
        .ok()
}
