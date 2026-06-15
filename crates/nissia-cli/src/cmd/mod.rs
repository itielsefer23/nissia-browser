pub mod action;
pub mod agent;
pub mod batch;
pub mod browser;
pub mod mcp;
pub mod read;
pub mod record;
pub mod replay;
pub mod schema;
pub mod search;
pub mod session;
pub mod snap;
pub mod update;

use anyhow::{bail, Context, Result};

/// Connect to the browser, launching a headless instance if none is running.
/// Headless is used on purpose: an isolated instance always exposes the debug
/// port, even when the user already has a normal Chrome open.
pub async fn ensure_browser(port: u16) -> Result<()> {
    if nissia_cdp::connect(port).await.is_ok() {
        return Ok(());
    }
    let exe = std::env::current_exe().context("cannot locate nissia executable")?;
    let _ = std::process::Command::new(exe)
        .args([
            "browser",
            "launch",
            "--headless",
            "--background",
            "--idle-timeout",
            "30",
            "--profile",
            "agent",
            "--port",
            &port.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to launch browser")?;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if nissia_cdp::connect(port).await.is_ok() {
            return Ok(());
        }
    }
    bail!("could not start the browser on port {port}")
}
