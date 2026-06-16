pub mod compressor;
pub mod extractor;
pub mod filter;

use nissia_cdp::commands::{DomGetBoxModel, DomGetDocument, DomQuerySelectorAll, RuntimeEvaluate};
use nissia_cdp::CdpTransport;

use crate::element_map::ElementMap;

/// Browser emulation settings passed through the CLI.
#[derive(Debug, Default, Clone)]
pub struct EmulationOptions {
    pub geo: Option<(f64, f64)>,
    pub locale: Option<String>,
    pub user_agent: Option<String>,
    /// IANA timezone id (e.g. "America/Sao_Paulo"). Kept consistent with `geo`.
    pub timezone: Option<String>,
}

fn session_emu_file() -> std::path::PathBuf {
    crate::data_dir().join("session_emu.json")
}

/// Persist the launch-time language + emulation so later commands reuse the same
/// fingerprint (geo/locale/timezone/lang/UA) without the caller repeating the
/// flags. Cross-command CONSISTENCY is the #1 thing anti-bot systems check, so a
/// session must look the same on every request, not just on the first `goto`.
pub fn save_session_emu(lang: &str, emu: &EmulationOptions) {
    let v = serde_json::json!({
        "lang": lang,
        "geo": emu.geo.map(|(a,b)| vec![a,b]),
        "locale": emu.locale,
        "user_agent": emu.user_agent,
        "timezone": emu.timezone,
    });
    let _ = std::fs::write(session_emu_file(), v.to_string());
}

/// Load the persisted session emulation (lang, options) written at launch, if any.
pub fn load_session_emu() -> Option<(String, EmulationOptions)> {
    let txt = std::fs::read_to_string(session_emu_file()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&txt).ok()?;
    let lang = v["lang"].as_str().unwrap_or("en-US").to_string();
    let geo = v["geo"].as_array().and_then(|a| {
        if a.len() == 2 {
            Some((a[0].as_f64()?, a[1].as_f64()?))
        } else {
            None
        }
    });
    let emu = EmulationOptions {
        geo,
        locale: v["locale"].as_str().map(|s| s.to_string()),
        user_agent: v["user_agent"].as_str().map(|s| s.to_string()),
        timezone: v["timezone"].as_str().map(|s| s.to_string()),
    };
    Some((lang, emu))
}

/// Build a clean `navigator.languages` list from a single language tag, with no
/// HTTP q-values (those belong only in the Accept-Language *header*). A real
/// browser's `navigator.languages` looks like `["pt-BR","pt","en"]`, never
/// `["pt-BR","en;q=0.9"]`. e.g. "pt-BR" → "pt-BR,pt,en"; "en-US" → "en-US,en".
fn clean_language_list(lang: &str) -> String {
    let base = lang.split('-').next().unwrap_or(lang);
    if lang == base {
        if lang.starts_with("en") {
            lang.to_string()
        } else {
            format!("{lang},en")
        }
    } else if base == "en" {
        format!("{lang},{base}")
    } else {
        format!("{lang},{base},en")
    }
}

