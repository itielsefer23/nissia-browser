use anyhow::Result;
use std::collections::HashMap;

/// Record a command step if recording is active.
fn maybe_record(command: &str, args: HashMap<String, String>) {
    if let Ok(Some(mut state)) = nissia_core::record::recorder::Recorder::load_state() {
        nissia_core::record::recorder::Recorder::record_step(&mut state, command, args, None);
        let _ = nissia_core::record::recorder::Recorder::save_state(&state);
    }
}

fn ok(fmt: &str, action: &str, extra: Option<(&str, &str)>) {
    if fmt == "json" {
        let mut obj = serde_json::json!({"status": "ok", "action": action});
        if let Some((k, v)) = extra {
            obj[k] = serde_json::Value::String(v.to_string());
        }
        println!("{}", obj);
    } else if let Some((_, v)) = extra {
        println!("{v}");
    } else {
        println!("ok");
    }
}

fn dry(fmt: &str, action: &str, args: serde_json::Value) {
    if fmt == "json" {
        println!(
            "{}",
            serde_json::json!({"status": "dry_run", "action": action, "args": args})
        );
    } else {
        println!("[dry-run] {action} {args}");
    }
}

/// Print action result with optional auto re-snap output.
async fn ok_with_snap(
    transport: &nissia_cdp::CdpTransport,
    fmt: &str,
    action: &str,
    lang: &str,
    no_snap: bool,
    emu: &nissia_core::snap::EmulationOptions,
) {
    if no_snap {
        ok(fmt, action, None);
        return;
    }

    // Enable page events for settle detection
    let _ = transport.send(&nissia_cdp::commands::PageEnable {}).await;

    if let Some(snap) = nissia_core::action::post_action_snap(transport, lang, emu).await {
        if fmt == "json" {
            let json = serde_json::json!({
                "status": "ok",
                "action": action,
                "snap": {
                    "output": snap.output,
                    "count": snap.element_count,
                }
            });
            println!("{}", json);
        } else {
            println!("ok\n---\n{}", snap.output);
            eprintln!("({} elements)", snap.element_count);
        }
    } else {
        ok(fmt, action, None);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_click(
    port: u16,
    ref_id: &str,
    fmt: &str,
    dry_run: bool,
    no_snap: bool,
    lang: &str,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    if dry_run {
        dry(fmt, "click", serde_json::json!({"ref": ref_id}));
        return Ok(());
    }
    let transport = nissia_cdp::connect(port).await?;
    nissia_core::action::click::execute(&transport, ref_id).await?;
    maybe_record("click", HashMap::from([("ref".into(), ref_id.into())]));
    ok_with_snap(&transport, fmt, "click", lang, no_snap, emu).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_fill(
    port: u16,
    ref_id: &str,
    value: &str,
    fmt: &str,
    dry_run: bool,
    no_snap: bool,
    lang: &str,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    if dry_run {
        dry(
            fmt,
            "fill",
            serde_json::json!({"ref": ref_id, "value": value}),
        );
        return Ok(());
    }
    let transport = nissia_cdp::connect(port).await?;
    nissia_core::action::fill::execute(&transport, ref_id, value).await?;
    maybe_record(
        "fill",
        HashMap::from([
            ("ref".into(), ref_id.into()),
            ("value".into(), value.into()),
        ]),
    );
    ok_with_snap(&transport, fmt, "fill", lang, no_snap, emu).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_type(
    port: u16,
    ref_id: &str,
    text: &str,
    fmt: &str,
    dry_run: bool,
    no_snap: bool,
    lang: &str,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    if dry_run {
        dry(
            fmt,
            "type",
            serde_json::json!({"ref": ref_id, "text": text}),
        );
        return Ok(());
    }
    let transport = nissia_cdp::connect(port).await?;
    nissia_core::action::type_text::execute(&transport, ref_id, text).await?;
    maybe_record(
        "type",
        HashMap::from([("ref".into(), ref_id.into()), ("text".into(), text.into())]),
    );
    ok_with_snap(&transport, fmt, "type", lang, no_snap, emu).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_select(
    port: u16,
    ref_id: &str,
    value: &str,
    fmt: &str,
    dry_run: bool,
    no_snap: bool,
    lang: &str,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    if dry_run {
        dry(
            fmt,
            "select",
            serde_json::json!({"ref": ref_id, "value": value}),
        );
        return Ok(());
    }
    let transport = nissia_cdp::connect(port).await?;
    nissia_core::action::select::execute(&transport, ref_id, value).await?;
    maybe_record(
        "select",
        HashMap::from([
            ("ref".into(), ref_id.into()),
            ("value".into(), value.into()),
        ]),
    );
    ok_with_snap(&transport, fmt, "select", lang, no_snap, emu).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn run_scroll(
    port: u16,
    direction: &str,
    amount: Option<i64>,
    fmt: &str,
    dry_run: bool,
    no_snap: bool,
    lang: &str,
    emu: &nissia_core::snap::EmulationOptions,
) -> Result<()> {
    if dry_run {
        dry(
            fmt,
            "scroll",
            serde_json::json!({"direction": direction, "amount": amount}),
        );
        return Ok(());
    }
    let transport = nissia_cdp::connect(port).await?;
    nissia_core::action::scroll::execute(&transport, direction, amount).await?;
    let mut scroll_args = HashMap::from([("direction".into(), direction.into())]);
    if let Some(a) = amount {
        scroll_args.insert("amount".into(), a.to_string());
    }
    maybe_record("scroll", scroll_args);
    ok_with_snap(&transport, fmt, "scroll", lang, no_snap, emu).await;
    Ok(())
}

pub async fn run_screenshot(port: u16, output: Option<&str>, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;
    let path = nissia_core::action::screenshot::execute(&transport, output).await?;
    let mut args = HashMap::new();
    if let Some(o) = output {
        args.insert("file".into(), o.into());
    }
    maybe_record("screenshot", args);
    ok(fmt, "screenshot", Some(("path", &path)));
    Ok(())
}

pub async fn run_wait(port: u16, condition: &str, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;

    let wait_condition = if condition == "navigation" {
        nissia_core::action::wait::WaitCondition::Navigation
    } else if let Ok(ms) = condition.parse::<u64>() {
        nissia_core::action::wait::WaitCondition::Timeout(ms)
    } else {
        nissia_core::action::wait::WaitCondition::Selector(condition)
    };

    nissia_core::action::wait::execute(&transport, wait_condition).await?;
    maybe_record(
        "wait",
        HashMap::from([("condition".into(), condition.into())]),
    );
    ok(fmt, "wait", None);
    Ok(())
}

pub async fn run_eval(port: u16, expression: &str, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;
    let result = transport
        .send(&nissia_cdp::commands::RuntimeEvaluate {
            expression: expression.to_string(),
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;

    if let Some(exc) = result.exception_details {
        anyhow::bail!("JavaScript error: {:?}", exc);
    }

    let value = result.result.value.unwrap_or(serde_json::Value::Null);

    maybe_record(
        "eval",
        HashMap::from([("expression".into(), expression.into())]),
    );

    if fmt == "json" {
        println!("{}", serde_json::to_string(&value)?);
    } else {
        match &value {
            serde_json::Value::String(s) => println!("{s}"),
            serde_json::Value::Null => println!("undefined"),
            other => println!("{}", serde_json::to_string_pretty(other)?),
        }
    }
    Ok(())
}

/// Close cookie/consent banners and remove blocking fixed overlays so the page can be read.
pub async fn run_dismiss(port: u16, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;
    let js = r#"(function(){var c=null;var rx=/^(aceptar|aceptar todo|aceptar y cerrar|accept|accept all|aceitar|aceitar todos|i agree|agree|consent|got it|entendido|de acuerdo|allow all|permitir|continuar)$/i;var b=[].slice.call(document.querySelectorAll('button,[role=button],a,input[type=button],input[type=submit]'));for(var i=0;i<b.length;i++){var t=((b[i].innerText||b[i].value||'')+'').trim();if(rx.test(t)){try{b[i].click();c=t;}catch(e){}break;}}var r=0;[].slice.call(document.querySelectorAll('div,section,aside,iframe')).forEach(function(e){try{var s=getComputedStyle(e);var fx=(s.position==='fixed'||s.position==='sticky');var big=e.offsetHeight>60&&e.offsetWidth>200;var z=parseInt(s.zIndex)||0;var id=((e.id||'')+' '+(e.className||'')+'').toLowerCase();var bn=/cookie|consent|gdpr|banner|modal|overlay|popup|paywall|newsletter|subscrib|interstitial|backdrop/.test(id);if((fx&&big&&z>=50)||(bn&&big)){e.remove();r++;}}catch(_){}});try{document.documentElement.style.overflow='auto';document.body.style.overflow='auto';}catch(e){}return JSON.stringify({accepted:c,removed:r});})()"#;
    let result = transport
        .send(&nissia_cdp::commands::RuntimeEvaluate {
            expression: js.to_string(),
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    let val = result.result.value.unwrap_or(serde_json::Value::Null);
    let raw = match &val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if fmt == "json" {
        println!("{raw}");
    } else {
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));
        let removed = parsed["removed"].as_i64().unwrap_or(0);
        match parsed["accepted"].as_str() {
            Some(a) => println!("dismissed {removed} overlays (accepted: {a})"),
            None => println!("dismissed {removed} overlays"),
        }
    }
    Ok(())
}
