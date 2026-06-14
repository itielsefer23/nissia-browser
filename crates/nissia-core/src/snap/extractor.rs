//! Extracts raw element data from the page using DOMSnapshot + Accessibility APIs.

use std::collections::HashMap;

use nissia_cdp::commands::{AXNode, AccessibilityGetFullAXTree, DomSnapshotCaptureSnapshot};
use nissia_cdp::types::BackendNodeId;
use nissia_cdp::CdpTransport;

/// A raw extracted element before filtering.
#[derive(Debug, Clone)]
pub struct RawElement {
    pub backend_node_id: BackendNodeId,
    pub node_index: usize,
    pub tag: String,
    pub attributes: HashMap<String, String>,
    pub role: String,
    pub name: String,
    pub value: String,
    pub bounds: Option<[f64; 4]>,
    pub is_visible: bool,
    pub ax_properties: HashMap<String, String>,
    pub ax_description: String,
}

/// Contextual information extracted from the page structure.
/// Maps each element's node_index to its surrounding context.
#[derive(Debug, Default)]
pub struct PageContext {
    /// Headings in document order: (node_index, level, text).
    pub headings: Vec<(usize, u8, String)>,
    /// Visible text blocks in document order: (node_index, text).
    pub text_blocks: Vec<(usize, String)>,
    /// Parent chain for structural proximity checks.
    pub parent_map: HashMap<usize, usize>,
    /// JS-extracted section summaries: heading text → section content text.
    /// Used when DOMSnapshot layout.text doesn't capture JS-rendered content.
    pub js_section_summaries: Vec<(String, String)>,
}

impl PageContext {
    /// Find the heading that applies to a given node_index
    /// (the last heading before this node in document order).
    pub fn heading_for(&self, node_index: usize) -> Option<(u8, &str)> {
        self.headings
            .iter()
            .rev()
            .find(|(idx, _, _)| *idx < node_index)
            .map(|(_, level, text)| (*level, text.as_str()))
    }

    /// Get ancestors up to `depth` levels for a node.
    fn ancestors(&self, node_index: usize, depth: usize) -> Vec<usize> {
        let mut result = Vec::with_capacity(depth);
        let mut current = node_index;
        for _ in 0..depth {
            if let Some(&parent) = self.parent_map.get(&current) {
                result.push(parent);
                current = parent;
            } else {
                break;
            }
        }
        result
    }

    /// Check if two nodes share a common ancestor within `depth` levels.
    fn shares_ancestor(&self, a: usize, b: usize, depth: usize) -> bool {
        let ancestors_a = self.ancestors(a, depth);
        let ancestors_b = self.ancestors(b, depth);
        ancestors_a.iter().any(|a| ancestors_b.contains(a))
    }

    /// Collect brief surrounding text near a node_index.
    /// Uses structural proximity (shared ancestors within 3 levels) rather than
    /// raw document-order distance, so text from unrelated sections is excluded.
    pub fn nearby_text(&self, node_index: usize, max_chars: usize) -> String {
        let mut nearby: Vec<&str> = Vec::new();
        let mut total_len = 0;
        for (idx, text) in &self.text_blocks {
            let dist = (*idx as isize - node_index as isize).unsigned_abs();
            // Must be within ±50 indices AND share a common ancestor within 3 levels
            if dist <= 50
                && !text.is_empty()
                && (self.parent_map.is_empty() || self.shares_ancestor(node_index, *idx, 3))
            {
                if total_len + text.len() > max_chars {
                    break;
                }
                nearby.push(text.as_str());
                total_len += text.len();
            }
        }
        nearby.join(" ")
    }

    /// Collect text blocks within a heading section (between start_idx and end_idx).
    /// Returns a compact summary joined with ` | `, up to max_chars.
    /// Falls back to JS-extracted section summaries if DOMSnapshot text is empty.
    pub fn section_text(
        &self,
        start_idx: usize,
        end_idx: usize,
        max_chars: usize,
        heading_text: &str,
    ) -> String {
        // Try DOMSnapshot text blocks first
        let mut parts: Vec<&str> = Vec::new();
        let mut total_len = 0;
        for (idx, text) in &self.text_blocks {
            if *idx >= start_idx && *idx < end_idx && !text.is_empty() {
                let needed = if parts.is_empty() {
                    text.len()
                } else {
                    text.len() + 3 // " | " separator
                };
                if total_len + needed > max_chars {
                    break;
                }
                parts.push(text.as_str());
                total_len += needed;
            }
        }

        if !parts.is_empty() {
            return parts.join(" | ");
        }

        // Fallback: JS-extracted section summaries (for SPA pages)
        let heading_lower = heading_text.to_lowercase();
        for (js_heading, js_content) in &self.js_section_summaries {
            if js_heading.to_lowercase() == heading_lower {
                if js_content.len() <= max_chars {
                    return js_content.clone();
                }
                return format!(
                    "{}...",
                    js_content
                        .chars()
                        .take(max_chars.saturating_sub(3))
                        .collect::<String>()
                );
            }
        }

        String::new()
    }
}

