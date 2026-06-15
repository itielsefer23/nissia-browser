//! `nissia search` — the tool's OWN web search. No API key required by default.
//!
//! Default (no key): real web results from Mojeek (HTTP) with a DuckDuckGo Instant
//! Answer fallback, so a search rarely comes back empty. No headless fingerprint, no
//! external LLM, no API key.
//!
//! Optional API backends for higher volume/quality: brave | serper | tavily. The key
//! and provider can come from env (NISSIA_SEARCH_API_KEY / NISSIA_SEARCH_PROVIDER) or
//! from a saved config file `<data_dir>/search.json` ({"provider":"brave","api_key":"..."}).
//! Paid providers are metered: usage is counted per month in `<data_dir>/usage.json` and
//! the remaining free quota is printed after each call.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use nissia_core::snap::EmulationOptions;

#[derive(serde::Serialize)]
struct Hit {
    title: String,
    url: String,
    snippet: String,
}

const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

/// Approx monthly free quota per provider (for the usage hint).
fn free_quota(provider: &str) -> Option<u64> {
    match provider {
        "brave" => Some(2000),
        "tavily" => Some(1000),
        "serper" => Some(2500), // one-time, but we still show the count
        _ => None,
    }
}

pub async fn run(
    port: u16,
    query: &str,
    n: usize,
    read_top: bool,
    fmt: &str,
    lang: &str,
    emu: &EmulationOptions,
) -> Result<()> {
    let client = reqwest::Client::new();
    let hits = fetch(&client, query, n).await?;

    if fmt == "json" {
        println!("{}", serde_json::to_string(&hits)?);
    } else if hits.is_empty() {
        eprintln!("(no results)");
    } else {
        for (i, h) in hits.iter().enumerate() {
            println!("{}. {}", i + 1, h.title);
            if !h.url.is_empty() {
                println!("   {}", h.url);
            }
            if !h.snippet.is_empty() {
                println!("   {}", h.snippet);
            }
        }
    }

    if read_top {
        if let Some(top) = hits.iter().find(|h| !h.url.is_empty()) {
            println!("\n--- TOP RESULT: {} ---", top.url);
            super::ensure_browser(port).await?;
            let transport = nissia_cdp::connect(port).await?;
            let r =
                nissia_core::read::execute(&transport, Some(&top.url), Some("main"), lang, 120, emu)
                    .await;
            match r {
                Ok(r) => println!("{}", r.output),
                Err(_) => {
                    let r =
                        nissia_core::read::execute(&transport, Some(&top.url), None, lang, 120, emu)
                            .await?;
                    println!("{}", r.output);
                }
            }
        }
    }

    Ok(())
}

/// Read provider + api_key from `<data_dir>/search.json` (if present).
fn load_config() -> (Option<String>, Option<String>) {
    let path = nissia_core::data_dir().join("search.json");
    if let Ok(txt) = std::fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            return (
                v["provider"].as_str().map(|s| s.to_string()),
                v["api_key"].as_str().map(|s| s.to_string()),
            );
        }
    }
    (None, None)
}

/// year-month "YYYY-MM" from the system clock (no extra deps; Hinnant's civil date).
fn current_month() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs / 86400 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y}-{m:02}")
}

/// Count one paid search for `provider` this month and print remaining free quota.
fn meter(provider: &str) {
    let path = nissia_core::data_dir().join("usage.json");
    let mut data: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let key = format!("{provider}-{}", current_month());
    let used = data[&key].as_u64().unwrap_or(0) + 1;
    data[&key] = json!(used);
    let _ = std::fs::write(&path, data.to_string());
    match free_quota(provider) {
        Some(cap) => eprintln!("({provider}: {used}/{cap} este mes)"),
        None => eprintln!("({provider}: {used} este mes)"),
    }
}

