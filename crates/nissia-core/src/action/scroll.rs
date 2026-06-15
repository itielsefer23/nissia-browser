//! Scroll action — human-paced, using TRUSTED mouse-wheel events (not JS
//! `scrollBy`), which is what behavioural anti-bot systems inspect (wheel cadence,
//! velocity, pauses). Includes a "read" mode that traverses the whole page like a
//! person scanning an article (progressive scrolls + reading dwell, F-pattern:
//! more attention near the top), bounded in time, closing late pop-ups as it goes.

use nissia_cdp::commands::{InputDispatchMouseEvent, RuntimeEvaluate};
use nissia_cdp::CdpTransport;

fn lcg(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed >> 33
}

fn seed_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0x9E37_79B9)
        | 1
}

/// Read [innerHeight, scrollHeight, scrollY] from the page.
async fn metrics(transport: &CdpTransport) -> (f64, f64, f64) {
    let js = "JSON.stringify([window.innerHeight, Math.max(document.body?document.body.scrollHeight:0, document.documentElement.scrollHeight), window.scrollY||window.pageYOffset||0])";
    if let Ok(r) = transport
        .send(&RuntimeEvaluate {
            expression: js.to_string(),
            return_by_value: Some(true),
            await_promise: None,
            context_id: None,
        })
        .await
    {
        if let Some(serde_json::Value::String(s)) = r.result.value {
            if let Ok(v) = serde_json::from_str::<Vec<f64>>(&s) {
                if v.len() == 3 {
                    return (v[0].max(1.0), v[1], v[2]);
                }
            }
        }
    }
    (800.0, 0.0, 0.0)
}

/// One trusted wheel "flick" of `delta_y` px, broken into a few smooth ticks
/// with easing + jitter, dispatched at a point inside the viewport.
async fn wheel(
    transport: &CdpTransport,
    delta_y: f64,
    seed: &mut u64,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let (px, py) = (300.0 + (lcg(seed) % 200) as f64, 250.0 + (lcg(seed) % 200) as f64);
    let ticks = 4 + (lcg(seed) % 4); // 4-7 ticks
    let mut sent = 0.0;
    for i in 0..ticks {
        // ease-out: bigger ticks first, smaller as the flick settles
        let frac = 1.0 - (i as f64 / ticks as f64);
        let mut d = (delta_y / ticks as f64) * (0.7 + 0.6 * frac);
        if i == ticks - 1 {
            d = delta_y - sent; // exact remainder on the last tick
        }
        sent += d;
        transport
            .send(&InputDispatchMouseEvent {
                event_type: "mouseWheel".to_string(),
                x: px,
                y: py,
                delta_x: Some(0.0),
                delta_y: Some(d),
                ..Default::default()
            })
            .await?;
        let pause = 14 + lcg(seed) % 26; // 14-40ms between ticks
        tokio::time::sleep(std::time::Duration::from_millis(pause)).await;
    }
    Ok(())
}

pub async fn execute(
    transport: &CdpTransport,
    direction: &str,
    amount: Option<i64>,
) -> Result<(), nissia_cdp::CdpTransportError> {
    if direction == "read" {
        return execute_read(transport, amount.map(|a| a.max(1) as u32)).await;
    }
    let pixels = amount.unwrap_or(600).abs().max(1) as f64;
    let mut seed = seed_now();
    let (dx, dy): (f64, f64) = match direction {
        "up" => (0.0, -pixels),
        "right" => (pixels, 0.0),
        "left" => (-pixels, 0.0),
        _ => (0.0, pixels),
    };
    if dx != 0.0 {
        // horizontal: single wheel with deltaX
        let (px, py) = (400.0, 300.0);
        transport
            .send(&InputDispatchMouseEvent {
                event_type: "mouseWheel".to_string(),
                x: px,
                y: py,
                delta_x: Some(dx),
                delta_y: Some(0.0),
                ..Default::default()
            })
            .await?;
    } else {
        wheel(transport, dy, &mut seed).await?;
    }
    Ok(())
}

/// Human "read-through": scroll the page top→bottom like a person scanning an
/// article. Progressive wheel flicks (~80% of a screen each) with reading dwell
/// that is longer near the top (F-pattern), an occasional small scroll-back
/// (re-reading), and a light pop-up dismiss every few screens. Bounded by
/// `max_screens` (default 12) so it always terminates quickly.
pub async fn execute_read(
    transport: &CdpTransport,
    max_screens: Option<u32>,
) -> Result<(), nissia_cdp::CdpTransportError> {
    let mut seed = seed_now();
    let (vh, _total, _y0) = metrics(transport).await;
    let max = max_screens.unwrap_or(8).clamp(1, 30);
    let step = vh * 1.1; // a bit more than a screen per flick — people SCAN, fast
    let started = std::time::Instant::now();
    let budget_ms = 5000; // hard cap: a quick scan, not a slow read

    for i in 0..max {
        wheel(transport, step, &mut seed).await?;

        // Reading dwell: people SCAN (research: ~79% scan, ~16% read word-by-word).
        // A short glance at the top (title/intro), then quick scanning below.
        let dwell = if i == 0 {
            450 + lcg(&mut seed) % 400 // ~0.45-0.85s
        } else {
            160 + lcg(&mut seed) % 240 // ~0.16-0.40s
        };
        tokio::time::sleep(std::time::Duration::from_millis(dwell)).await;

        // Occasional small scroll-back, like re-reading a line (~10%).
        if lcg(&mut seed) % 100 < 10 {
            wheel(transport, -(vh * 0.15), &mut seed).await?;
            tokio::time::sleep(std::time::Duration::from_millis(140 + lcg(&mut seed) % 200)).await;
        }

        // Close late-appearing overlays/ads/consent so reading isn't blocked
        // (timed pop-ups appear after a delay, so check often — dismiss is cheap).
        if i % 2 == 1 {
            let _ = crate::action::dismiss::execute(transport).await;
        }

        // Stop at the bottom, or when the time budget is spent (whichever first).
        let (nvh, ntotal, ny) = metrics(transport).await;
        if ny + nvh >= ntotal - 4.0 || started.elapsed().as_millis() as u64 >= budget_ms {
            break;
        }
    }
    // One more sweep at the end: the popup that pops up right as you finish.
    let _ = crate::action::dismiss::execute(transport).await;
    Ok(())
}
