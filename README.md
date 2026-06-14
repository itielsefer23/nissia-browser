<p align="center">
  <strong>nissia browser</strong><br>
  <em>The token-cheap browser CLI for AI agents</em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust">
</p>

---

**nissia browser** lets an AI agent (Claude Code, Cursor, Codex, Continue, Windsurf, ...)
drive a real browser while spending as **few tokens as possible**.

The agent is the brain. nissia is the cheap eyes and hands. There is **no internal LLM
and no API key**: your agent decides what to do, and nissia executes it with the smallest
possible output coming back into the agent's context. It is a CLI, not an MCP server, on
purpose: you get back exactly what you print, and nothing lingers in context.

```
$ nissia snap https://github.com/login --focus form
# Sign in to GitHub
@e1 [textbox] "Username or email address"
@e2 [textbox] "Password"
@e3 [button] "Sign in"

$ nissia fill @e1 "octocat" --no-snap
$ nissia fill @e2 "hunter2" --no-snap
$ nissia click @e3
```

## Two ways to use it

1. **Navigate** (your agent drives): `snap` / `read` / `eval` / `click` / `fill` / `type`
   / `scroll`, with the cheap defaults below. No key, no internal model.
2. **Search** (built in): `nissia search "<query>"` returns a compact list of results
   over plain HTTP. No API key required.

## Why nissia is cheap

| Leak | Typical cost | How nissia cuts it |
|------|--------------|--------------------|
| full-page `snap` | 2,000-4,000 tokens | always pass `--focus`; prefer `read` / `eval` |
| auto re-snap after every action | 2-4k per action | act with `--no-snap`; observe only when needed |
| base64 screenshots into context | huge | `screenshot --file out.png` returns a path, not bytes |

More detail: [docs/TOKEN-ECONOMY.md](docs/TOKEN-ECONOMY.md).

## Install

```bash
git clone https://github.com/OWNER/nissia-browser.git
cd nissia-browser
cargo install --path crates/nissia-cli   # installs the `nissia` binary
```

## Quickstart

```bash
nissia browser launch --headless --background   # isolated, persistent profile
nissia snap https://example.com --focus main    # structure + @eN refs (focused = cheap)
nissia read --focus main                         # page text as markdown
nissia eval "document.title"                     # run JS, extract exactly what you need
nissia click @e1 --no-snap                       # act without paying for a re-snap
nissia browser stop
```

> If you already have a normal Chrome open, prefer `--headless`: a visible launch can
> hand off to the existing instance and never open the debug port.

## Search (no API key)

```bash
nissia search "anthropic claude code" --n 5      # default backend: Mojeek (no key)
nissia search "rust async runtime" --read        # also read the top result
```

Want higher-volume or Google-grade results? Optionally set an API key:

```bash
export NISSIA_SEARCH_API_KEY=...
export NISSIA_SEARCH_PROVIDER=brave              # brave | serper | tavily
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
`search` `session` `record` `replay` `browser` `schema` `mcp` `init`

Run `nissia --help` or `nissia schema [command]` for details.

## Credits

nissia browser is built on the Chrome DevTools Protocol core and snapshot compressor from
the MIT-licensed **snact** project by Kiyeon Jeon. nissia reshapes it into its own tool
with a built-in no-key `search`, token-economy wrappers, and a headless-first workflow.
The MIT license (see [LICENSE](LICENSE)) requires keeping that original credit.

## License

MIT, see [LICENSE](LICENSE).