/// Apply browser environment overrides (Accept-Language, geolocation, locale, user-agent).
/// Must be called before navigation.
pub async fn apply_emulation(
    transport: &CdpTransport,
    lang: &str,
    opts: &EmulationOptions,
) -> Result<(), nissia_cdp::CdpTransportError> {
    use nissia_cdp::commands::{NetworkEnable, NetworkSetExtraHTTPHeaders};

    // Accept-Language header
    transport.send(&NetworkEnable {}).await?;
    let mut headers = std::collections::HashMap::new();
    headers.insert("Accept-Language".to_string(), format!("{lang},en;q=0.9"));
    transport
        .send(&NetworkSetExtraHTTPHeaders { headers })
        .await?;

    // Geolocation override
    if let Some((lat, lon)) = opts.geo {
        use nissia_cdp::commands::EmulationSetGeolocationOverride;
        transport
            .send(&EmulationSetGeolocationOverride {
                latitude: Some(lat),
                longitude: Some(lon),
                accuracy: Some(100.0),
            })
            .await?;
    }

    // Locale override (affects Intl: number/date formatting, currency).
    if let Some(ref loc) = opts.locale {
        use nissia_cdp::commands::EmulationSetLocaleOverride;
        transport
            .send(&EmulationSetLocaleOverride {
                locale: Some(loc.clone()),
            })
            .await?;
    }

    // Timezone override — keep it consistent with the overridden geolocation.
    // A geo in Rio but a timezone in New York is a classic anti-bot mismatch.
    if let Some(ref tz) = opts.timezone {
        use nissia_cdp::commands::EmulationSetTimezoneOverride;
        let _ = transport
            .send(&EmulationSetTimezoneOverride {
                timezone_id: tz.clone(),
            })
            .await;
    }

    // User-Agent + Accept-Language. The `acceptLanguage` field of
    // setUserAgentOverride is what actually drives `navigator.language` /
    // `navigator.languages` (setLocaleOverride only touches Intl). We send it
    // ALWAYS so the JS language matches Accept-Language and the locale — even
    // when the caller did not pass a custom UA, in which case we reuse the
    // browser's REAL user-agent (no UA faking, just language consistency).
    {
        use nissia_cdp::commands::{EmulationSetUserAgentOverride, RuntimeEvaluate};
        let ua = match opts.user_agent {
            Some(ref ua) => ua.clone(),
            None => transport
                .send(&RuntimeEvaluate {
                    expression: "navigator.userAgent".to_string(),
                    return_by_value: Some(true),
                    await_promise: Some(false),
                    context_id: None,
                })
                .await
                .ok()
                .and_then(|r| r.result.value)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default(),
        };
        if !ua.is_empty() {
            transport
                .send(&EmulationSetUserAgentOverride {
                    user_agent: ua,
                    // CLEAN list (no q-values): this becomes navigator.languages,
                    // and a real browser never has ";q=0.9" inside that array.
                    accept_language: Some(clean_language_list(lang)),
                    platform: None,
                })
                .await?;
        }
    }

    Ok(())
}

/// Result of a snap operation.
pub struct SnapResult {
    pub output: String,
    pub element_map: ElementMap,
    pub element_count: usize,
}

/// Execute a snap: extract interactable elements from the current page.
pub async fn execute(
    transport: &CdpTransport,
    url: Option<&str>,
    focus: Option<&str>,
    lang: &str,
    emu: &EmulationOptions,
) -> Result<SnapResult, nissia_cdp::CdpTransportError> {
    // Apply environment overrides (Accept-Language, geo, locale, user-agent)
    apply_emulation(transport, lang, emu).await?;

    // Navigate if URL provided
    if let Some(url) = url {
        use nissia_cdp::commands::PageNavigate;
        let nav = PageNavigate {
            url: url.to_string(),
        };
        let resp = transport.send(&nav).await?;
        if let Some(err) = resp.error_text {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "Page.navigate".into(),
                code: -1,
                message: err,
            });
        }

        // Wait for the DOM to be ready (fast: does not block on ads/images).
        wait_dom_ready(transport, 6000).await;
    }

    // Extract elements and page context
    let (raw_elements, mut page_context) = extractor::extract(transport).await?;

    // Supplement section summaries via JS for SPA pages where DOMSnapshot layout.text
    // may not capture dynamically rendered content (prices, descriptions, etc.)
    // JS extracts text grouped by their nearest heading, then we match to PageContext headings.
    if let Ok(js_sections) = extract_section_summaries_js(transport).await {
        page_context.js_section_summaries = js_sections;
    }

    // Resolve focus bounds if --focus was provided
    let focus_bounds = if let Some(selector) = focus {
        resolve_focus_bounds(transport, selector)
            .await
            .unwrap_or(None)
    } else {
        None
    };

    // Filter to interactable elements (optionally constrained to focus bounds)
    let filtered = filter::filter_elements(raw_elements, focus_bounds);

    // Compress into output format and build element map (with page context)
    let (output, element_map) = compressor::compress(filtered, Some(&page_context));
    let element_count = element_map.elements.len();

    // Persist element map
    element_map
        .save()
        .map_err(|e| nissia_cdp::CdpTransportError::ConnectionFailed(e.to_string()))?;

    Ok(SnapResult {
        output,
        element_map,
        element_count,
    })
}