/// Extract all elements from the page by merging DOMSnapshot and Accessibility data.
/// Also extracts page context (headings, text blocks) for contextual snap output.
pub async fn extract(
    transport: &CdpTransport,
) -> Result<(Vec<RawElement>, PageContext), nissia_cdp::CdpTransportError> {
    // Enable Page events for proper load detection
    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    // Capture DOM snapshot with computed styles for visibility checks
    let snapshot_cmd = DomSnapshotCaptureSnapshot {
        computed_styles: vec![
            "display".to_string(),
            "visibility".to_string(),
            "opacity".to_string(),
        ],
        include_dom_rects: Some(true),
        include_paint_order: None,
    };
    let snapshot = transport.send(&snapshot_cmd).await?;

    // Get accessibility tree
    let ax_cmd = AccessibilityGetFullAXTree {
        depth: None,
        frame_id: None,
    };
    let ax_tree = transport.send(&ax_cmd).await?;

    // Build a lookup from backendNodeId -> AXNode
    let ax_by_backend_id: HashMap<BackendNodeId, &AXNode> = ax_tree
        .nodes
        .iter()
        .filter_map(|n| n.backend_dom_node_id.map(|id| (id, n)))
        .collect();

    // Process each document in the snapshot
    let mut elements = Vec::new();
    let mut context = PageContext::default();

    for doc in &snapshot.documents {
        let strings = &snapshot.strings;
        let nodes = &doc.nodes;
        let layout = &doc.layout;

        // Build layout lookup: node_index -> bounds
        let mut layout_bounds: HashMap<usize, [f64; 4]> = HashMap::new();
        for (i, &node_idx) in layout.node_index.iter().enumerate() {
            if let Some(bounds) = layout.bounds.get(i) {
                if bounds.len() == 4 {
                    layout_bounds.insert(
                        node_idx as usize,
                        [bounds[0], bounds[1], bounds[2], bounds[3]],
                    );
                }
            }
        }

        // Build layout text lookup: node_index -> rendered text
        let mut layout_text: HashMap<usize, String> = HashMap::new();
        for (i, &node_idx) in layout.node_index.iter().enumerate() {
            let text_idx = layout.text.get(i).copied().unwrap_or(-1);
            if text_idx >= 0 {
                if let Some(text) = strings.get(text_idx as usize) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        layout_text.insert(node_idx as usize, trimmed.to_string());
                    }
                }
            }
        }

        // Build computed styles lookup for visibility
        let mut style_values: HashMap<usize, Vec<String>> = HashMap::new();
        for (i, &node_idx) in layout.node_index.iter().enumerate() {
            if let Some(style_indices) = layout.styles.get(i) {
                let values: Vec<String> = style_indices
                    .iter()
                    .map(|&si| strings.get(si as usize).cloned().unwrap_or_default())
                    .collect();
                style_values.insert(node_idx as usize, values);
            }
        }

        // Build children map + parent map for text collection and structural proximity
        let mut children_map: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, &parent_idx) in nodes.parent_index.iter().enumerate() {
            if parent_idx >= 0 {
                context.parent_map.insert(i, parent_idx as usize);
                children_map.entry(parent_idx as usize).or_default().push(i);
            }
        }

        let node_count = nodes.node_name.len();

        for i in 0..node_count {
            let backend_id = *nodes.backend_node_id.get(i).unwrap_or(&0);

            let tag = nodes
                .node_name
                .get(i)
                .and_then(|&idx| strings.get(idx as usize))
                .cloned()
                .unwrap_or_default()
                .to_lowercase();

            let bounds = layout_bounds.get(&i).copied();
            let is_visible = check_visibility(&style_values.get(&i), bounds);

            // --- Collect page context: headings and text blocks ---
            if is_visible {
                // Headings (h1-h6): collect for section markers
                if tag.len() == 2 && tag.starts_with('h') {
                    if let Some(level) = tag[1..].parse::<u8>().ok().filter(|l| (1..=6).contains(l))
                    {
                        let heading_text = collect_child_text(i, nodes, strings, &children_map);
                        if !heading_text.is_empty() {
                            context
                                .headings
                                .push((i, level, truncate_str(&heading_text, 120)));
                        }
                    }
                }

                // Text blocks: semantic text tags + leaf divs (for prices, descriptions)
                let is_text_tag = matches!(
                    tag.as_str(),
                    "p" | "li"
                        | "td"
                        | "th"
                        | "span"
                        | "label"
                        | "dd"
                        | "dt"
                        | "figcaption"
                        | "blockquote"
                        | "strong"
                        | "em"
                        | "b"
                        | "time"
                        | "data"
                        | "mark"
                        | "a"
                );
                let is_leaf_container = matches!(tag.as_str(), "div" | "section")
                    && !children_map.get(&i).is_some_and(|c| {
                        c.iter().any(|&ci| {
                            nodes.node_type.get(ci).copied().unwrap_or(0) == 1
                            // 1 = element node; leaf means no element children
                        })
                    });
                if is_text_tag || is_leaf_container {
                    if let Some(text) = layout_text.get(&i) {
                        let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if clean.len() > 1 {
                            context.text_blocks.push((i, truncate_str(&clean, 150)));
                        }
                    }
                }
            }

            // --- Continue building interactable elements ---
            if backend_id == 0 {
                continue;
            }

            // Parse attributes
            let attrs = parse_attributes(nodes.attributes.get(i), strings);

            // Get accessibility info
            let (role, name, value, ax_props, ax_desc) =
                if let Some(ax_node) = ax_by_backend_id.get(&backend_id) {
                    (
                        ax_value_str(&ax_node.role),
                        ax_value_str(&ax_node.name),
                        ax_value_str(&ax_node.value),
                        extract_ax_properties(ax_node),
                        ax_value_str(&ax_node.description),
                    )
                } else {
                    (
                        String::new(),
                        String::new(),
                        String::new(),
                        HashMap::new(),
                        String::new(),
                    )
                };

            elements.push(RawElement {
                backend_node_id: backend_id,
                node_index: i,
                tag,
                attributes: attrs,
                role,
                name,
                value,
                bounds,
                is_visible,
                ax_properties: ax_props,
                ax_description: ax_desc,
            });
        }
    }

    Ok((elements, context))
}

