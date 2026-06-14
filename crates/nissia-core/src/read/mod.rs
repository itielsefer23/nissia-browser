//! Page content reader — extracts visible text in a compact structured format.
//! Unlike `snap` (which only surfaces interactable elements), `read` surfaces
//! the human-readable text content of a page so an AI agent can understand
//! what is displayed without taking a screenshot.

use nissia_cdp::commands::{PageNavigate, RuntimeEvaluate};
use nissia_cdp::CdpTransport;

/// Result of a read operation.
pub struct ReadResult {
    /// Structured markdown-like text output.
    pub output: String,
    /// Approximate line count (useful for truncation warnings).
    pub line_count: usize,
}

/// JS that extracts visible structured text from the page.
/// Returns a JSON array of content items.
fn build_extractor_js(focus_selector: Option<&str>) -> String {
    let root_expr = match focus_selector {
        Some(sel) => {
            let escaped = sel.replace('`', "\\`").replace('\\', "\\\\");
            format!("document.querySelector(`{escaped}`) || document.body")
        }
        None => "document.body".to_string(),
    };

    format!(
        r#"(function() {{
  var skip = {{'script':1,'style':1,'noscript':1,'svg':1,'head':1,'meta':1,'link':1,'iframe':1,'canvas':1}};
  var items = [];
  var seen = new Set();

  function visible(el) {{
    try {{
      var s = window.getComputedStyle(el);
      if (s.display === 'none' || s.visibility === 'hidden' || s.opacity === '0') return false;
      var r = el.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) return false;
    }} catch(e) {{}}
    return true;
  }}

  function push(item) {{
    // Deduplicate: skip identical consecutive text
    var key = item.t + ':' + item.text;
    if (seen.has(key)) return;
    seen.add(key);
    items.push(item);
  }}

  function walk(el, inList) {{
    if (!el || el.nodeType !== 1) return;
    var tag = el.tagName.toLowerCase();
    if (skip[tag]) return;
    if (!visible(el)) return;

    // Heading
    if (/^h[1-6]$/.test(tag)) {{
      var t = (el.innerText || '').trim().replace(/\s+/g, ' ');
      if (t) push({{t:'h', level: parseInt(tag[1]), text: t.slice(0, 200)}});
      return;
    }}

    // Table
    if (tag === 'table') {{
      var rows = el.querySelectorAll('tr');
      rows.forEach(function(tr) {{
        if (!visible(tr)) return;
        var cells = [];
        tr.querySelectorAll('th,td').forEach(function(td) {{
          cells.push((td.innerText || '').trim().replace(/\s+/g, ' ').slice(0, 80));
        }});
        if (cells.some(function(c) {{ return c.length > 0; }})) {{
          push({{t:'tr', text: cells.join(' | ')}});
        }}
      }});
      return;
    }}

    // List item
    if (tag === 'li') {{
      var t = (el.innerText || '').trim().replace(/\s+/g, ' ');
      if (t) push({{t:'li', text: t.slice(0, 200)}});
      return;
    }}

    // Paragraph / label / caption / figcaption / blockquote / dt / dd
    if (tag === 'p' || tag === 'label' || tag === 'figcaption' ||
        tag === 'blockquote' || tag === 'dt' || tag === 'dd' || tag === 'caption') {{
      var t = (el.innerText || '').trim().replace(/\s+/g, ' ');
      if (t) push({{t:'p', text: t.slice(0, 300)}});
      return;
    }}

    // Leaf node with text and no significant children
    var children = el.children;
    if (children.length === 0) {{
      var t = (el.innerText || el.textContent || '').trim().replace(/\s+/g, ' ');
      if (t && t.length > 1 && t.length < 300) push({{t:'text', text: t}});
      return;
    }}

    for (var i = 0; i < children.length; i++) walk(children[i], inList);
  }}

  var root = {root_expr};
  if (root) walk(root, false);
  return JSON.stringify(items);
}})()
"#
    )
}

/// Render the JSON items from JS into compact markdown text.
fn render_items(json: &str, max_lines: usize) -> (String, usize) {
    let items: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (json.to_string(), 1),
    };

    let mut lines: Vec<String> = Vec::new();

    for item in &items {
        let t = item.get("t").and_then(|v| v.as_str()).unwrap_or("text");
        let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if text.is_empty() {
            continue;
        }

        let line = match t {
            "h" => {
                let level = item.get("level").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                let hashes = "#".repeat(level.min(6));
                format!("{hashes} {text}")
            }
            "li" => format!("- {text}"),
            "tr" => format!("| {text} |"),
            "p" | "text" => text.to_string(),
            _ => text.to_string(),
        };

        lines.push(line);

        if lines.len() >= max_lines {
            lines.push(format!(
                "... ({} more items truncated)",
                items.len() - lines.len()
            ));
            break;
        }
    }

    let count = lines.len();
    (lines.join("\n"), count)
}

/// Execute a read: extract visible text content from the current page.
pub async fn execute(
    transport: &CdpTransport,
    url: Option<&str>,
    focus: Option<&str>,
    lang: &str,
    max_lines: usize,
    emu: &crate::snap::EmulationOptions,
) -> Result<ReadResult, nissia_cdp::CdpTransportError> {
    // Apply environment overrides (Accept-Language, geo, locale, user-agent)
    crate::snap::apply_emulation(transport, lang, emu).await?;

    // Navigate if URL provided
    if let Some(url) = url {
        use nissia_cdp::commands::PageEnable;
        transport.send(&PageEnable {}).await?;

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

        transport
            .wait_for_event("Page.loadEventFired", std::time::Duration::from_secs(30))
            .await?;
    }

    // Run the content extractor JS
    let js = build_extractor_js(focus);
    let eval_result = transport
        .send(&RuntimeEvaluate {
            expression: js,
            return_by_value: Some(true),
            await_promise: Some(false),
            context_id: None,
        })
        .await?;

    if let Some(exc) = eval_result.exception_details {
        return Err(nissia_cdp::CdpTransportError::CommandFailed {
            method: "Runtime.evaluate (read)".into(),
            code: -1,
            message: format!("{:?}", exc),
        });
    }

    let json_str = eval_result
        .result
        .value
        .as_ref()
        .and_then(|v| v.as_str())
        .unwrap_or("[]");

    let (output, line_count) = render_items(json_str, max_lines);

    Ok(ReadResult { output, line_count })
}
