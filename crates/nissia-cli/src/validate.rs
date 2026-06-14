//! Input validation to harden against agent hallucinations.

use anyhow::{bail, Result};

/// Validate an element reference (e.g., @e1, @e42).
pub fn element_ref(ref_id: &str) -> Result<()> {
    if !ref_id.starts_with('@') {
        bail!("Invalid element ref '{ref_id}': must start with '@' (e.g., @e1)");
    }

    let id_part = &ref_id[1..];

    if id_part.is_empty() {
        bail!("Invalid element ref '{ref_id}': empty identifier after '@'");
    }

    if !id_part.chars().all(|c| c.is_alphanumeric() || c == '_') {
        bail!(
            "Invalid element ref '{ref_id}': only alphanumeric characters and underscores allowed"
        );
    }

    if id_part.contains("..") {
        bail!("Invalid element ref '{ref_id}': path traversal not allowed");
    }

    if ref_id.chars().any(|c| (c as u32) < 0x20) {
        bail!("Invalid element ref '{ref_id}': control characters not allowed");
    }

    Ok(())
}

/// Validate a fill/type value for safety.
/// Blocks control characters and excessively long values.
pub fn fill_value(value: &str) -> Result<()> {
    if value.len() > 10_000 {
        bail!(
            "Value too long ({} chars): maximum 10,000 characters allowed",
            value.len()
        );
    }

    // Block control characters except tab (\t), newline (\n), carriage return (\r)
    if value.chars().any(|c| {
        let u = c as u32;
        u < 0x20 && u != 0x09 && u != 0x0A && u != 0x0D
    }) {
        bail!("Value contains disallowed control characters");
    }

    Ok(())
}

/// Validate a CSS selector for safety.
pub fn css_selector(selector: &str) -> Result<()> {
    if selector.is_empty() {
        bail!("Empty CSS selector");
    }

    if selector.contains("../") || selector.contains("..\\") {
        bail!("Invalid selector: path traversal patterns not allowed");
    }

    if selector.chars().any(|c| (c as u32) < 0x20) {
        bail!("Invalid selector: control characters not allowed");
    }

    // Block javascript: protocol in selectors
    if selector.to_lowercase().contains("javascript:") {
        bail!("Invalid selector: javascript: protocol not allowed");
    }

    Ok(())
}