/// Extract text content grouped by section headings via JS.
/// Returns (heading_text, section_content) pairs.
/// This captures JS-rendered content that DOMSnapshot may miss on SPAs.
async fn extract_section_summaries_js(
    transport: &CdpTransport,
) -> Result<Vec<(String, String)>, nissia_cdp::CdpTransportError> {
    let js = r#"(function(){
var skip={'script':1,'style':1,'noscript':1,'svg':1,'head':1,'meta':1,'link':1,'iframe':1,'canvas':1};
var sections=[];var curH='';var curT=[];
function flush(){if(curH&&curT.length){sections.push({h:curH,t:curT.join(' | ').slice(0,200)});}curT=[];}
function v(el){try{var s=window.getComputedStyle(el);
if(s.display==='none'||s.visibility==='hidden'||s.opacity==='0')return false;
var r=el.getBoundingClientRect();if(r.width===0&&r.height===0)return false;}catch(e){}return true;}
function walk(el){if(!el||el.nodeType!==1)return;var tag=el.tagName.toLowerCase();
if(skip[tag])return;if(!v(el))return;
if(/^h[1-6]$/.test(tag)){flush();curH=(el.innerText||'').trim().replace(/\s+/g,' ').slice(0,120);return;}
if(tag==='p'||tag==='li'||tag==='td'||tag==='th'||tag==='dt'||tag==='dd'||
tag==='figcaption'||tag==='blockquote'||tag==='label'){
var t=(el.innerText||'').trim().replace(/\s+/g,' ');
if(t&&t.length>1&&t.length<300)curT.push(t.slice(0,150));return;}
if((tag==='div'||tag==='section'||tag==='span')&&el.children.length===0){
var t=(el.innerText||el.textContent||'').trim().replace(/\s+/g,' ');
if(t&&t.length>1&&t.length<200)curT.push(t.slice(0,150));return;}
for(var c=0;c<el.children.length;c++)walk(el.children[c]);}
walk(document.body);flush();return JSON.stringify(sections);
})()"#;

    let result = transport
        .send(&RuntimeEvaluate {
            expression: js.to_string(),
            return_by_value: Some(true),
            await_promise: Some(false),
            context_id: None,
        })
        .await?;

    let json_str = result
        .result
        .value
        .as_ref()
        .and_then(|v| v.as_str())
        .unwrap_or("[]");

    let items: Vec<serde_json::Value> = serde_json::from_str(json_str).unwrap_or_default();

    Ok(items
        .iter()
        .filter_map(|item| {
            let heading = item.get("h")?.as_str()?.to_string();
            let text = item.get("t")?.as_str()?.to_string();
            if heading.is_empty() || text.is_empty() {
                return None;
            }
            Some((heading, text))
        })
        .collect())
}

/// Resolve the bounding box [x, y, w, h] of the first element matching `selector`.
/// Returns None if the selector matches nothing or has no layout.
async fn resolve_focus_bounds(
    transport: &CdpTransport,
    selector: &str,
) -> Result<Option<[f64; 4]>, nissia_cdp::CdpTransportError> {
    let doc = transport
        .send(&DomGetDocument {
            depth: Some(0),
            pierce: None,
        })
        .await?;
    let root_id = doc.root.node_id;

    let result = transport
        .send(&DomQuerySelectorAll {
            node_id: root_id,
            selector: selector.to_string(),
        })
        .await?;

    let Some(&node_id) = result.node_ids.first() else {
        return Ok(None);
    };

    let box_model = transport
        .send(&DomGetBoxModel {
            node_id: Some(node_id),
            backend_node_id: None,
        })
        .await?;

    let content = &box_model.model.content;
    if content.len() < 8 {
        return Ok(None);
    }

    // content quad: [x0,y0, x1,y1, x2,y2, x3,y3]
    let xs: Vec<f64> = content.iter().step_by(2).copied().collect();
    let ys: Vec<f64> = content.iter().skip(1).step_by(2).copied().collect();
    let x_min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let y_min = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let x_max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let y_max = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    Ok(Some([x_min, y_min, x_max - x_min, y_max - y_min]))
}


/// Wait until the DOM is ready (document.readyState interactive/complete) instead of
/// blocking on the full `load` event (which can take ~30s on ad-heavy pages). Polls
/// readyState so it returns as soon as content exists, capped at `max_ms`.
pub async fn wait_dom_ready(transport: &CdpTransport, max_ms: u64) {
    let start = std::time::Instant::now();
    loop {
        let r = transport
            .send(&nissia_cdp::commands::RuntimeEvaluate {
                expression: "document.readyState".to_string(),
                return_by_value: Some(true),
                await_promise: None,
                context_id: None,
            })
            .await;
        if let Ok(resp) = r {
            if let Some(serde_json::Value::String(st)) = resp.result.value {
                if st == "interactive" || st == "complete" {
                    break;
                }
            }
        }
        if start.elapsed().as_millis() as u64 >= max_ms {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }
    // brief settle for late-rendered content / SPA hydration
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
}
