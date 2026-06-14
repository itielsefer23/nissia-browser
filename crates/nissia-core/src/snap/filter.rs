//! Filters raw elements to only interactable, visible elements.

use super::extractor::RawElement;

/// Roles that indicate an interactable element.
const INTERACTABLE_ROLES: &[&str] = &[
    "button",
    "link",
    "textbox",
    "checkbox",
    "radio",
    "combobox",
    "menuitem",
    "tab",
    "switch",
    "slider",
    "searchbox",
    "option",
    "spinbutton",
    "listbox",
];

/// Tags that indicate an interactable element (when role is missing).
const INTERACTABLE_TAGS: &[&str] = &[
    "a", "button", "input", "select", "textarea", "details", "summary",
];

/// Filter raw elements to only those that are interactable and visible.
/// If `focus_bounds` is provided ([x, y, w, h]), only elements whose center
/// falls within those bounds are included.
pub fn filter_elements(
    elements: Vec<RawElement>,
    focus_bounds: Option<[f64; 4]>,
) -> Vec<RawElement> {
    elements
        .into_iter()
        .filter(|el| {
            // Focus scope: element center must be within focus bounds
            if let (Some(bounds), Some(fb)) = (el.bounds, focus_bounds) {
                let cx = bounds[0] + bounds[2] / 2.0;
                let cy = bounds[1] + bounds[3] / 2.0;
                let in_focus =
                    cx >= fb[0] && cx <= fb[0] + fb[2] && cy >= fb[1] && cy <= fb[1] + fb[3];
                if !in_focus {
                    return false;
                }
            }
            // Must be visible
            if !el.is_visible {
                return false;
            }

            // Check if aria-hidden
            if el.attributes.get("aria-hidden").map(|v| v.as_str()) == Some("true") {
                return false;
            }

            // Check role
            if !el.role.is_empty() && INTERACTABLE_ROLES.contains(&el.role.as_str()) {
                return true;
            }

            // Check tag
            if INTERACTABLE_TAGS.contains(&el.tag.as_str()) {
                return true;
            }

            // Check for contenteditable
            if el.attributes.get("contenteditable").map(|v| v.as_str()) == Some("true") {
                return true;
            }

            // Check for role attribute on div/span
            if let Some(role_attr) = el.attributes.get("role") {
                if INTERACTABLE_ROLES.contains(&role_attr.as_str()) {
                    return true;
                }
            }

            // Check for tabindex (explicitly focusable)
            if el.attributes.contains_key("tabindex") {
                return true;
            }

            false
        })
        .collect()
}
