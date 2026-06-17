//! Type action: character-by-character key dispatch.

use nissia_cdp::commands::{
    DomResolveNode, InputDispatchKeyEvent, RuntimeCallFunctionOn, RuntimeEvaluate,
};
use nissia_cdp::CdpTransport;
use serde_json::Value;

use crate::behavior;
use crate::element_map::ElementMap;

/// JS that returns true if the currently focused element is a payment-card number
/// or security-code (CVV/CVC) field. Used to hard-refuse typing card data, while
/// still allowing the use of payment methods already saved in the browser.
const FINANCIAL_FIELD_JS: &str = "(function(){var a=document.activeElement;if(!a)return false;\
var ac=(a.getAttribute('autocomplete')||'').toLowerCase();if(/cc-number|cc-csc|cc-exp/.test(ac))return true;\
var hay=((a.name||'')+' '+(a.id||'')+' '+(a.getAttribute('placeholder')||'')+' '+(a.getAttribute('aria-label')||'')).toLowerCase();\
if(/(card.?number|cardnum|creditcard|n[u\\u00fa]mero.{0,6}(tarjeta|cart[a\\u00e3]o)|\\bcvv\\b|\\bcvc\\b|security.?code|c[o\\u00f3]digo.{0,6}seguran|c[o\\u00f3]digo.{0,6}seguridad|\\bcsc\\b)/.test(hay))return true;\
return false;})()";

pub async fn execute(
    transport: &CdpTransport,
    ref_id: &str,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let map = ElementMap::load().map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to load element map: {e}"))
    })?;

    let entry = map
        .get(ref_id)
        .ok_or_else(|| nissia_cdp::CdpTransportError::CommandFailed {
            method: "type".into(),
            code: -1,
            message: format!("Element {ref_id} not found. Run `nissia snap` first."),
        })?;

    // Focus the element
    let resolved = transport
        .send(&DomResolveNode {
            node_id: None,
            backend_node_id: Some(entry.backend_node_id),
            object_group: Some("nissia".to_string()),
        })
        .await?;

    if let Some(object_id) = &resolved.object.object_id {
        transport
            .send(&RuntimeCallFunctionOn {
                function_declaration: "function() { this.focus(); }".to_string(),
                object_id: Some(object_id.clone()),
                arguments: None,
                return_by_value: Some(true),
                await_promise: None,
            })
            .await?;
    }

    type_chars(transport, text).await
}

/// Type into the first element matching a CSS selector (focus + clear + type),
/// without needing an `@eN` snapshot ref. Makes `batch` flows self-contained for
/// form fields (search boxes, autocomplete inputs).
pub async fn execute_selector(
    transport: &CdpTransport,
    selector: &str,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let sel_json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    // Pick the VISIBLE match via an elementFromPoint hit-test (responsive sites
    // often render a hidden duplicate of the field earlier in the DOM; focusing
    // that one types into nowhere). Same reliable picker as `click --sel`.
    let focus_js = format!(
        "(function(){{\
var els=Array.from(document.querySelectorAll({sel_json}));\
if(!els.length)return 'notfound';\
var vw=window.innerWidth,vh=window.innerHeight;\
function hit(e){{var r=e.getBoundingClientRect();if(r.width===0&&r.height===0)return false;var cx=r.left+r.width/2,cy=r.top+r.height/2;if(cx<0||cx>vw||cy<0||cy>vh)return false;var t=document.elementFromPoint(cx,cy);return !!t&&(t===e||e.contains(t)||t.contains(e));}}\
var act=document.activeElement;\
var el=(act&&els.indexOf(act)>=0)?act:(els.find(hit)||els.find(function(e){{var r=e.getBoundingClientRect();return r.width>0&&r.height>0&&e.offsetParent!==null;}})||els[0]);\
if(el!==document.activeElement){{el.focus();}}try{{if(el.select)el.select();}}catch(e){{}}return 'ok';\
}})()"
    );
    let r = transport
        .send(&RuntimeEvaluate {
            expression: focus_js,
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    if let Some(Value::String(s)) = r.result.value {
        if s == "notfound" {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "type".into(),
                code: -1,
                message: format!("No element matches selector: {selector}"),
            });
        }
    }
    type_chars(transport, text).await
}

/// Type into whatever element is currently focused (document.activeElement),
/// without a selector. Use after a `click`/`clicksel` that opened an overlay /
/// proxy search input (Booking, Wikipedia, Google Flights...): the overlay input
/// often has a different element than the bar input, so a selector-based type
/// misses it, but it IS the focused one. Selects existing text first so we replace.
pub async fn execute_active(
    transport: &CdpTransport,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let r = transport
        .send(&RuntimeEvaluate {
            expression: "(function(){var a=document.activeElement;if(!a)return 'none';var ed=a.tagName==='INPUT'||a.tagName==='TEXTAREA'||a.isContentEditable;if(!ed)return 'notedit';try{if(a.select)a.select();}catch(e){}return 'ok';})()".to_string(),
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    if let Some(Value::String(s)) = r.result.value {
        if s == "none" || s == "notedit" {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "typeactive".into(),
                code: -1,
                message: "no editable element is focused (click the field first)".into(),
            });
        }
    }
    type_chars(transport, text).await
}

