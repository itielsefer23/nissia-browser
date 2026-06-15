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
    browser: bool,
    open: Option<usize>,
    fmt: &str,
    lang: &str,
    emu: &EmulationOptions,
) -> Result<()> {
    let client = reqwest::Client::new();
    let mut hits = if browser {
        ddg_browser(port, query, n, open, lang, emu).await?
    } else {
        fetch(&client, query, n).await?
    };
    // Reliability net: if the HTTP search came back empty (e.g. an engine rate-limited
    // us), fall back to a real (headless) browser DuckDuckGo search, which is reliable.
    if hits.is_empty() && !browser {
        if let Ok(h) = ddg_browser(port, query, n, None, lang, emu).await {
            hits = h;
        }
    }

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
        "searxng" => {
            let base = std::env::var("NISSIA_SEARXNG_URL")
                .ok()
                .or_else(cfg_searxng_url)
                .context("set NISSIA_SEARXNG_URL (or \"searxng_url\" in search.json) to your SearXNG instance, e.g. http://localhost:8888")?;
            searxng(client, &base, query, n).await
        }
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
            "unknown NISSIA_SEARCH_PROVIDER '{other}' (use: auto | mojeek | ddg | searxng | brave | serper | tavily)"
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

/// Free, reliable web search via the RUNNING browser (DuckDuckGo HTML), ads filtered.
/// Works without any API key because it is a real browser; no HTTP rate limits.
async fn ddg_browser(
    port: u16,
    query: &str,
    n: usize,
    open: Option<usize>,
    lang: &str,
    emu: &EmulationOptions,
) -> Result<Vec<Hit>> {
    super::ensure_browser(port).await?;
    let transport = nissia_cdp::connect(port).await?;
    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    // If the browser is ALREADY on a DuckDuckGo results page for this same query
    // (e.g. we just listed results and now `--open N` is opening one), reuse that
    // page — do NOT re-run the whole search. Re-searching looks like going "back to
    // the start, re-typing the query" to the user, and is slower.
    let q_json = serde_json::to_string(query).unwrap_or_else(|_| "\"\"".to_string());
    let already_check = format!(
        "(function(){{try{{var onddg=/duckduckgo\\.com/.test(location.href);var has=!!document.querySelector('.result');var inp=document.querySelector('input[name=q]');var v=inp?(inp.value||'').trim().toLowerCase():'';var q={q_json}.trim().toLowerCase();return onddg&&has&&v===q;}}catch(e){{return false;}}}})()"
    );
    let already = transport
        .send(&nissia_cdp::commands::RuntimeEvaluate {
            expression: already_check,
            return_by_value: Some(true),
            await_promise: None,
            context_id: None,
        })
        .await
        .ok()
        .and_then(|r| r.result.value)
        == Some(Value::Bool(true));

    // Human search: open DuckDuckGo, move+click the search box, TYPE the query and
    // submit — instead of teleporting straight to a results URL. Falls back to the
    // direct results URL if the box can't be operated, so search never breaks.
    let typed = async {
        if already {
            return Ok::<(), nissia_cdp::CdpTransportError>(());
        }
        nissia_core::snap::execute(
            &transport,
            Some("https://html.duckduckgo.com/html/"),
            None,
            lang,
            emu,
        )
        .await?;
        nissia_core::action::wait::execute(
            &transport,
            nissia_core::action::wait::WaitCondition::Selector("input[name=q]"),
        )
        .await?;
        nissia_core::action::click::execute_selector(&transport, "input[name=q]").await?;
        nissia_core::action::type_text::execute_selector(&transport, "input[name=q]", query).await?;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        // Submit: click the search button (DDG's /html/ form ignores Enter), with a
        // human mouse trajectory. Press Enter too as a belt-and-suspenders.
        let _ = nissia_core::action::key::execute(&transport, "enter").await;
        let _ = nissia_core::action::click::execute_selector(
            &transport,
            "input[type=submit], button[type=submit]",
        )
        .await;
        nissia_core::snap::wait_dom_ready(&transport, 8000).await;
        Ok::<(), nissia_cdp::CdpTransportError>(())
    }
    .await;
    if typed.is_err() {
        let url =
            reqwest::Url::parse_with_params("https://html.duckduckgo.com/html/", &[("q", query)])?;
        let _ = nissia_core::snap::execute(&transport, Some(url.as_str()), None, lang, emu).await?;
    }
    let js = format!(
        r#"JSON.stringify(Array.from(document.querySelectorAll('.result')).filter(function(r){{return !/result--ad|result--sponsored/.test(r.className)}}).map(function(r){{var a=r.querySelector('.result__a');var s=r.querySelector('.result__snippet');var h=a?a.href:'';try{{var u=new URL(h,location.href);var t=u.searchParams.get('uddg');if(t)h=decodeURIComponent(t);}}catch(e){{}}return{{title:a?a.innerText.trim():'',url:h,snippet:s?s.innerText.trim():''}}}}).filter(function(x){{return x.title && !/\/y\.js|duckduckgo\.com/.test(x.url)}}).slice(0,{n}))"#
    );
    fn extract_hits(raw: &str) -> Vec<Hit> {
        serde_json::from_str::<Vec<Value>>(raw)
            .unwrap_or_default()
            .into_iter()
            .map(|it| Hit {
                title: it["title"].as_str().unwrap_or("").to_string(),
                url: it["url"].as_str().unwrap_or("").to_string(),
                snippet: it["snippet"].as_str().unwrap_or("").to_string(),
            })
            .collect()
    }

    let result = transport
        .send(&nissia_cdp::commands::RuntimeEvaluate {
            expression: js.clone(),
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    let raw = match result.result.value {
        Some(Value::String(s)) => s,
        Some(other) => other.to_string(),
        None => "[]".to_string(),
    };
    let mut hits: Vec<Hit> = extract_hits(&raw);

    // Reliability: if the typed search came back empty (a timing hiccup or an odd
    // phrasing), retry once via the direct results URL so search never silently
    // returns nothing.
    if hits.is_empty() {
        if let Ok(url) =
            reqwest::Url::parse_with_params("https://html.duckduckgo.com/html/", &[("q", query)])
        {
            let _ = nissia_core::snap::execute(&transport, Some(url.as_str()), None, lang, emu).await;
            let result2 = transport
                .send(&nissia_cdp::commands::RuntimeEvaluate {
                    expression: js.clone(),
                    return_by_value: Some(true),
                    await_promise: Some(true),
                    context_id: None,
                })
                .await?;
            let raw2 = match result2.result.value {
                Some(Value::String(s)) => s,
                Some(other) => other.to_string(),
                None => "[]".to_string(),
            };
            hits = extract_hits(&raw2);
        }
    }

    // Human navigation: optionally CLICK the chosen result with a real mouse click
    // (natural referrer from the search engine), instead of teleporting to the URL.
    if let Some(rank) = open {
        let idx = rank.saturating_sub(1);
        let coord_js = format!(
            r#"(function(){{var a=Array.from(document.querySelectorAll('.result')).filter(function(r){{return !/result--ad|result--sponsored/.test(r.className)}}).map(function(r){{return r.querySelector('.result__a')}}).filter(Boolean);var el=a[{idx}];if(!el)return '';el.scrollIntoView({{block:'center'}});var b=el.getBoundingClientRect();return JSON.stringify([b.left+b.width/2,b.top+b.height/2]);}})()"#
        );
        let cr = transport
            .send(&nissia_cdp::commands::RuntimeEvaluate {
                expression: coord_js,
                return_by_value: Some(true),
                await_promise: Some(true),
                context_id: None,
            })
            .await?;
        if let Some(Value::String(xystr)) = cr.result.value {
            if let Ok(xy) = serde_json::from_str::<Vec<f64>>(&xystr) {
                if xy.len() == 2 {
                    // human reading pause, then a real curved-trajectory mouse click
                    tokio::time::sleep(std::time::Duration::from_millis(320)).await;
                    let _ = nissia_core::action::click::human_click_at(&transport, xy[0], xy[1]).await;
                    nissia_core::snap::wait_dom_ready(&transport, 6000).await;
                }
            }
        }
    }

    Ok(hits)
}

/// SearXNG instance URL from search.json ("searxng_url"), if present.
fn cfg_searxng_url() -> Option<String> {
    let path = nissia_core::data_dir().join("search.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["searxng_url"].as_str().map(|s| s.to_string()))
}

/// Self-hosted SearXNG (free, unlimited, aggregates Google/Bing/etc). Needs a URL.
async fn searxng(client: &reqwest::Client, base: &str, query: &str, n: usize) -> Result<Vec<Hit>> {
    let base = base.trim_end_matches('/');
    let url = reqwest::Url::parse_with_params(
        &format!("{base}/search"),
        &[("q", query), ("format", "json")],
    )?;
    let v: Value = client
        .get(url)
        .header("User-Agent", UA)
        .send()
        .await
        .context("SearXNG request failed")?
        .json()
        .await
        .context("SearXNG response was not JSON (enable the JSON format in your instance settings)")?;
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
