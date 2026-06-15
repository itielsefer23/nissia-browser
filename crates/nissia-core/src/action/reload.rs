//! Reload action: refresh the current page, then wait for the DOM to be ready.
//!
//! This is the "human recovery" move: when a site errors, half-loads or hangs,
//! a person just reloads and tries again. `hard` bypasses the HTTP cache.

use nissia_cdp::commands::PageReload;
use nissia_cdp::CdpTransport;

pub async fn execute(
    transport: &CdpTransport,
    hard: bool,
) -> Result<(), nissia_cdp::CdpTransportError> {
    transport
        .send(&PageReload {
            ignore_cache: if hard { Some(true) } else { None },
        })
        .await?;
    // Wait for the reloaded document to become interactive (fast, adaptive).
    crate::snap::wait_dom_ready(transport, 8000).await;
    Ok(())
}
