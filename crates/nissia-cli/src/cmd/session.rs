use anyhow::Result;

pub async fn run_save(port: u16, name: &str, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;

    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    let profile = nissia_core::session::capture_session(&transport, name).await?;

    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({
                "status": "ok",
                "action": "session_save",
                "name": name,
                "cookies": profile.cookies.len(),
            })
        );
    } else {
        println!(
            "Session '{}' saved ({} cookies)",
            name,
            profile.cookies.len()
        );
    }
    Ok(())
}

pub async fn run_load(port: u16, name: &str, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;

    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    nissia_core::session::restore_session(&transport, name).await?;

    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({"status": "ok", "action": "session_load", "name": name})
        );
    } else {
        println!("Session '{name}' restored");
    }
    Ok(())
}

pub fn run_list(fmt: &str) -> Result<()> {
    let sessions = nissia_core::session::SessionProfile::list()?;
    if fmt == "json" {
        println!("{}", serde_json::json!({"sessions": sessions}));
    } else if sessions.is_empty() {
        println!("No saved sessions");
    } else {
        for name in sessions {
            println!("{name}");
        }
    }
    Ok(())
}

pub fn run_delete(name: &str, fmt: &str) -> Result<()> {
    nissia_core::session::SessionProfile::delete(name)?;
    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({"status": "ok", "action": "session_delete", "name": name})
        );
    } else {
        println!("Session '{name}' deleted");
    }
    Ok(())
}
