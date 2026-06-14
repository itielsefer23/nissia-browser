//! `nissia search` — the tool's OWN web search. No API key required by default.
//!
//! Default backend: Mojeek over plain HTTP (no key, scrape-tolerant, no headless
//! fingerprint). So the calling agent (e.g. Claude Code) never has to navigate to
//! Google itself, and never needs an external LLM/API to search.
//!
//! Optional API backends for higher volume/quality: set NISSIA_SEARCH_API_KEY and
//! NISSIA_SEARCH_PROVIDER = brave | serper | tavily.

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

async fn fetch(client: &reqwest::Client, query: &str, n: usize) -> Result<Vec<Hit>> {
    let key = std::env::var("NISSIA_SEARCH_API_KEY").ok();
    let provider = std::env::var("NISSIA_SEARCH_PROVIDER").unwrap_or_else(|_| {
        if key.is_some() {
            "brave".to_string()
        } else {
            "mojeek".to_string()
        }
    });

    match provider.as_str() {
        "mojeek" => mojeek(client, query, n).await,
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
        other => bail!(
            "unknown NISSIA_SEARCH_PROVIDER '{other}' (use: mojeek | brave | serper | tavily)"
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
    // Numeric entities: &#NN; and &#xHH;. Map fancy quotes/dashes to ASCII.
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

/// Parse Mojeek's result HTML (stable, simple markup: li.rN > a.title + p.s).
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
                hits.push(Hit {
                    title,
                    url,
                    snippet,
                });
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
    let html = client
        .get(url)
        .header("User-Agent", UA)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Mojeek request failed")?
        .text()
        .await
        .context("Mojeek response read failed")?;
    Ok(parse_mojeek(&html, n))
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
