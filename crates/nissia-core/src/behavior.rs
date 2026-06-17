//! Adaptive human-behaviour timing + RNG, shared by the input actions
//! (mouse, scroll, typing).
//!
//! Everything here runs inside the binary (native CDP calls), so simulating a
//! human costs ZERO model tokens. `Pace` lets us be fully human where it matters
//! (visible Agent mode, login / checkout / protected pages) and fast where it
//! doesn't (headless Navigate / Search) — so realism never costs speed by
//! default. 2026 anti-bot systems score trajectory entropy, scroll inertia,
//! typing rhythm and dwell time, so a clean Bézier / constant-speed / instant
//! action is an immediate tell; the helpers here add the variance that reads as
//! human.

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pace {
    /// Minimal delays — headless internal use (Navigate / Search). Still uses
    /// trusted input events, just without the human pauses.
    Fast,
    /// Full human realism — the visible Agent-mode default.
    Human,
    /// Extra-careful human — login / checkout / captcha / strongly-protected pages.
    Protected,
}

impl Pace {
    pub fn parse(s: &str) -> Pace {
        match s.trim().to_ascii_lowercase().as_str() {
            "fast" => Pace::Fast,
            "protected" | "careful" => Pace::Protected,
            _ => Pace::Human,
        }
    }

    /// Multiplier applied to human delays (0 = instant, 1 = human, >1 = careful).
    pub fn factor(self) -> f64 {
        match self {
            Pace::Fast => 0.0,
            Pace::Human => 1.0,
            Pace::Protected => 1.45,
        }
    }

    /// Active pace for this process: env `NISSIA_PACE` wins, else the value
    /// persisted at launch, else `Human`. Cached for the process lifetime.
    pub fn current() -> Pace {
        static P: OnceLock<Pace> = OnceLock::new();
        *P.get_or_init(|| {
            if let Ok(v) = std::env::var("NISSIA_PACE") {
                if !v.trim().is_empty() {
                    return Pace::parse(&v);
                }
            }
            load_pace().unwrap_or(Pace::Human)
        })
    }
}

fn pace_file() -> std::path::PathBuf {
    crate::data_dir().join("session_pace")
}

/// Persist the session pace (called at launch: visible → human, headless → fast),
/// so every later command in the session inherits it without repeating a flag.
pub fn save_pace(p: Pace) {
    let s = match p {
        Pace::Fast => "fast",
        Pace::Human => "human",
        Pace::Protected => "protected",
    };
    let _ = std::fs::write(pace_file(), s);
}

fn load_pace() -> Option<Pace> {
    std::fs::read_to_string(pace_file())
        .ok()
        .map(|s| Pace::parse(&s))
}

/// 64-bit LCG seed from the clock.
pub fn rng_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1
}

/// LCG → pseudo-random f64 in [0,1).
pub fn rand01(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 33) as f64) / (1u64 << 31) as f64
}

/// Gaussian sample (Box–Muller) with the given mean / std-dev.
pub fn gauss(seed: &mut u64, mean: f64, std: f64) -> f64 {
    let u1 = rand01(seed).max(1e-9);
    let u2 = rand01(seed);
    let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    mean + z * std
}

/// Sleep `base_ms` scaled by the active pace, with ±25% jitter. No-op in Fast.
pub async fn pause(base_ms: u64) {
    let f = Pace::current().factor();
    if f <= 0.0 || base_ms == 0 {
        return;
    }
    let mut s = rng_seed();
    let jitter = 0.75 + rand01(&mut s) * 0.5; // 0.75..1.25
    let ms = (base_ms as f64 * f * jitter) as u64;
    if ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
}

/// Dwell after a navigation, before the first interaction: humans pause to take
/// the page in, and anti-bot JS challenges also need a moment to settle. Scaled
/// by pace (Human ~1.2–3.0s, Protected ~1.8–4.4s, Fast: none).
pub async fn dwell_after_load() {
    let p = Pace::current();
    if p == Pace::Fast {
        return;
    }
    let mut s = rng_seed();
    let base = 1200.0 + rand01(&mut s) * 1800.0;
    let ms = (base * p.factor()) as u64;
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}
