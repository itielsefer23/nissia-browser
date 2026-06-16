//! Click action: resolve @eN reference, compute center coordinates, dispatch mouse events.

use nissia_cdp::commands::{DomGetBoxModel, InputDispatchMouseEvent, RuntimeEvaluate};
use nissia_cdp::CdpTransport;
use serde_json::Value;

use crate::element_map::ElementMap;

pub async fn execute(
    transport: &CdpTransport,
    ref_id: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let map = ElementMap::load().map_err(|e| {
        nissia_cdp::CdpTransportError::ConnectionFailed(format!("Failed to load element map: {e}"))
    })?;

    let entry = map
        .get(ref_id)
        .ok_or_else(|| nissia_cdp::CdpTransportError::CommandFailed {
            method: "click".into(),
            code: -1,
            message: format!("Element {ref_id} not found. Run `nissia snap` first."),
        })?;

    // Get box model to find center coordinates
    let box_model = transport
        .send(&DomGetBoxModel {
            node_id: None,
            backend_node_id: Some(entry.backend_node_id),
        })
        .await?;

    let (cx, cy) = compute_center(&box_model.model.content);
    human_click_at(transport, cx, cy).await?;
    Ok(())
}

/// Click the first VISIBLE element matching a CSS selector with a real (trusted)
/// mouse click. Unlike `execute`, this does NOT need the element to be in the
/// snapshot index — essential for calendar day cells, custom grids and widgets
/// that the accessibility snapshot does not surface. Scrolls the element into
/// view first.
///
/// Coordinates come from `getBoundingClientRect` of the visible match (viewport
/// CSS pixels), which is exactly the space `Input.dispatchMouseEvent` expects.
/// We deliberately do NOT use `DOM.querySelectorAll` + `getBoxModel` here: many
/// responsive sites (e.g. Google Flights) render a hidden duplicate of a widget
/// earlier in the DOM, and `querySelectorAll` returns that hidden node, so the
/// click lands nowhere. Picking the visible match in JS avoids that trap.
pub async fn execute_selector(
    transport: &CdpTransport,
    selector: &str,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let sel_json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());

    // Resolve the visible match, MARK it with a data attribute, scroll it into
    // view, and return its viewport center. Among multiple matches we pick the
    // first that is actually rendered (non-zero box and hittable), so hidden
    // responsive duplicates are skipped. Marking lets us re-measure the *same*
    // element later by selector, which is how we survive layout shift on
    // lazy-loading SPAs (the box moves as images stream in below the fold).
    let js_resolve = format!(
        "(function(){{\
try{{document.querySelectorAll('[data-nzc]').forEach(function(e){{e.removeAttribute('data-nzc');}});}}catch(_){{}}\
var els=Array.from(document.querySelectorAll({sel_json}));\
if(!els.length)return 'notfound';\
var vw=window.innerWidth,vh=window.innerHeight;\
function center(e){{var b=e.getBoundingClientRect();return {{e:e,b:b,cx:b.left+b.width/2,cy:b.top+b.height/2}};}}\
function hittable(o){{if(o.b.width===0&&o.b.height===0)return false;if(o.cx<0||o.cx>vw||o.cy<0||o.cy>vh)return false;var t=document.elementFromPoint(o.cx,o.cy);return !!t&&(t===o.e||o.e.contains(t));}}\
var cand=els.map(center);\
var chosen=cand.find(hittable);\
if(!chosen){{var first=els.find(function(e){{return e.offsetParent!==null;}})||els[0];first.scrollIntoView({{block:'center',inline:'center'}});chosen=center(first);}}\
if(chosen.b.width===0&&chosen.b.height===0)return 'hidden';\
try{{chosen.e.setAttribute('data-nzc','1');}}catch(_){{}}\
return JSON.stringify([chosen.cx,chosen.cy]);\
}})()"
    );

    let r = transport
        .send(&RuntimeEvaluate {
            expression: js_resolve,
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;

    match r.result.value {
        Some(Value::String(ref s)) if s == "notfound" => {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "click".into(),
                code: -1,
                message: format!("No element matches selector: {selector}"),
            });
        }
        Some(Value::String(ref s)) if s == "hidden" => {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "click".into(),
                code: -1,
                message: format!("Element for selector {selector} is not visible"),
            });
        }
        Some(Value::String(_)) => {}
        _ => {
            return Err(nissia_cdp::CdpTransportError::CommandFailed {
                method: "click".into(),
                code: -1,
                message: format!("Could not locate selector: {selector}"),
            });
        }
    }

    // Wait for the marked element's position to STABILIZE. News/e-commerce SPAs
    // stream content in after a scroll, shifting the target's box (sometimes by
    // thousands of px when scrolling up a long virtualized feed). A fixed delay
    // can't win that race, so we poll until the box stops moving, re-scrolling it
    // into view on each tick if it drifts off-screen.
    let (mut cx, mut cy) = settle_marked(transport).await.ok_or_else(|| {
        nissia_cdp::CdpTransportError::CommandFailed {
            method: "click".into(),
            code: -1,
            message: format!("Element for selector {selector} vanished before click"),
        }
    })?;

    // Move the human cursor toward the target, then VERIFY it is still under the
    // pointer (layout may have shifted during the ~200ms trajectory). If it
    // moved, re-measure once and glide to the corrected spot before clicking.
    human_move(transport, cx, cy).await?;
    tokio::time::sleep(std::time::Duration::from_millis(110)).await;
    if let Some((ok, nx, ny)) = verify_marked(transport).await {
        if !ok && (nx - cx).abs() + (ny - cy).abs() > 4.0 {
            cx = nx;
            cy = ny;
            human_move(transport, cx, cy).await?;
        }
    }

    if std::env::var("NISSIA_DEBUG_CLICK").is_ok() {
        eprintln!("[debug] selector visible center = ({cx:.1}, {cy:.1})");
    }
    human_click_at(transport, cx, cy).await?;

    // Best-effort cleanup of the marker so it never leaks into the page.
    let _ = transport
        .send(&RuntimeEvaluate {
            expression: "(function(){try{var e=document.querySelector('[data-nzc]');if(e)e.removeAttribute('data-nzc');}catch(_){}return 1;})()".to_string(),
            return_by_value: Some(true),
            await_promise: Some(false),
            context_id: None,
        })
        .await;
    Ok(())
}

