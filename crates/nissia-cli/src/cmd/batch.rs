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
//!   fill @eN <value...>
//!   type @eN <text...>
//!   select @eN <value...>
//!   scroll [up|down]
//!   wait <ms>
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
        other => bail!("unknown batch verb '{other}'"),
    }
}
