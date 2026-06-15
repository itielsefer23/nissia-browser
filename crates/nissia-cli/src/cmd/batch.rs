//! `nissia batch` — run a whole sequence of steps in ONE process, on ONE CDP
//! connection. This is the no-API speed win: instead of the calling agent issuing
//! one command per model round-trip, it composes the full plan and runs it in a
//! single turn. Reads steps from stdin, one verb per line.
//!
//! Verbs (first word of each line; `#` lines and blank lines are ignored):
//!   goto <url>            navigate (prints "(navigated, N elements)")
//!   snap [selector]       interactable elements as @eN (focused if selector given)
//!   read [selector]       page text as markdown (focused if selector given)
//!   eval <js...>          run JS (rest of line), print result
//!   click @eN
//!   clicksel <css>        real mouse click on a CSS selector (calendar cells, grids)
//!   key <name>            press enter|tab|escape|arrowdown|... (submit, autocomplete)
//!   fill @eN <value...>
//!   type @eN <text...>
//!   typesel <css> => <text>  human-type into a CSS selector (no @eN snap needed).
//!                            Use " => " to separate (the selector may contain spaces).
//!   select @eN <value...>
//!   scroll [up|down]
//!   dismiss               close cookie/consent banners + blocking overlays
//!   reload [hard]         refresh the page and wait for DOM ready (human recovery)
//!   wait <ms>             fixed pause (use sparingly)
//!   waitfor <css>         ADAPTIVE: wait until selector appears (max 10s) — prefer this
//!   waitgone <css>        wait until selector disappears, e.g. a spinner (max 15s)
//!
//! Element refs (@eN) persist across steps within the batch. Actions do NOT auto
//! re-snap (cheap by default): add an explicit `snap`/`read`/`eval` line to observe.

use anyhow::{bail, Context, Result};
use std::io::Read;

use nissia_core::snap::EmulationOptions;

pub async fn run(port: u16, lang: &str, emu: &EmulationOptions) -> Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("failed to read batch script from stdin")?;

    super::ensure_browser(port).await?;
    let transport = nissia_cdp::connect(port).await?;
    transport.send(&nissia_cdp::commands::PageEnable {}).await?;

    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        println!(">> {line}");
        match exec_line(&transport, line, lang, emu).await {
            Ok(out) => {
                if !out.is_empty() {
                    println!("{out}");
                }
            }
            Err(e) => println!("ERROR: {e}"),
        }
    }
    Ok(())
}

fn value_to_string(v: Option<serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s,
        Some(serde_json::Value::Null) | None => "null".to_string(),
        Some(other) => other.to_string(),
    }
}

fn split2(rest: &str) -> (&str, &str) {
    match rest.split_once(char::is_whitespace) {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (rest.trim(), ""),
    }
}

async fn exec_line(
    transport: &nissia_cdp::CdpTransport,
    line: &str,
    lang: &str,
    emu: &EmulationOptions,
) -> Result<String> {
    let (verb, rest) = match line.split_once(char::is_whitespace) {
        Some((v, r)) => (v, r.trim()),
        None => (line, ""),
    };

    match verb {
        "goto" => {
            let r = nissia_core::snap::execute(transport, Some(rest), None, lang, emu).await?;
            Ok(format!("(navigated, {} elements)", r.element_count))
        }
        "snap" => {
            let focus = if rest.is_empty() { None } else { Some(rest) };
            let r = nissia_core::snap::execute(transport, None, focus, lang, emu).await?;
            Ok(r.output)
        }
        "read" => {
            let focus = if rest.is_empty() { None } else { Some(rest) };
            let r = nissia_core::read::execute(transport, None, focus, lang, 120, emu).await?;
            Ok(r.output)
        }
        "eval" => {
            let result = transport
                .send(&nissia_cdp::commands::RuntimeEvaluate {
                    expression: rest.to_string(),
                    return_by_value: Some(true),
                    await_promise: Some(true),
                    context_id: None,
                })
                .await?;
            if let Some(exc) = result.exception_details {
                bail!("JavaScript error: {:?}", exc);
            }
            Ok(value_to_string(result.result.value))
        }
        "click" => {
            nissia_core::action::click::execute(transport, rest).await?;
            Ok("ok".to_string())
        }
        "clicksel" => {
            nissia_core::action::click::execute_selector(transport, rest).await?;
            Ok("ok".to_string())
        }
        "key" => {
            nissia_core::action::key::execute(transport, rest).await?;
            Ok("ok".to_string())
        }
        "dismiss" => {
            let raw = nissia_core::action::dismiss::execute(transport).await?;
            Ok(raw)
        }
        "reload" => {
            let hard = rest.eq_ignore_ascii_case("hard");
            nissia_core::action::reload::execute(transport, hard).await?;
            Ok("ok".to_string())
        }
        "fill" => {
            let (r, v) = split2(rest);
            nissia_core::action::fill::execute(transport, r, v).await?;
            Ok("ok".to_string())
        }
        "type" => {
            let (r, v) = split2(rest);
            nissia_core::action::type_text::execute(transport, r, v).await?;
            Ok("ok".to_string())
        }
        "typesel" => {
            // The selector itself can contain spaces (e.g. [aria-label*="dónde quieres"]),
            // so split selector from text on " => " (fall back to first space only if the
            // delimiter is absent and the selector is simple).
            let (sel, v) = match rest.split_once(" => ") {
                Some((s, t)) => (s.trim(), t.trim()),
                None => split2(rest),
            };
            nissia_core::action::type_text::execute_selector(transport, sel, v).await?;
            Ok("ok".to_string())
        }
        "select" => {
            let (r, v) = split2(rest);
            nissia_core::action::select::execute(transport, r, v).await?;
            Ok("ok".to_string())
        }
        "scroll" => {
            let dir = if rest.is_empty() { "down" } else { rest };
            nissia_core::action::scroll::execute(transport, dir, None).await?;
            Ok("ok".to_string())
        }
        "wait" => {
            let ms: u64 = rest.parse().unwrap_or(500);
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            Ok(String::new())
        }
        "waitfor" => {
            nissia_core::action::wait::execute(
                transport,
                nissia_core::action::wait::WaitCondition::Selector(rest),
            )
            .await?;
            Ok("ready".to_string())
        }
        "waitgone" => {
            nissia_core::action::wait::execute(
                transport,
                nissia_core::action::wait::WaitCondition::SelectorGone(rest),
            )
            .await?;
            Ok("gone".to_string())
        }
        other => bail!("unknown batch verb '{other}'"),
    }
}
