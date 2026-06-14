//! `nissia search` — cheap web search over plain HTTP (no browser, no headless
//! fingerprint, no SERP dumped into context).
//!
//! Backends (auto-selected):
//!   * default, NO key:  DuckDuckGo Instant Answer API — always works, great for
//!                        quick facts / disambiguation (not full web ranking).
//!   * with an API key:  real web results. Set NISSIA_SEARCH_API_KEY and optionally
//!                        NISSIA_SEARCH_PROVIDER = brave | serper | tavily.
//!
//! For deep, real web research prefer `nissia agent "<goal>"` (it navigates with a
//! cheap internal model and returns only the answer).

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use nissia_core::snap::EmulationOptions;

#[derive(serde::Serialize)]
struct Hit {
    title: String,
    url: String,
    snippet: String,
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
            super::agent::ensure_browser(port).await?;
            let transport = nissia_cdp::connect(port).await?;
            let r = nissia_core::read::execute(
                &transport,
                Some(&top.url),
                Some("main"),
                lang,
                120,
                emu,
            )
            .await;
            match r {
                Ok(r) => println!("{}", r.output),
                Err(_) => {
                    let r = nissia_core::read::execute(
                        &transport,
                        Some(&top.url),
                        None,
                        lang,
                        120,
                        emu,
                    )
                    .await?;
                    println!("{}", r.output);
                }
            }
        }
    }

    Ok(())
}

async fn fetch(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<Hit>> {
    let key = std::env::var("NISSIA_SEARCH_API_KEY").ok();
    let provider = std::env::var("NISSIA_SEARCH_PROVIDER").unwrap_or_else(|_| {
        if key.is_some() {
            "brave".to_string()
        } else {
            "ddg".to_string()
        }
    });

    match provider.as_str() {
        "ddg" => ddg_instant(client, query, n).await,
        "brave" => {
            let k = key.context("NISSIA_SEARCH_API_KEY required for brave")?;
            brave(client, query, n, &k).await
        }
        "serper" => {
            let k = key.context("NISSIA_SEARCH_API_KEY required for serper")?;
            serper(client, query, n, &k).await
        }
        "tavily" => {
            let k = key.context("NISSIA_SEARCH_API_KEY required for tavily")?;
            tavily(client, query, n, &k).await
        }
        other => bail!("unknown NISSIA_SEARCH_PROVIDER '{other}' (use: ddg | brave | serper | tavily)"),
    }
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
