//! Capture and restore browser sessions (cookies, localStorage, sessionStorage).

use nissia_cdp::commands::{
    CookieParam, NetworkGetCookies, NetworkSetCookies, PageGetFrameTree, RuntimeEvaluate,
};
use nissia_cdp::CdpTransport;

use super::profile::SessionProfile;

/// Capture current browser session state.
pub async fn capture_session(
    transport: &CdpTransport,
    name: &str,
) -> Result<SessionProfile, nissia_cdp::CdpTransportError> {
    // Get cookies
    let cookies_resp = transport.send(&NetworkGetCookies { urls: None }).await?;

    // Get frame tree to discover origins
    let frame_tree = transport.send(&PageGetFrameTree {}).await?;
    let current_url = frame_tree.frame_tree.frame.url.clone();

    // Get localStorage
    let local_storage = eval_storage(transport, "localStorage").await?;

    // Get sessionStorage
    let session_storage = eval_storage(transport, "sessionStorage").await?;

    let profile = SessionProfile {
        version: 1,
        name: name.to_string(),
        created_at: chrono_now(),
        updated_at: chrono_now(),
        cookies: cookies_resp.cookies,
        local_storage,
        session_storage,
        last_url: current_url,
    };

    profile.save().map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to save session: {e}"))
    })?;
    Ok(profile)
}

/// Restore a saved browser session.
pub async fn restore_session(
    transport: &CdpTransport,
    name: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let profile = SessionProfile::load(name).map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!(
            "Failed to load session '{name}': {e}"
        ))
    })?;

    // Restore cookies
    let cookie_params: Vec<CookieParam> = profile
        .cookies
        .iter()
        .map(|c| CookieParam {
            name: c.name.clone(),
            value: c.value.clone(),
            domain: Some(c.domain.clone()),
            path: Some(c.path.clone()),
            expires: Some(c.expires),
            http_only: Some(c.http_only),
            secure: Some(c.secure),
            same_site: c.same_site.clone(),
        })
        .collect();

    if !cookie_params.is_empty() {
        transport
            .send(&NetworkSetCookies {
                cookies: cookie_params,
            })
            .await?;
    }

    // Navigate to the saved URL first so we can set storage
    transport
        .send(&nissia_cdp::commands::PageNavigate {
            url: profile.last_url.clone(),
        })
        .await?;

    transport
        .wait_for_event("Page.loadEventFired", std::time::Duration::from_secs(30))
        .await?;

    // Restore localStorage
    for (key, value) in &profile.local_storage {
        let js = format!(
            "localStorage.setItem({}, {})",
            serde_json::to_string(key).unwrap(),
            serde_json::to_string(value).unwrap()
        );
        transport
            .send(&RuntimeEvaluate {
                expression: js,
                return_by_value: Some(true),
                await_promise: None,
                context_id: None,
            })
            .await?;
    }

    // Restore sessionStorage
    for (key, value) in &profile.session_storage {
        let js = format!(
            "sessionStorage.setItem({}, {})",
            serde_json::to_string(key).unwrap(),
            serde_json::to_string(value).unwrap()
        );
        transport
            .send(&RuntimeEvaluate {
                expression: js,
                return_by_value: Some(true),
                await_promise: None,
                context_id: None,
            })
            .await?;
    }

    Ok(())
}

async fn eval_storage(
    transport: &CdpTransport,
    storage_name: &str,
) -> Result<std::collections::HashMap<String, String>, nissia_cdp::CdpTransportError> {
    let js = format!(
        r#"JSON.stringify(Object.fromEntries(
            Object.keys({storage_name}).map(k => [k, {storage_name}.getItem(k)])
        ))"#
    );

    let resp = transport
        .send(&RuntimeEvaluate {
            expression: js,
            return_by_value: Some(true),
            await_promise: None,
            context_id: None,
        })
        .await?;

    let json_str = resp
        .result
        .value
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "{}".to_string());

    let map: std::collections::HashMap<String, String> =
        serde_json::from_str(&json_str).unwrap_or_default();
    Ok(map)
}

fn chrono_now() -> String {
    // Simple ISO 8601 timestamp without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{now}")
}
