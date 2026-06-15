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
    /// Launch a Chromium-based browser with remote debugging enabled.
    /// `user_data_dir` controls the profile directory (persistent profiles keep
    /// cookies/history between sessions, reducing bot detection). `prefer` picks
    /// the browser (chrome|edge|opera|brave|chromium); None = auto-detect.
    /// When `headless` is false the window opens maximized in a fresh window
    /// (the "Agente" visible mode), so the user can watch it work.
    pub fn launch(
        port: u16,
        headless: bool,
        user_data_dir: &std::path::Path,
        prefer: Option<&str>,
    ) -> CdpResult<Self> {
        let chrome_path = find_chrome(prefer)?;
        std::fs::create_dir_all(user_data_dir).ok();

        // Minimal flags only — avoid bot-detection signals.
        // Removed: --disable-background-networking, --disable-sync, --disable-translate,
        //          --metrics-recording-only, --safebrowsing-disable-auto-update
        // These flags are common bot fingerprints that trigger CAPTCHA on Amazon etc.
        //
        // IMPORTANT: we do NOT add --disable-blink-features=AutomationControlled in the
        // visible window. Recent Chrome shows a yellow "unsupported command-line flag"
        // infobar for it — a visible "you are automated" tell, the opposite of stealth.
        // We don't pass --enable-automation either, so navigator.webdriver stays false
        // on its own; for headless we add the flag (no UI there) plus a JS override at
        // connect time (see transport) as a belt-and-suspenders.
        let mut args = vec![
            format!("--remote-debugging-port={port}"),
            format!("--user-data-dir={}", user_data_dir.display()),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
        ];

        if headless {
            args.push("--headless=new".to_string());
            args.push("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".to_string());
            args.push("--disable-blink-features=AutomationControlled".to_string());
        } else {
            // Visible "Agente" window: maximized, fresh window, no translate nag, and
            // no "restore pages?" bubble after our taskkill stop (unclean exit).
            args.push("--start-maximized".to_string());
            args.push("--new-window".to_string());
            args.push("--disable-features=Translate,TranslateUI".to_string());
            args.push("--hide-crash-restore-bubble".to_string());
            args.push("about:blank".to_string());
        }

        let mut cmd = Command::new(&chrome_path);
        cmd.args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        // On Windows, detach the child so it does NOT inherit the caller's console
        // or pipe handles — otherwise a `browser launch --background` invoked by the
        // skill would keep the calling shell's stdout pipe open and appear to hang.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        }

        let child = cmd.spawn().map_err(|e| {
            CdpTransportError::BrowserNotFound(format!(
                "Failed to launch browser at {chrome_path}: {e}"
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

/// Per-OS candidate executable paths for each supported browser, in the order
/// (chrome, edge, opera, brave, chromium). Cross-platform: macOS / Linux / Windows.
fn browser_candidates() -> Vec<(&'static str, Vec<String>)> {
    if cfg!(target_os = "macos") {
        vec![
            ("chrome", vec!["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into()]),
            ("edge", vec!["/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into()]),
            ("opera", vec!["/Applications/Opera.app/Contents/MacOS/Opera".into()]),
            ("brave", vec!["/Applications/Brave Browser.app/Contents/MacOS/Brave Browser".into()]),
            ("chromium", vec!["/Applications/Chromium.app/Contents/MacOS/Chromium".into()]),
        ]
    } else if cfg!(target_os = "linux") {
        vec![
            ("chrome", vec!["google-chrome".into(), "google-chrome-stable".into()]),
            ("edge", vec!["microsoft-edge".into(), "microsoft-edge-stable".into()]),
            ("opera", vec!["opera".into()]),
            ("brave", vec!["brave-browser".into(), "brave".into()]),
            ("chromium", vec!["chromium".into(), "chromium-browser".into()]),
        ]
    } else {
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
        let pf86 = std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| r"C:\Program Files (x86)".into());
        vec![
            ("chrome", vec![
                format!(r"{pf}\Google\Chrome\Application\chrome.exe"),
                format!(r"{pf86}\Google\Chrome\Application\chrome.exe"),
                format!(r"{local}\Google\Chrome\Application\chrome.exe"),
            ]),
            ("edge", vec![
                format!(r"{pf86}\Microsoft\Edge\Application\msedge.exe"),
                format!(r"{pf}\Microsoft\Edge\Application\msedge.exe"),
            ]),
            ("opera", vec![
                format!(r"{local}\Programs\Opera\opera.exe"),
                format!(r"{local}\Programs\Opera\launcher.exe"),
                format!(r"{local}\Programs\Opera GX\opera.exe"),
                format!(r"{pf}\Opera\opera.exe"),
            ]),
            ("brave", vec![
                format!(r"{pf}\BraveSoftware\Brave-Browser\Application\brave.exe"),
                format!(r"{pf86}\BraveSoftware\Brave-Browser\Application\brave.exe"),
                format!(r"{local}\BraveSoftware\Brave-Browser\Application\brave.exe"),
            ]),
            ("chromium", vec![format!(r"{local}\Chromium\Application\chrome.exe")]),
        ]
    }
}

fn path_exists(c: &str) -> bool {
    if std::path::Path::new(c).exists() {
        return true;
    }
    // On Linux the candidate may be a bare command name on PATH.
    if cfg!(target_os = "linux") {
        return Command::new("which")
            .arg(c)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    false
}

/// Detect installed Chromium-based browsers on this machine.
/// Returns `(name, path)` for each one found (deduped, in canonical order).
pub fn detect_browsers() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, paths) in browser_candidates() {
        if let Some(p) = paths.iter().find(|c| path_exists(c)) {
            out.push((name.to_string(), p.clone()));
        }
    }
    out
}

/// Find a Chromium-based browser. Honors CHROME_PATH, then the `prefer` argument,
/// then the NISSIA_BROWSER env var (chrome|edge|opera|brave|chromium), else
/// auto-detects in canonical order. Cross-platform.
fn find_chrome(prefer: Option<&str>) -> CdpResult<String> {
    if let Ok(p) = std::env::var("CHROME_PATH") {
        if std::path::Path::new(&p).exists() {
            return Ok(p);
        }
    }
    let prefer = prefer
        .map(|s| s.to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::env::var("NISSIA_BROWSER").unwrap_or_default().to_lowercase());

    let candidates = browser_candidates();
    // Try the preferred browser first, then fall back to canonical order.
    if !prefer.is_empty() {
        if let Some((_, paths)) = candidates.iter().find(|(n, _)| *n == prefer) {
            if let Some(p) = paths.iter().find(|c| path_exists(c)) {
                return Ok(p.clone());
            }
        }
    }
    for (_, paths) in &candidates {
        if let Some(p) = paths.iter().find(|c| path_exists(c)) {
            return Ok(p.clone());
        }
    }

    Err(CdpTransportError::BrowserNotFound(
        "No Chromium-based browser found (Chrome/Edge/Opera/Brave/Chromium). Set CHROME_PATH."
            .to_string(),
    ))
}
