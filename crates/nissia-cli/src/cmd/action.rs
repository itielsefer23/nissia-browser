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
    let js = r#"(function(){var clicked=[];var sels=['#onetrust-accept-btn-handler','#didomi-notice-agree-button','.fc-cta-consent','.fc-button-consent','#sp-cc-accept','.qc-cmp2-summary-buttons button','button[mode=primary]','[data-testid=accept-button]','[aria-label*=accept i]','[aria-label*=aceptar i]','.cc-allow','.cc-dismiss','#cookie-accept','.cookie-accept'];function docs(){var a=[document];try{[].forEach.call(document.querySelectorAll('iframe'),function(f){try{if(f.contentDocument)a.push(f.contentDocument);}catch(e){}});}catch(e){}return a;}var rx=/^(aceptar|aceptar todo|aceptar y cerrar|acepto|accept|accept all|i accept|aceitar|aceitar todos|agree|i agree|consent|got it|entendido|de acuerdo|allow all|permitir|continuar|ok)$/i;docs().forEach(function(d){sels.forEach(function(se){try{var b=d.querySelector(se);if(b){b.click();clicked.push(se);}}catch(e){}});try{var bs=[].slice.call(d.querySelectorAll('button,[role=button],a'));for(var i=0;i<bs.length;i++){var t=((bs[i].innerText||bs[i].textContent||'')+'').trim();if(t.length<25&&rx.test(t)){bs[i].click();clicked.push(t);break;}}}catch(e){}});var removed=0;var vw=window.innerWidth;var vh=window.innerHeight;[].slice.call(document.querySelectorAll('div,section,aside,iframe,dialog')).forEach(function(e){try{var s=getComputedStyle(e);if(s.display==='none')return;var pos=s.position;var z=parseInt(s.zIndex)||0;var r=e.getBoundingClientRect();var big=r.height>60&&r.width>200;var covers=r.width>=vw*0.6&&r.height>=vh*0.5;var id=((e.id||'')+' '+(e.className||'')+'').toLowerCase();var bn=/cookie|consent|gdpr|cmp|banner|modal|overlay|popup|paywall|newsletter|subscrib|interstitial|backdrop|sp_message|qc-cmp|didomi|onetrust|truste|outbrain|taboola|sponsor|promoted/.test(id);var fixedish=(pos==='fixed'||pos==='sticky');if((fixedish&&big&&z>=40)||(bn&&big)||((fixedish||pos==='absolute')&&covers&&z>=10)){e.remove();removed++;}}catch(_){}});try{document.documentElement.style.overflow='auto';if(document.body){document.body.style.overflow='auto';}}catch(e){}return JSON.stringify({accepted: clicked.length? clicked.slice(0,3).join(', '): null, removed: removed});})()"#;
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

/// Press a single key (enter, tab, escape, arrowdown, etc.) as a real key event.
/// Lets the agent submit searches (enter), move fields (tab), or pick autocomplete
/// suggestions (arrowdown + enter), like a human.
pub async fn run_key(port: u16, key_name: &str, fmt: &str) -> Result<()> {
    let transport = nissia_cdp::connect(port).await?;
    let (key, code, vk, text): (&str, &str, i64, Option<&str>) = match key_name.to_lowercase().as_str() {
        "enter" | "return" => ("Enter", "Enter", 13, None),
        "tab" => ("Tab", "Tab", 9, None),
        "escape" | "esc" => ("Escape", "Escape", 27, None),
        "backspace" => ("Backspace", "Backspace", 8, None),
        "delete" | "del" => ("Delete", "Delete", 46, None),
        "arrowdown" | "down" => ("ArrowDown", "ArrowDown", 40, None),
        "arrowup" | "up" => ("ArrowUp", "ArrowUp", 38, None),
        "arrowleft" | "left" => ("ArrowLeft", "ArrowLeft", 37, None),
        "arrowright" | "right" => ("ArrowRight", "ArrowRight", 39, None),
        "space" => (" ", "Space", 32, Some(" ")),
        "pagedown" => ("PageDown", "PageDown", 34, None),
        "pageup" => ("PageUp", "PageUp", 33, None),
        "home" => ("Home", "Home", 36, None),
        "end" => ("End", "End", 35, None),
        other => anyhow::bail!("unknown key {other:?} (enter|tab|escape|backspace|delete|arrowup|arrowdown|arrowleft|arrowright|space|pageup|pagedown|home|end)"),
    };
    let down = nissia_cdp::commands::InputDispatchKeyEvent {
        event_type: "keyDown".to_string(),
        key: Some(key.to_string()),
        text: text.map(|t| t.to_string()),
        unmodified_text: text.map(|t| t.to_string()),
        code: Some(code.to_string()),
        windows_virtual_key_code: Some(vk),
        native_virtual_key_code: Some(vk),
    };
    transport.send(&down).await?;
    let up = nissia_cdp::commands::InputDispatchKeyEvent {
        event_type: "keyUp".to_string(),
        key: Some(key.to_string()),
        text: None,
        unmodified_text: None,
        code: Some(code.to_string()),
        windows_virtual_key_code: Some(vk),
        native_virtual_key_code: Some(vk),
    };
    transport.send(&up).await?;
    maybe_record("key", HashMap::from([("key".into(), key_name.into())]));
    ok(fmt, "key", None);
    Ok(())
}
