//! Compresses filtered elements into token-minimal output format.
//! Output format: `@e1 [button] "Sign In"`

use super::extractor::{PageContext, RawElement};
use crate::element_map::{ElementEntry, ElementMap};

/// Compress filtered elements into output string and element map.
/// When `context` is provided, elements are grouped by section headings
/// and enriched with nearby text for AI comprehension.
pub fn compress(elements: Vec<RawElement>, context: Option<&PageContext>) -> (String, ElementMap) {
    let mut map = ElementMap::new();
    let mut lines = Vec::new();
    let mut current_heading: Option<&str> = None;

    // Pre-compute heading boundaries for section text extraction
    let heading_boundaries: Vec<(usize, usize)> = context
        .map(|ctx| {
            ctx.headings
                .iter()
                .enumerate()
                .map(|(i, (idx, _, _))| {
                    let end = ctx
                        .headings
                        .get(i + 1)
                        .map(|(next_idx, _, _)| *next_idx)
                        .unwrap_or(usize::MAX);
                    (*idx, end)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut last_heading_idx: Option<usize> = None;

    for el in &elements {
        // Insert section heading if it changed
        if let Some(ctx) = context {
            if let Some((level, heading_text)) = ctx.heading_for(el.node_index) {
                let is_new = current_heading != Some(heading_text);
                if is_new {
                    current_heading = Some(heading_text);
                    let hashes = "#".repeat(level as usize);
                    // Add blank line before heading (except first)
                    if !lines.is_empty() {
                        lines.push(String::new());
                    }
                    lines.push(format!("{hashes} {heading_text}"));

                    // Section content summary: key text blocks in this section
                    // Find the heading boundary for this heading
                    if let Some(heading_node_idx) = ctx
                        .headings
                        .iter()
                        .rev()
                        .find(|(idx, _, t)| *idx < el.node_index && t.as_str() == heading_text)
                        .map(|(idx, _, _)| *idx)
                    {
                        if last_heading_idx != Some(heading_node_idx) {
                            last_heading_idx = Some(heading_node_idx);
                            if let Some((start, end)) = heading_boundaries
                                .iter()
                                .find(|(s, _)| *s == heading_node_idx)
                            {
                                let summary = ctx.section_text(*start, *end, 300, heading_text);
                                if !summary.is_empty() {
                                    lines.push(format!("> {summary}"));
                                }
                            }
                        }
                    }
                }
            }
        }

        let selector_hint = build_selector_hint(el);

        let entry = ElementEntry {
            backend_node_id: el.backend_node_id,
            role: if el.role.is_empty() {
                tag_to_role(&el.tag, &el.attributes)
            } else {
                el.role.clone()
            },
            name: el.name.clone(),
            selector_hint,
            tag: el.tag.clone(),
            attributes: el.attributes.clone(),
        };

        let ref_id = map.insert(entry.clone());
        let line = format_element_line(&ref_id, &entry, el, context);
        lines.push(line);
    }

    (lines.join("\n"), map)
}

fn format_element_line(
    ref_id: &str,
    entry: &ElementEntry,
    raw: &RawElement,
    context: Option<&PageContext>,
) -> String {
    let mut parts = Vec::new();

    // @eN
    parts.push(ref_id.to_string());

    // [role] or [tag:type]
    let role_display = format_role_display(&entry.role, &entry.tag, &raw.attributes);
    parts.push(format!("[{role_display}]"));

    // "name" — accessible name or best fallback label in quotes
    let label = best_label(entry, raw);
    if !label.is_empty() {
        parts.push(format!("\"{}\"", label));
    }

    // Extra decision-relevant attributes
    let extras = format_extras(raw);
    if !extras.is_empty() {
        parts.push(extras);
    }

    // Contextual text — brief nearby text that helps understand what this element relates to
    if let Some(ctx) = context {
        let nearby = ctx.nearby_text(raw.node_index, 150);
        if !nearby.is_empty() && nearby != label {
            // Only add if it provides new information beyond the label
            let nearby_trimmed = truncate(&nearby, 120);
            parts.push(format!("— {nearby_trimmed}"));
        }
    }

    parts.join(" ")
}

fn format_role_display(
    role: &str,
    tag: &str,
    attrs: &std::collections::HashMap<String, String>,
) -> String {
    match tag {
        "input" => {
            let input_type = attrs.get("type").map(|s| s.as_str()).unwrap_or("text");
            format!("input:{input_type}")
        }
        _ => {
            if role.is_empty() {
                tag.to_string()
            } else {
                role.to_string()
            }
        }
    }
}

/// Pick the best human-readable label for an element.
/// Priority: accessible name > aria-label > title > placeholder > id
fn best_label(entry: &ElementEntry, raw: &RawElement) -> String {
    if !entry.name.is_empty() {
        return truncate(&entry.name, 60);
    }
    if let Some(v) = raw.attributes.get("aria-label") {
        if !v.is_empty() {
            return truncate(v, 60);
        }
    }
    if let Some(v) = raw.attributes.get("title") {
        if !v.is_empty() {
            return truncate(v, 60);
        }
    }
    if let Some(v) = raw.attributes.get("placeholder") {
        if !v.is_empty() {
            return truncate(v, 60);
        }
    }
    String::new()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

fn format_extras(el: &RawElement) -> String {
    let mut extras = Vec::new();

    // id — useful for identifying elements
    if let Some(id) = el.attributes.get("id") {
        if !id.is_empty() {
            extras.push(format!("id=\"{id}\""));
        }
    }

    // placeholder (only if not already used as label)
    if let Some(ph) = el.attributes.get("placeholder") {
        if !ph.is_empty()
            && el.name.is_empty()
            && !el.attributes.contains_key("aria-label")
            && !el.attributes.contains_key("title")
        {
            // placeholder was used as label, skip in extras
        } else if !ph.is_empty() {
            extras.push(format!("placeholder=\"{ph}\""));
        }
    }

    // href for links (truncated)
    if let Some(href) = el.attributes.get("href") {
        if !href.is_empty() {
            let display = truncate(href, 50);
            extras.push(format!("href=\"{display}\""));
        }
    }

    // checked state
    if el.ax_properties.get("checked").map(|v| v.as_str()) == Some("true")
        || el.attributes.contains_key("checked")
    {
        extras.push("checked".to_string());
    }

    // disabled state
    if el.ax_properties.get("disabled").map(|v| v.as_str()) == Some("true")
        || el.attributes.contains_key("disabled")
    {
        extras.push("disabled".to_string());
    }

    // expanded/collapsed state (dropdowns, accordions, menus)
    match el.ax_properties.get("expanded").map(|v| v.as_str()) {
        Some("true") => extras.push("expanded".to_string()),
        Some("false") => extras.push("collapsed".to_string()),
        _ => {}
    }

    // selected state (tabs, options)
    if el.ax_properties.get("selected").map(|v| v.as_str()) == Some("true") {
        extras.push("selected".to_string());
    }

    // haspopup (clicking opens a popup/menu/dialog)
    if let Some(popup) = el.ax_properties.get("haspopup") {
        if popup != "false" {
            extras.push(format!("popup:{popup}"));
        }
    }

    // required (form validation)
    if el.ax_properties.get("required").map(|v| v.as_str()) == Some("true")
        || el.attributes.contains_key("required")
    {
        extras.push("required".to_string());
    }

    // readonly
    if el.ax_properties.get("readonly").map(|v| v.as_str()) == Some("true")
        || el.attributes.contains_key("readonly")
    {
        extras.push("readonly".to_string());
    }

    // current value for inputs
    if !el.value.is_empty() && el.value != el.name {
        extras.push(format!("value=\"{}\"", el.value));
    }

    // accessibility description (e.g. "Opens in new tab", validation messages)
    if !el.ax_description.is_empty() {
        extras.push(format!("desc=\"{}\"", truncate(&el.ax_description, 80)));
    }

    extras.join(" ")
}

fn build_selector_hint(el: &RawElement) -> String {
    let mut selector = el.tag.clone();

    if let Some(id) = el.attributes.get("id") {
        if !id.is_empty() {
            return format!("#{id}");
        }
    }

    if let Some(name) = el.attributes.get("name") {
        if !name.is_empty() {
            selector.push_str(&format!("[name=\"{name}\"]"));
            return selector;
        }
    }

    if let Some(input_type) = el.attributes.get("type") {
        selector.push_str(&format!("[type=\"{input_type}\"]"));
    }

    selector
}

fn tag_to_role(tag: &str, attrs: &std::collections::HashMap<String, String>) -> String {
    match tag {
        "a" => "link".to_string(),
        "button" => "button".to_string(),
        "input" => {
            let t = attrs.get("type").map(|s| s.as_str()).unwrap_or("text");
            match t {
                "checkbox" => "checkbox".to_string(),
                "radio" => "radio".to_string(),
                "submit" | "button" | "reset" => "button".to_string(),
                _ => "textbox".to_string(),
            }
        }
        "select" => "combobox".to_string(),
        "textarea" => "textbox".to_string(),
        _ => tag.to_string(),
    }
}
