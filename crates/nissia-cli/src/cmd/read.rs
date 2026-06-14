use anyhow::Result;

pub async fn run(
    port: u16,
    url: Option<&str>,
    focus: Option<&str>,
    fmt: &str,
    lang: &str,
    max_lines: usize,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;

    let result = nissia_core::read::execute(&transport, url, focus, lang, max_lines, emu).await?;

    // Record step if recording is active
    if let Ok(Some(mut state)) = nissia_core::record::recorder::Recorder::load_state() {
        let mut args = std::collections::HashMap::new();
        if let Some(u) = url {
            args.insert("url".to_string(), u.to_string());
        }
        if let Some(f) = focus {
            args.insert("focus".to_string(), f.to_string());
        }
        nissia_core::record::recorder::Recorder::record_step(&mut state, "read", args, None);
        let _ = nissia_core::record::recorder::Recorder::save_state(&state);
    }

    match fmt {
        "json" => {
            let json = serde_json::json!({
                "content": result.output,
                "line_count": result.line_count,
            });
            println!("{}", serde_json::to_string(&json)?);
        }
        _ => {
            println!("{}", result.output);
            eprintln!("({} lines)", result.line_count);
        }
    }

    Ok(())
}