/// Human-paced character typing into the currently focused element: variable
/// (lognormal-ish) inter-key cadence, per-key hold, longer pauses after spaces /
/// punctuation, an occasional "thinking" pause, and a rare typo→backspace→
/// correct. Keystroke dynamics are biometric-grade signals, so constant delays
/// read as a bot. Scales with the active pace (Fast → near-instant). Native → 0 tokens.
async fn type_chars(
    transport: &CdpTransport,
    text: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    // SAFETY HARD GUARD: never type into a payment-card / CVV field. Buying with a
    // payment method ALREADY saved in the browser is allowed (with the user's
    // confirmation), but ENTERING card numbers / security codes is prohibited —
    // refuse at the binary level so no agent can do it, and tell the user to enter
    // those themselves.
    let guard = transport
        .send(&RuntimeEvaluate {
            expression: FINANCIAL_FIELD_JS.to_string(),
            return_by_value: Some(true),
            await_promise: Some(false),
            context_id: None,
        })
        .await?;
    if let Some(Value::Bool(true)) = guard.result.value {
        return Err(nissia_cdp::CdpTransportError::CommandFailed {
            method: "type".into(),
            code: -1,
            message: "refusing to type into a payment-card / CVV field — entering card details is not allowed; ask the user to enter it themselves (use only payment methods already saved)".into(),
        });
    }
    let f = behavior::Pace::current().factor();
    let mut seed = behavior::rng_seed();
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    for (idx, &ch) in chars.iter().enumerate() {
        // Rare human typo + correction (~2%), human paces only, mid-word, letters only.
        if f > 0.0 && idx > 0 && idx + 1 < n && ch.is_alphabetic() && behavior::rand01(&mut seed) < 0.02
        {
            let wrong = typo_char(&mut seed, ch);
            press_char(transport, wrong, f, &mut seed).await?;
            behavior::pause(130).await;
            press_backspace(transport).await?;
            behavior::pause(95).await;
        }
        press_char(transport, ch, f, &mut seed).await?;
        if f > 0.0 {
            let base = behavior::gauss(&mut seed, 110.0, 45.0).clamp(40.0, 280.0);
            let extra = if ch == ' ' || ".,;:!?".contains(ch) {
                90.0
            } else {
                0.0
            };
            // rare "thinking" pause mid-query
            let think = if behavior::rand01(&mut seed) < 0.06 {
                250.0 + behavior::rand01(&mut seed) * 400.0
            } else {
                0.0
            };
            let ms = ((base + extra + think) * f) as u64;
            if ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }
        }
    }
    Ok(())
}

/// Press one character key (keyDown → human hold → keyUp).
async fn press_char(
    transport: &CdpTransport,
    ch: char,
    f: f64,
    seed: &mut u64,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let s = ch.to_string();
    transport
        .send(&InputDispatchKeyEvent {
            event_type: "keyDown".to_string(),
            key: Some(s.clone()),
            text: Some(s.clone()),
            unmodified_text: Some(s.clone()),
            code: None,
            windows_virtual_key_code: None,
            native_virtual_key_code: None,
        })
        .await?;
    if f > 0.0 {
        let hold = behavior::gauss(seed, 45.0, 16.0).clamp(15.0, 110.0) as u64;
        tokio::time::sleep(std::time::Duration::from_millis(hold)).await;
    }
    transport
        .send(&InputDispatchKeyEvent {
            event_type: "keyUp".to_string(),
            key: Some(s),
            text: None,
            unmodified_text: None,
            code: None,
            windows_virtual_key_code: None,
            native_virtual_key_code: None,
        })
        .await?;
    Ok(())
}

/// Press Backspace (proper VK 8) to correct a simulated typo.
async fn press_backspace(transport: &CdpTransport) -> Result<(), nissia_cdp::CdpTransportError> {
    for et in ["keyDown", "keyUp"] {
        transport
            .send(&InputDispatchKeyEvent {
                event_type: et.to_string(),
                key: Some("Backspace".to_string()),
                text: None,
                unmodified_text: None,
                code: Some("Backspace".to_string()),
                windows_virtual_key_code: Some(8),
                native_virtual_key_code: Some(8),
            })
            .await?;
    }
    Ok(())
}

/// A plausible wrong key for `ch` (a random lowercase letter, never equal to ch).
fn typo_char(seed: &mut u64, ch: char) -> char {
    let letters = b"abcdefghijklmnopqrstuvwxyz";
    let c = letters[((behavior::rand01(seed) * 26.0) as usize) % 26] as char;
    if c == ch {
        if ch == 'a' {
            's'
        } else {
            'a'
        }
    } else {
        c
    }
}
