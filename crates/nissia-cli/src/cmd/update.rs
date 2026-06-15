//! `nissia update [--check]` — tell the user when a newer nissia is published.
//!
//! Since nissia is distributed as an open-source skill that the author updates
//! often, every install should be able to notice a new version. This checks the
//! latest GitHub release tag and compares it to the running binary. The result is
//! cached for 24h (so the skill can call `--check` on startup cheaply, without
//! hitting the network or slowing things down every time).

use anyhow::Result;
use serde::{Deserialize, Serialize};

const REPO: &str = "itielsefer23/nissia-browser";
const CACHE_TTL_SECS: u64 = 24 * 60 * 60;

#[derive(Serialize, Deserialize, Default)]
struct UpdateCache {
    checked_at: u64,
    latest: String,
}

fn cache_path() -> std::path::PathBuf {
    nissia_core::data_dir().join("update_check.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Parse "v0.3.1" / "0.3.1-rc" into comparable numeric components.
fn parse_version(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// True if `latest` is strictly newer than `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (parse_version(latest), parse_version(current));
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

async fn fetch_latest_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let resp = client
        .get(&url)
        .header("User-Agent", "nissia-browser")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    let json: serde_json::Value = resp.json().await?;
    json.get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no tag_name in GitHub response"))
}

/// Return the latest version, using the 24h cache unless `force` is set.
async fn latest_version(force: bool) -> Option<String> {
    if !force {
        if let Ok(txt) = std::fs::read_to_string(cache_path()) {
            if let Ok(c) = serde_json::from_str::<UpdateCache>(&txt) {
                if !c.latest.is_empty() && now_secs().saturating_sub(c.checked_at) < CACHE_TTL_SECS {
                    return Some(c.latest);
                }
            }
        }
    }
    match fetch_latest_tag().await {
        Ok(tag) => {
            let cache = UpdateCache {
                checked_at: now_secs(),
                latest: tag.clone(),
            };
            if let Ok(txt) = serde_json::to_string(&cache) {
                let _ = std::fs::write(cache_path(), txt);
            }
            Some(tag)
        }
        Err(_) => None,
    }
}

pub async fn run(check: bool, fmt: &str) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    // `--check` uses the cache (cheap, for skill startup); a bare `update` forces
    // a fresh network check.
    let latest = latest_version(!check).await;

    match latest {
        Some(tag) if is_newer(&tag, current) => {
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({"update_available": true, "current": current, "latest": tag})
                );
            } else {
                println!("update available: {current} -> {tag}");
                if !check {
                    println!(
                        "  run the installer to update:\n  curl -fsSL https://raw.githubusercontent.com/{REPO}/master/install.sh | sh\n  (Windows PowerShell: irm https://raw.githubusercontent.com/{REPO}/master/install.ps1 | iex)\n  then re-copy the skill: see the repo README."
                    );
                }
            }
        }
        Some(tag) => {
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({"update_available": false, "current": current, "latest": tag})
                );
            } else if !check {
                println!("up to date ({current})");
            }
            // In --check mode and up to date: print nothing (so the skill stays quiet).
        }
        None => {
            if fmt == "json" {
                println!(
                    "{}",
                    serde_json::json!({"update_available": false, "current": current, "error": "could not reach GitHub"})
                );
            } else if !check {
                println!("could not check for updates (offline?) — current {current}");
            }
        }
    }
    Ok(())
}
