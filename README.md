<p align="center">
  <strong>nissia browser</strong><br>
  <em>The token-cheap browser CLI for AI agents — snap + act + autonomous agent</em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust">
</p>

---

**nissia browser** lets AI agents (Claude Code, Cursor, Codex, Continue, Windsurf, …)
drive a real browser while spending as **few tokens as possible**. It is a CLI, not an
MCP server, on purpose: you get back exactly what you print and nothing lingers in the
model's context.

One `snap` returns page structure, section content, and every actionable element with
compact `@eN` refs — enough to understand and act in a single turn.

```
$ nissia snap https://github.com/login --focus form
# Sign in to GitHub
@e1 [textbox] "Username or email address"
@e2 [textbox] "Password"
@e3 [button] "Sign in"

$ nissia fill @e1 "octocat"
$ nissia fill @e2 "hunter2" --no-snap
$ nissia click @e3
```

## Why nissia

- **Token-economic by design.** `--focus`, `--no-snap`, screenshots to file, and an
  autonomous agent mode that keeps navigation cost off the caller. See
  [docs/TOKEN-ECONOMY.md](docs/TOKEN-ECONOMY.md).
- **Autonomous agent.** `nissia agent "<goal>"` drives the browser with a cheap internal
  model and prints only the final answer.
- **Cheap search.** `nissia search "<query>"` over plain HTTP — no headless fingerprint.
- **Fast native Rust** over the Chrome DevTools Protocol. No Playwright/Puppeteer.
- **Sessions & record/replay.** Persist logins; replay flows at zero LLM cost.

## Install

```bash
# from source (requires the Rust toolchain)
git clone https://github.com/OWNER/nissia-browser.git
cd nissia-browser
cargo install --path crates/nissia-cli
# this installs the `nissia` binary
```

## Quickstart

```bash
nissia browser launch --headless --background   # start Chrome (isolated, persistent profile)
nissia snap https://example.com                 # structure + @eN refs
nissia read --focus main                        # page text as markdown
nissia eval "document.title"                    # run JS, extract data
nissia click @e1                                # act (add --no-snap to stay cheap)
nissia browser stop
```

> Tip: if you already have a normal Chrome open, prefer `--headless` — a visible launch
> can hand off to the existing instance and never open the debug port.

## Autonomous agent

Give it a goal; it navigates on its own and prints only the answer. The caller spends
~0 tokens on the in-between pages.

```bash
export NISSIA_AGENT_API_KEY=...                 # or OPENROUTER_API_KEY / OPENAI_API_KEY / ANTHROPIC_API_KEY
export NISSIA_AGENT_PROVIDER=openai             # openai-compatible | anthropic
export NISSIA_AGENT_BASE_URL=https://openrouter.ai/api/v1
export NISSIA_AGENT_MODEL=openai/gpt-4o-mini    # use something cheap

nissia agent "what is the latest stable Rust version?" --url https://www.rust-lang.org
```

## Search

```bash
nissia search "anthropic claude" --n 5          # free DuckDuckGo Instant Answer (no key)
nissia search "rust async runtime" --read       # also read the top result

# real web results (optional):
export NISSIA_SEARCH_API_KEY=...
export NISSIA_SEARCH_PROVIDER=brave             # brave | serper | tavily
```

## Token-economy helpers

`examples/economy/nissia-economy.ps1` (Windows) and `nissia-economy.sh` (Unix) wrap the
binary with the cheap defaults baked in: `peek` (focused read), `grab` (eval),
`act` (no re-snap), `shot` (screenshot to file), and a headless-first launch guard.

```bash
./examples/economy/nissia-economy.sh open https://example.com main
./examples/economy/nissia-economy.sh grab "document.title"
```

## Commands

`snap` `read` `eval` `click` `fill` `type` `select` `scroll` `screenshot` `wait`
`agent` `search` `session` `record` `replay` `browser` `schema` `mcp` `init`

Run `nissia --help` or `nissia schema [command]` for details.

## Credits

nissia browser is a fork of [**snact**](https://github.com/vericontext/snact) by
Kiyeon Jeon, which provides the excellent CDP core and the compact snapshot
compressor. nissia adds the autonomous `agent` mode, HTTP `search`, token-economy
wrappers, and a headless-first workflow. Huge thanks to the original author.

## License

MIT — see [LICENSE](LICENSE). Original copyright retained for the upstream project.
