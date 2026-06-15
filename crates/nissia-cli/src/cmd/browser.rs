use anyhow::Result;
use std::path::PathBuf;

fn pid_file(port: u16) -> PathBuf {
    nissia_core::data_dir().join(format!("chrome-{port}.pid"))
}

#[allow(clippy::too_many_arguments)]
pub fn run_launch(
    port: u16,
    headless: bool,
    background: bool,
    profile: Option<&str>,
    browser: Option<&str>,
    idle_timeout: Option<u32>,
    fmt: &str,
) -> Result<()> {
    // Check if already running
    if let Some(pid) = read_pid(port) {
        if is_process_alive(pid) {
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "already_running", "port": port, "pid": pid})
                );
            } else {
                eprintln!("Chrome already running on port {} (pid {})", port, pid);
            }
            return Ok(());
        }
        // Stale pid file
        std::fs::remove_file(pid_file(port)).ok();
    }

    // Persistent profile directory — keeps cookies/state between sessions
    let profile_name = profile.unwrap_or("default");
    let profile_dir = nissia_core::data_dir().join("profiles").join(profile_name);
    let managed = nissia_cdp::ManagedBrowser::launch(port, headless, &profile_dir, browser)?;
    let pid = managed.pid();

    // Save PID to file
    std::fs::write(pid_file(port), pid.to_string())?;

    // Initialize heartbeat so idle-timeout watchdog has a starting point
    let heartbeat_path = nissia_core::data_dir().join("heartbeat");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    std::fs::write(&heartbeat_path, ts.to_string()).ok();

    // Spawn idle-timeout watchdog if requested
    if let Some(minutes) = idle_timeout {
        spawn_idle_watchdog(pid, &heartbeat_path, minutes);
    }

    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({"status": "launched", "port": port, "pid": pid, "background": background, "idle_timeout_min": idle_timeout})
        );
    } else {
        let timeout_msg = idle_timeout
            .map(|m| format!(" (idle timeout: {m}m)"))
            .unwrap_or_default();
        println!("Chrome launched on port {port} (pid {pid}){timeout_msg}");
    }

    if background {
        // Detach — let the browser run independently
        std::mem::forget(managed);
    } else {
        println!("Press Ctrl+C to stop");
        std::thread::park();
    }

    Ok(())
}

/// Spawn a background process that kills Chrome after `minutes` of inactivity.
fn spawn_idle_watchdog(chrome_pid: u32, heartbeat_path: &std::path::Path, minutes: u32) {
    let timeout_secs = minutes as u64 * 60;
    let hb = heartbeat_path.display().to_string();

    // Self-contained shell watchdog — survives the parent process exiting.
    let script = format!(
        r#"while true; do
  sleep 60
  if ! kill -0 {chrome_pid} 2>/dev/null; then exit 0; fi
  if [ -f "{hb}" ]; then
    last=$(cat "{hb}" 2>/dev/null || echo 0)
    now=$(date +%s)
    idle=$((now - last))
    if [ "$idle" -ge {timeout_secs} ]; then
      kill {chrome_pid} 2>/dev/null
      rm -f "{hb}"
      exit 0
    fi
  fi
done"#
    );

    std::process::Command::new("sh")
        .args(["-c", &script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
        .ok();
}

pub fn run_stop(port: u16, fmt: &str) -> Result<()> {
    if let Some(pid) = read_pid(port) {
        if is_process_alive(pid) {
            kill_process(pid);
            std::fs::remove_file(pid_file(port)).ok();
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "stopped", "port": port, "pid": pid})
                );
            } else {
                println!("Chrome stopped (pid {})", pid);
            }
        } else {
            std::fs::remove_file(pid_file(port)).ok();
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({"status": "not_running", "port": port})
                );
            } else {
                println!("Chrome not running on port {}", port);
            }
        }
    } else if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({"status": "not_running", "port": port})
        );
    } else {
        println!("Chrome not running on port {}", port);
    }
    Ok(())
}

pub fn run_status(port: u16, fmt: &str) -> Result<()> {
    let running = read_pid(port).is_some_and(is_process_alive);
    let pid = read_pid(port);

    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({
                "port": port,
                "running": running,
                "pid": pid,
            })
        );
    } else if running {
        println!("Chrome running on port {} (pid {})", port, pid.unwrap());
    } else {
        println!("Chrome not running on port {}", port);
    }
    Ok(())
}

fn read_pid(port: u16) -> Option<u32> {
    std::fs::read_to_string(pid_file(port))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        // tasklist prints "INFO: No tasks..." when the PID is not running.
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout);
            return s.lines().any(|l| l.contains(&format!("\"{pid}\"")));
        }
        return false;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn kill_process(pid: u32) {
    #[cfg(windows)]
    {
        // /T also kills child processes (Chrome spawns several).
        std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
    }
}

/// Bring the visible browser tab/window to the front (cross-platform via CDP
/// `Page.bringToFront`). Used by the "Agente" mode so the user actually sees the
/// page update, and to recover focus after long command sequences.
pub async fn run_focus(port: u16, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;
    transport
        .send(&nissia_cdp::commands::PageBringToFront {})
        .await?;
    if fmt == "json" {
        println!("{}", serde_json::json!({"status": "ok", "action": "focus"}));
    } else {
        println!("ok");
    }
    Ok(())
}

/// List Chromium-based browsers installed on this machine (so the skill can ask
/// the user which one to use). Cross-platform.
pub fn run_detect(fmt: &str) -> Result<()> {
    let found = nissia_cdp::detect_browsers();
    if fmt == "json" {
        let arr: Vec<serde_json::Value> = found
            .iter()
            .map(|(n, p)| serde_json::json!({"name": n, "path": p}))
            .collect();
        println!("{}", serde_json::json!({"browsers": arr}));
    } else if found.is_empty() {
        println!("(no Chromium-based browser found — install Chrome, Edge, Brave or Opera)");
    } else {
        for (n, p) in &found {
            println!("{n}\t{p}");
        }
    }
    Ok(())
}