/// Collect text content from child text nodes (for heading text).
fn collect_child_text(
    parent_idx: usize,
    nodes: &nissia_cdp::commands::NodeTreeSnapshot,
    strings: &[String],
    children_map: &HashMap<usize, Vec<usize>>,
) -> String {
    let mut text = String::new();
    if let Some(children) = children_map.get(&parent_idx) {
        for &child_idx in children {
            // text node (node_type 3)
            let node_type = nodes.node_type.get(child_idx).copied().unwrap_or(0);
            if node_type == 3 {
                let val_idx = nodes.node_value.get(child_idx).copied().unwrap_or(-1);
                if val_idx >= 0 {
                    if let Some(s) = strings.get(val_idx as usize) {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            if !text.is_empty() {
                                text.push(' ');
                            }
                            text.push_str(trimmed);
                        }
                    }
                }
            } else {
                // Recurse into child elements
                let child_text = collect_child_text(child_idx, nodes, strings, children_map);
                if !child_text.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&child_text);
                }
            }
        }
    }
    text
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

fn parse_attributes(
    attr_indices: Option<&Vec<i64>>,
    strings: &[String],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(indices) = attr_indices else {
        return map;
    };
    // Attributes come in pairs: [name_idx, value_idx, name_idx, value_idx, ...]
    for chunk in indices.chunks(2) {
        if chunk.len() == 2 {
            let key = strings.get(chunk[0] as usize).cloned().unwrap_or_default();
            let val = strings.get(chunk[1] as usize).cloned().unwrap_or_default();
            if !key.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

fn check_visibility(styles: &Option<&Vec<String>>, bounds: Option<[f64; 4]>) -> bool {
    // No layout = not visible
    if bounds.is_none() {
        return false;
    }

    let bounds = bounds.unwrap();
    // Zero-size = not visible
    if bounds[2] <= 0.0 || bounds[3] <= 0.0 {
        return false;
    }

    if let Some(styles) = styles {
        // styles order matches computed_styles request: [display, visibility, opacity]
        if let Some(display) = styles.first() {
            if display == "none" {
                return false;
            }
        }
        if let Some(visibility) = styles.get(1) {
            if visibility == "hidden" || visibility == "collapse" {
                return false;
            }
        }
        if let Some(opacity) = styles.get(2) {
            if let Ok(v) = opacity.parse::<f64>() {
                if v == 0.0 {
                    return false;
                }
            }
        }
    }

    true
}

fn ax_value_str(val: &Option<nissia_cdp::commands::AXValue>) -> String {
    val.as_ref()
        .and_then(|v| v.value.as_ref())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn extract_ax_properties(node: &AXNode) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for prop in &node.properties {
        if let Some(val) = &prop.value.value {
            let val_str = match val {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                other => other.to_string(),
            };
            props.insert(prop.name.clone(), val_str);
        }
    }
    props
}
