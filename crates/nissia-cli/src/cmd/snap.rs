use anyhow::Result;

/// Patterns that may indicate prompt injection in web page content.
const INJECTION_PATTERNS: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "forget previous instructions",
    "forget all previous",
    "you are now",
    "disregard previous",
    "override previous",
    "new instructions:",
    "system prompt:",
    "system: ",
];

/// Warn if any element labels/attributes contain potential injection patterns.
fn check_injection(result: &nissia_core::snap::SnapResult) {
    for entry in result.element_map.elements.values() {
        let fields = [entry.name.as_str(), entry.selector_hint.as_str()];
        for field in fields {
            let lower = field.to_lowercase();
            for pattern in INJECTION_PATTERNS {
                if lower.contains(pattern) {
                    eprintln!(
                        "warning: potential prompt injection detected in page content: {:?}",
                        &field[..field.len().min(120)]
                    );
                    return; // one warning is enough
                }
            }
        }
        for val in entry.attributes.values() {
            let lower = val.to_lowercase();
            for pattern in INJECTION_PATTERNS {
                if lower.contains(pattern) {
                    eprintln!(
                        "warning: potential prompt injection detected in page content: {:?}",
                        &val[..val.len().min(120)]
                    );
                    return;
                }
            }
        }
    }
}

pub async fn run(
    port: u16,
    url: Option<&str>,
    focus: Option<&str>,
    fmt: &str,
    lang: &str,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;

    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    let result = nissia_core::snap::execute(&transport, url, focus, lang, emu).await?;

    // Record step if recording is active
    if let Ok(Some(mut state)) = nissia_core::record::recorder::Recorder::load_state() {
        let mut args = std::collections::HashMap::new();
        if let Some(u) = url {
            args.insert("url".to_string(), u.to_string());
        }
        if let Some(f) = focus {
            args.insert("focus".to_string(), f.to_string());
        }
        nissia_core::record::recorder::Recorder::record_step(
            &mut state,
            "snap",
            args,
            Some(&result.element_map),
        );
        let _ = nissia_core::record::recorder::Recorder::save_state(&state);
    }

    check_injection(&result);

    match fmt {
        "json" => {
            let json = serde_json::json!({
                "elements": result.element_map.elements,
                "count": result.element_count,
            });
            println!("{}", serde_json::to_string(&json)?);
        }
        "ndjson" => {
            // Sort by numeric index for deterministic output order
            let mut entries: Vec<_> = result.element_map.elements.iter().collect();
            entries.sort_by_key(|(ref_id, _)| {
                ref_id
                    .trim_start_matches("@e")
                    .parse::<usize>()
                    .unwrap_or(usize::MAX)
            });
            for (ref_id, entry) in entries {
                let line = serde_json::json!({
                    "ref": ref_id,
                    "role": entry.role,
                    "name": entry.name,
                    "tag": entry.tag,
                    "selector_hint": entry.selector_hint,
                    "attributes": entry.attributes,
                });
                println!("{}", serde_json::to_string(&line)?);
            }
        }
        _ => {
            println!("{}", result.output);
            eprintln!("({} elements)", result.element_count);
        }
    }

    Ok(())
}
