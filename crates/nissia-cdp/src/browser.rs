//! Chrome browser discovery and lifecycle management.

use std::process::{Child, Command};
use std::time::Duration;

use crate::error::{CdpResult, CdpTransportError};
use crate::transport::CdpTransport;
use crate::types::BrowserVersion;

/// Discovers the browser-level CDP WebSocket URL.
pub async fn discover_ws_url(port: u16) -> CdpResult<String> {
    let response = http_get(port, "/json/version").await?;

    let version: BrowserVersion = serde_json::from_str(&response).map_err(|e| {
        CdpTransportError::BrowserNotFound(format!("Failed to parse /json/version: {e}"))
    })?;

    Ok(version.web_socket_debugger_url)
}

/// Discover the WebSocket URL for an existing page target (tab).
/// Falls back to creating a new tab if none exist.
pub async fn discover_page_ws_url(port: u16) -> CdpResult<String> {
    let body = http_get(port, "/json/list").await?;

    let targets: Vec<crate::types::TargetInfo> = serde_json::from_str(&body).map_err(|e| {
        CdpTransportError::BrowserNotFound(format!("Failed to parse /json/list: {e}"))
    })?;

    // Find a page target
    if let Some(target) = targets.iter().find(|t| t.target_type == "page") {
        if let Some(ws_url) = &target.web_socket_debugger_url {
            return Ok(ws_url.clone());
        }
    }

    // No page target found — create one
    let body = http_get(port, "/json/new?about:blank").await?;
    let target: crate::types::TargetInfo = serde_json::from_str(&body).map_err(|e| {
        CdpTransportError::BrowserNotFound(format!("Failed to parse /json/new response: {e}"))
    })?;

    target.web_socket_debugger_url.ok_or_else(|| {
        CdpTransportError::BrowserNotFound("New tab has no webSocketDebuggerUrl".into())
    })
}

/// Connect to an already-running Chrome instance (connects to a page target).
/// Also updates the heartbeat file for idle-timeout detection.
pub async fn connect(port: u16) -> CdpResult<CdpTransport> {
    let ws_url = discover_page_ws_url(port).await?;
    let transport = CdpTransport::connect(&ws_url).await?;

    // Update heartbeat for idle-timeout watchdog
    if let Some(dir) = dirs::data_local_dir() {
        let path = dir.join("nissia").join("heartbeat");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = std::fs::write(path, ts.to_string());
    }

    Ok(transport)
}

/// Simple HTTP GET helper for Chrome DevTools HTTP endpoints.
async fn http_get(port: u16, path: &str) -> CdpResult<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| {
                CdpTransportError::BrowserNotFound(format!(
                    "Cannot connect to Chrome on port {port}: {e}"
                ))
            })?;

        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.map_err(|e| {
            CdpTransportError::ConnectionFailed(format!("Failed to send request: {e}"))
        })?;

        let mut buf = Vec::with_capacity(4096);
        let mut tmp = [0u8; 4096];
        loop {
            match stream.read(&mut tmp).await {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    let text = String::from_utf8_lossy(&buf);
                    if let Some(body_start) = text.find("\r\n\r\n") {
                        let body = &text[body_start + 4..];
                        let trimmed = body.trim();
                        if (trimmed.ends_with('}') || trimmed.ends_with(']')) && !trimmed.is_empty()
                        {
                            break;
                        }
                    }
                }
                Err(e) => {
                    return Err(CdpTransportError::ConnectionFailed(format!(
                        "Failed to read response: {e}"
                    )));
                }
            }
        }

        let text = String::from_utf8_lossy(&buf).to_string();
        text.split("\r\n\r\n")
            .nth(1)
            .map(|s| s.to_string())
            .ok_or_else(|| {
                CdpTransportError::BrowserNotFound(format!(
                    "Invalid HTTP response from port {port}{path}"
                ))
            })
    })
    .await
    .map_err(|_| {
        CdpTransportError::BrowserNotFound(format!("Timeout connecting to Chrome on port {port}"))
    })?
}

/// A managed Chrome process.
pub struct ManagedBrowser {
    child: Child,
    port: u16,
}

impl ManagedBrowser {
    /// Launch Chrome with remote debugging enabled.
    /// `user_data_dir` controls the Chrome profile directory.
    /// Persistent profiles keep cookies/history between sessions, reducing bot detection.
    pub fn launch(port: u16, headless: bool, user_data_dir: &std::path::Path) -> CdpResult<Self> {
        let chrome_path = find_chrome()?;
        std::fs::create_dir_all(user_data_dir).ok();

        // Minimal flags only — avoid bot-detection signals.
        // Removed: --disable-background-networking, --disable-sync, --disable-translate,
        //          --metrics-recording-only, --safebrowsing-disable-auto-update
        // These flags are common bot fingerprints that trigger CAPTCHA on Amazon etc.
        let mut args = vec![
            format!("--remote-debugging-port={port}"),
            format!("--user-data-dir={}", user_data_dir.display()),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
        ];

        if headless {
            args.push("--headless=new".to_string());
        }

        let child = Command::new(&chrome_path)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                CdpTransportError::BrowserNotFound(format!(
                    "Failed to launch Chrome at {chrome_path}: {e}"
                ))
            })?;

        Ok(Self { child, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Connect to this browser's CDP endpoint.
    pub async fn connect(&self) -> CdpResult<CdpTransport> {
        // Give Chrome a moment to start up and listen on the port.
        let mut attempts = 0;
        loop {
            match connect(self.port).await {
                Ok(transport) => return Ok(transport),
                Err(_) if attempts < 20 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

impl Drop for ManagedBrowser {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Find Chrome/Chromium binary path.
fn find_chrome() -> CdpResult<String> {
    let candidates = if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        ]
    } else if cfg!(target_os = "linux") {
        vec![
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ]
    } else {
        vec![
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ]
    };

    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return Ok(candidate.to_string());
        }
    }

    // Try PATH for Linux
    if cfg!(target_os = "linux") {
        for candidate in &candidates {
            if Command::new("which")
                .arg(candidate)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Ok(candidate.to_string());
            }
        }
    }

    Err(CdpTransportError::BrowserNotFound(
        "Chrome/Chromium not found. Please install Chrome or set CHROME_PATH.".to_string(),
    ))
}