async fn fetch(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<Hit>> {
    let (cfg_provider, cfg_key) = load_config();
    let key = std::env::var("NISSIA_SEARCH_API_KEY").ok().or(cfg_key);
    let provider = std::env::var("NISSIA_SEARCH_PROVIDER")
        .ok()
        .or(cfg_provider)
        .unwrap_or_else(|| {
            if key.is_some() {
                "brave".to_string()
            } else {
                "auto".to_string()
            }
        });

    match provider.as_str() {
        // No-key default: real web results, with a reliable fallback.
        "auto" => {
            if let Ok(h) = mojeek(client, query, n).await {
                if !h.is_empty() {
                    return Ok(h);
                }
            }
            ddg_instant(client, query, n).await
        }
        "mojeek" => mojeek(client, query, n).await,
        "ddg" => ddg_instant(client, query, n).await,
        "brave" | "serper" | "tavily" => {
            let k = key.context("NISSIA_SEARCH_API_KEY (o search.json) requerido para este proveedor")?;
            let hits = match provider.as_str() {
                "brave" => brave(client, query, n, &k).await?,
                "serper" => serper(client, query, n, &k).await?,
                _ => tavily(client, query, n, &k).await?,
            };
            meter(&provider);
            Ok(hits)
        }
        other => bail!(
            "unknown NISSIA_SEARCH_PROVIDER '{other}' (use: auto | mojeek | ddg | brave | serper | tavily)"
        ),
    }
}

fn decode_entities(s: &str) -> String {
    let named = s
        .replace("&amp;", "&")
        .replace("&apos;", "'")
        .replace("&rsquo;", "'")
        .replace("&lsquo;", "'")
        .replace("&quot;", "\"")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&raquo;", ">>")
        .replace("&rsaquo;", ">")
        .replace("&mdash;", "-")
        .replace("&ndash;", "-")
        .replace("&hellip;", "...");
    let re = regex::Regex::new(r"&#(x?)([0-9A-Fa-f]+);").unwrap();
    re.replace_all(&named, |c: &regex::Captures| {
        let cp = if &c[1] == "x" {
            u32::from_str_radix(&c[2], 16).unwrap_or(0)
        } else {
            c[2].parse::<u32>().unwrap_or(0)
        };
        match cp {
            39 | 8216 | 8217 | 8218 | 8242 => "'".to_string(),
            34 | 8220 | 8221 | 8243 => "\"".to_string(),
            8208 | 8209 | 8211 | 8212 => "-".to_string(),
            160 => " ".to_string(),
            8230 => "...".to_string(),
            _ => char::from_u32(cp).map(|ch| ch.to_string()).unwrap_or_default(),
        }
    })
    .into_owned()
}

fn parse_mojeek(html: &str, n: usize) -> Vec<Hit> {
    let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
    let title_re =
        regex::Regex::new(r#"(?s)<a class="title"[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap();
    let snip_re = regex::Regex::new(r#"(?s)<p class="s">(.*?)</p>"#).unwrap();

    let clean = |s: &str| -> String {
        let t = tag_re.replace_all(s, "");
        decode_entities(t.trim())
    };

    let mut hits = Vec::new();
    for block in html.split("<li class=\"r").skip(1) {
        if let Some(c) = title_re.captures(block) {
            let url = decode_entities(c.get(1).map(|m| m.as_str()).unwrap_or("").trim());
            let title = clean(c.get(2).map(|m| m.as_str()).unwrap_or(""));
            let snippet = snip_re
                .captures(block)
                .map(|s| clean(s.get(1).map(|m| m.as_str()).unwrap_or("")))
                .unwrap_or_default();
            if !title.is_empty() {
                hits.push(Hit { title, url, snippet });
            }
            if hits.len() >= n {
                break;
            }
        }
    }
    hits
}

async fn mojeek(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<Hit>> {
    let url = reqwest::Url::parse_with_params("https://www.mojeek.com/search", &[("q", query)])?;
    let resp = client
        .get(url)
        .header("User-Agent", UA)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Mojeek request failed")?;
    if !resp.status().is_success() {
        bail!("Mojeek HTTP {}", resp.status());
    }
    let html = resp.text().await.context("Mojeek response read failed")?;
    Ok(parse_mojeek(&html, n))
}

fn collect_topics(arr: &[Value], out: &mut Vec<Hit>) {
    for it in arr {
        if let Some(sub) = it["Topics"].as_array() {
            collect_topics(sub, out);
        } else {
            let text = it["Text"].as_str().unwrap_or("");
            let url = it["FirstURL"].as_str().unwrap_or("");
            if !text.is_empty() {
                out.push(Hit {
                    title: text.chars().take(80).collect(),
                    url: url.to_string(),
                    snippet: text.to_string(),
                });
            }
        }
    }
}

async fn ddg_instant(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<Hit>> {
    let url = reqwest::Url::parse_with_params(
        "https://api.duckduckgo.com/",
        &[
            ("q", query),
            ("format", "json"),
            ("no_html", "1"),
            ("skip_disambig", "1"),
        ],
    )?;
    let v: Value = client
        .get(url)
        .header("User-Agent", "nissia-browser")
        .send()
        .await
        .context("DuckDuckGo request failed")?
        .json()
        .await
        .context("DuckDuckGo response was not JSON")?;

    let mut hits = Vec::new();
    let abstract_text = v["AbstractText"].as_str().unwrap_or("");
    if !abstract_text.is_empty() {
        hits.push(Hit {
            title: v["Heading"].as_str().unwrap_or("").to_string(),
            url: v["AbstractURL"].as_str().unwrap_or("").to_string(),
            snippet: abstract_text.to_string(),
        });
    }
    if let Some(rt) = v["RelatedTopics"].as_array() {
        collect_topics(rt, &mut hits);
    }
    hits.truncate(n);
    Ok(hits)
}

async fn brave(client: &reqwest::Client, query: &str, n: usize, key: &str) -> Result<Vec<Hit>> {
    let count = n.to_string();
    let url = reqwest::Url::parse_with_params(
        "https://api.search.brave.com/res/v1/web/search",
        &[("q", query), ("count", count.as_str())],
    )?;
    let v: Value = client
        .get(url)
        .header("X-Subscription-Token", key)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Brave request failed")?
        .json()
        .await
        .context("Brave response was not JSON")?;
    let mut hits = Vec::new();
    if let Some(arr) = v["web"]["results"].as_array() {
        for it in arr.iter().take(n) {
            hits.push(Hit {
                title: it["title"].as_str().unwrap_or("").to_string(),
                url: it["url"].as_str().unwrap_or("").to_string(),
                snippet: it["description"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(hits)
}

async fn serper(client: &reqwest::Client, query: &str, n: usize, key: &str) -> Result<Vec<Hit>> {
    let body = json!({ "q": query, "num": n });
    let v: Value = client
        .post("https://google.serper.dev/search")
        .header("X-API-KEY", key)
        .json(&body)
        .send()
        .await
        .context("Serper request failed")?
        .json()
        .await
        .context("Serper response was not JSON")?;
    let mut hits = Vec::new();
    if let Some(arr) = v["organic"].as_array() {
        for it in arr.iter().take(n) {
            hits.push(Hit {
                title: it["title"].as_str().unwrap_or("").to_string(),
                url: it["link"].as_str().unwrap_or("").to_string(),
                snippet: it["snippet"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(hits)
}

async fn tavily(client: &reqwest::Client, query: &str, n: usize, key: &str) -> Result<Vec<Hit>> {
    let body = json!({ "api_key": key, "query": query, "max_results": n });
    let v: Value = client
        .post("https://api.tavily.com/search")
        .json(&body)
        .send()
        .await
        .context("Tavily request failed")?
        .json()
        .await
        .context("Tavily response was not JSON")?;
    let mut hits = Vec::new();
    if let Some(arr) = v["results"].as_array() {
        for it in arr.iter().take(n) {
            hits.push(Hit {
                title: it["title"].as_str().unwrap_or("").to_string(),
                url: it["url"].as_str().unwrap_or("").to_string(),
                snippet: it["content"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(hits)
}