/// Poll the marked element (`[data-nzc]`) until its viewport position stops
/// moving (lazy content settled), re-scrolling it into view on each tick if it
/// has drifted off-screen. Returns the stabilized viewport center. Caps at
/// ~1.6s so a perpetually-animating page still proceeds with its last reading.
async fn settle_marked(transport: &CdpTransport) -> Option<(f64, f64)> {
    let js = "(function(){var e=document.querySelector('[data-nzc]');if(!e)return 'gone';\
var b=e.getBoundingClientRect();var cx=b.left+b.width/2,cy=b.top+b.height/2;\
if(cy<0||cy>window.innerHeight||cx<0||cx>window.innerWidth){e.scrollIntoView({block:'center',inline:'center'});b=e.getBoundingClientRect();cx=b.left+b.width/2;cy=b.top+b.height/2;}\
return JSON.stringify([cx,cy]);})()";
    let mut last = f64::NAN;
    let mut out: Option<(f64, f64)> = None;
    for i in 0..10 {
        let r = transport
            .send(&RuntimeEvaluate {
                expression: js.to_string(),
                return_by_value: Some(true),
                await_promise: Some(false),
                context_id: None,
            })
            .await
            .ok()?;
        let (cx, cy) = match r.result.value {
            Some(Value::String(ref s)) if s != "gone" => {
                let v: Vec<f64> = serde_json::from_str(s).ok()?;
                if v.len() == 2 {
                    (v[0], v[1])
                } else {
                    return out;
                }
            }
            _ => return out,
        };
        out = Some((cx, cy));
        if i > 0 && (cy - last).abs() < 2.0 {
            return out;
        }
        last = cy;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    out
}

/// After the cursor has moved, check whether the marked element is still under
/// its own center point (it may have shifted during the trajectory). Returns
/// `(still_hittable, fresh_cx, fresh_cy)`.
async fn verify_marked(transport: &CdpTransport) -> Option<(bool, f64, f64)> {
    let r = transport
        .send(&RuntimeEvaluate {
            expression: "(function(){var e=document.querySelector('[data-nzc]');if(!e)return 'gone';var b=e.getBoundingClientRect();var cx=b.left+b.width/2,cy=b.top+b.height/2;var t=document.elementFromPoint(cx,cy);var ok=!!t&&(t===e||e.contains(t));return JSON.stringify([ok?1:0,cx,cy]);})()".to_string(),
            return_by_value: Some(true),
            await_promise: Some(false),
            context_id: None,
        })
        .await
        .ok()?;
    match r.result.value {
        Some(Value::String(ref s)) if s != "gone" => {
            let v: Vec<f64> = serde_json::from_str(s).ok()?;
            if v.len() == 3 {
                Some((v[0] != 0.0, v[1], v[2]))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Dispatch a trusted left click at viewport coordinates, preceded by a HUMAN
/// mouse trajectory (curved Bézier path, eased velocity, jitter, micro-adjust).
/// This is what anti-bot systems look at: a click with no prior mouse movement,
/// or a straight teleport, is an instant tell. All of this runs inside the binary
/// (native CDP calls), so it costs zero tokens and ~100-180ms.
pub async fn human_click_at(
    transport: &CdpTransport,
    x: f64,
    y: f64,
) -> Result<(), nissia_cdp::CdpTransportError> {
    human_move(transport, x, y).await?;
    // settle, then a tiny micro-adjustment (humans rarely land dead-center)
    let mut seed = rng_seed();
    tokio::time::sleep(std::time::Duration::from_millis(50 + (seed >> 33) % 80)).await;
    let (mx, my) = (
        x + (rand01(&mut seed) - 0.5) * 2.0,
        y + (rand01(&mut seed) - 0.5) * 2.0,
    );
    let _ = transport
        .send(&InputDispatchMouseEvent {
            event_type: "mouseMoved".to_string(),
            x: mx,
            y: my,
            button: None,
            click_count: None,
            ..Default::default()
        })
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(20 + (seed >> 29) % 40)).await;

    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mousePressed".to_string(),
            x: mx,
            y: my,
            button: Some("left".to_string()),
            click_count: Some(1),
            ..Default::default()
        })
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(40 + (seed >> 25) % 70)).await;

    transport
        .send(&InputDispatchMouseEvent {
            event_type: "mouseReleased".to_string(),
            x: mx,
            y: my,
            button: Some("left".to_string()),
            click_count: Some(1),
            ..Default::default()
        })
        .await?;

    save_cursor(mx, my);
    Ok(())
}

/// Move the mouse from its last known position to (tx,ty) along a curved,
/// human-like path: cubic Bézier with random control points, ease-in-out
/// velocity (slow→fast→slow, à la Fitts's law) and small per-step jitter.
pub async fn human_move(
    transport: &CdpTransport,
    tx: f64,
    ty: f64,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let (sx, sy) = load_cursor();
    let mut seed = rng_seed();
    let dist = ((tx - sx).powi(2) + (ty - sy).powi(2)).sqrt();
    if dist < 3.0 {
        return Ok(());
    }
    // Control points: along the line at ~30%/70% plus a perpendicular wobble.
    let (dx, dy) = (tx - sx, ty - sy);
    let (px, py) = (-dy / dist, dx / dist); // unit perpendicular
    let wobble = (dist * 0.18).min(90.0);
    let off1 = (rand01(&mut seed) - 0.5) * 2.0 * wobble;
    let off2 = (rand01(&mut seed) - 0.5) * 2.0 * wobble;
    let c1 = (sx + dx * 0.30 + px * off1, sy + dy * 0.30 + py * off1);
    let c2 = (sx + dx * 0.70 + px * off2, sy + dy * 0.70 + py * off2);

    let steps = (dist / 8.0).clamp(14.0, 32.0) as u32;
    for i in 1..=steps {
        let t_lin = i as f64 / steps as f64;
        // smoothstep easing → slow at the ends, fast in the middle
        let t = t_lin * t_lin * (3.0 - 2.0 * t_lin);
        let mt = 1.0 - t;
        // cubic Bézier B(t)
        let bx = mt * mt * mt * sx
            + 3.0 * mt * mt * t * c1.0
            + 3.0 * mt * t * t * c2.0
            + t * t * t * tx;
        let by = mt * mt * mt * sy
            + 3.0 * mt * mt * t * c1.1
            + 3.0 * mt * t * t * c2.1
            + t * t * t * ty;
        let jx = (rand01(&mut seed) - 0.5) * 1.4;
        let jy = (rand01(&mut seed) - 0.5) * 1.4;
        transport
            .send(&InputDispatchMouseEvent {
                event_type: "mouseMoved".to_string(),
                x: bx + jx,
                y: by + jy,
                button: None,
                click_count: None,
                ..Default::default()
            })
            .await?;
        // faster in the middle, slower at the ends (1 - speed)
        let speed = 1.0 - (2.0 * t_lin - 1.0).abs(); // 0..1..0
        let delay = 3 + ((1.0 - speed) * 9.0) as u64;
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
    save_cursor(tx, ty);
    Ok(())
}

fn rng_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1
}

/// LCG → pseudo-random f64 in [0,1).
fn rand01(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 33) as f64) / (1u64 << 31) as f64
}

fn cursor_file() -> std::path::PathBuf {
    crate::data_dir().join("cursor.json")
}

/// Last known cursor position (so trajectories start where the pointer "is").
fn load_cursor() -> (f64, f64) {
    if let Ok(t) = std::fs::read_to_string(cursor_file()) {
        if let Ok(v) = serde_json::from_str::<Value>(&t) {
            if let (Some(x), Some(y)) = (v["x"].as_f64(), v["y"].as_f64()) {
                return (x, y);
            }
        }
    }
    (180.0, 180.0)
}

fn save_cursor(x: f64, y: f64) {
    let _ = std::fs::write(cursor_file(), serde_json::json!({"x": x, "y": y}).to_string());
}

/// Compute center point from a content quad (8 values: 4 x,y pairs).
fn compute_center(quad: &[f64]) -> (f64, f64) {
    if quad.len() >= 8 {
        let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
        let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;
        (x, y)
    } else if quad.len() >= 4 {
        // bounds: [x, y, w, h]
        (quad[0] + quad[2] / 2.0, quad[1] + quad[3] / 2.0)
    } else {
        (0.0, 0.0)
    }
}
