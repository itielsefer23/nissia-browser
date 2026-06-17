# Changelog

All notable changes to nissia are documented here.

## 0.3.4

Human-like browsing, warm login profile, safe buying, and reinforced 2026 stealth.
The whole realism layer runs inside the binary (native CDP), so it costs 0 tokens and
is adaptive: full human behavior where it matters, fast everywhere else.

### Added
- **Behavior engine** (`behavior.rs`): adaptive pacing `Fast | Human | Protected` with a
  global `--pace` flag and an RNG using gaussian / lognormal distributions. Default:
  visible launch = human, headless = fast; the pace is inherited by every later command.
- **Human mouse**: cubic Bezier path + per-point gaussian jitter + overshoot and
  correction (~35% of clicks) + non-uniform (ease-in-out) velocity.
- **Human scroll**: wheel inertia, reading pauses, and an occasional short scroll-up
  (human pace only).
- **Human typing**: lognormal cadence, longer pauses after space/punctuation, and an
  occasional typo + backspace.
- **Dwell after navigation** so a freshly loaded page is not hit instantly like a bot.
- **`nissia browser login`**: a dedicated persistent profile to sign in once. It stays
  warm and is reused in Agent mode (the strongest anti-bot signal). Chrome 136+ no longer
  exposes the live Default profile over the debug port, hence a dedicated one.
- **Honest reporting** helper (IntersectionObserver) in the recipes: count only what
  actually entered the viewport and report "scanned N of M", never the whole DOM.
- **`nissia init`** writes `AGENTS.md` + `.nissia/recipes.md` (the full per-site playbook)
  so Cursor, Codex, and other tools get the same depth as Claude Code; `--force` refreshes
  the docs.

### Safety
- **Buy flow stops at the order summary** and waits for explicit confirmation before
  clicking Pay (only with payment already saved).
- **Payment-card / CVV fields are refused**: both `type` and `fill` guard against typing
  into card-number, CVV/CVC, or expiry fields.

### Stealth (2026)
- **Consistency layer**: timezone override (`Emulation.setTimezoneOverride`), a clean
  `navigator.language` / `navigator.languages` (no leaked `;q=` values), and a UA override
  with the real user agent plus accept-language. geo / timezone / locale / UA set on
  `launch` now persist across the whole session.
- Verified 0 red flags / 31 tests on bot.sannysoft; nissia still never calls
  `Runtime.enable` (the #1 CDP tell).

### Fixed
- **`read --focus <selector>`** now picks the match with the most text when several
  elements match (e.g. an embedded model-card `<article>` plus the real article body).
  Previously it took the first match and could return a 2-line stub instead of the page.
- Reliable clicks on lazy-loading SPAs: the target is scrolled in, the layout is allowed
  to settle, and the cursor is verified on target before clicking.
- `install.ps1` now adds the binary directory to the user PATH (persistent + current
  session), so `nissia` works right after install.

## 0.3.3

Dead-simple install: paste the repo link and the agent installs it. README rewritten for
"give this link to your agent" plus an AI-agent install recipe.

## 0.3.2

Reliable clicks on lazy SPAs, stealth consistency (geo/timezone/locale/UA agree), and
cross-tool parity via `nissia init` (AGENTS.md + recipes for Cursor/Codex).
