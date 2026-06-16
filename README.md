<p align="center">
  <img src="assets/nissia-browser-icon.png" alt="nissia browser" width="132">
</p>

<h1 align="center">nissia browser</h1>

<p align="center">
  <em>A token-cheap way for AI agents to browse the web</em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust">
</p>

---

**nissia browser** lets an AI coding agent (Claude Code, Codex, Cursor and others) use a real
web browser while spending very few tokens. It is a small command-line tool: your agent runs
simple commands and only the useful result comes back. No MCP server, no heavy screenshots,
and no API keys for everyday use. Works on **Windows, macOS and Linux**.

## Why it is cheap and fast
- It returns just the text or data you ask for, never whole pages or images.
- Whole flows run in one `nissia batch` (one connection, one round-trip) with adaptive waits.
- A full "operate a form and read the results" task is about **500–900 tokens and a few seconds**.
- Works with any agent that can run a shell command.

## The 3 modes
1. **Search** — quickly find things on the web and get a short list of results. The fastest.
2. **Navigate** — open a specific site and read or extract what you need (runs in the
   background, no window).
3. **Agent** — give it a goal: it searches, opens the best page, closes cookie and ad
   pop-ups, fills forms and reads the answer, in a real visible browser you can watch.

## What makes the Agent mode good
- **Closes the annoying stuff.** Cookie consent banners (OneTrust, Didomi, Sourcepoint,
  Quantcast, even the ones inside iframes) and ad pop-ups, so the page is readable.
- **Acts like a human.** Real mouse clicks (including calendar cells and grids via
  `click --sel`), human-paced scrolling and typing, natural referrers, and it keeps
  `navigator.webdriver` false **without** the flag that shows Chrome's "you are automated"
  banner — so sites do not treat it as a bot.
- **Recovers like a human.** If a page errors or half-loads, it can `reload` and retry.
- **Your browser, your choice.** Chrome, Edge, Brave or Opera (`nissia browser detect`).

## Install
Prebuilt binary (no Rust needed):
```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/itielsefer23/nissia-browser/master/install.sh | sh
# Windows (PowerShell)
irm https://raw.githubusercontent.com/itielsefer23/nissia-browser/master/install.ps1 | iex
```
Or build from source:
```bash
git clone https://github.com/itielsefer23/nissia-browser.git
cd nissia-browser
cargo install --path crates/nissia-cli   # installs the `nissia` command
```

## Quick examples
```bash
nissia search "best laptops 2026"             # quick web search
nissia browser detect                         # which browsers are installed
nissia snap https://example.com --focus main  # open a page, list clickable elements
nissia read https://example.com --focus main  # read a page as clean text
nissia dismiss                                # close cookie banners and pop-ups
nissia update                                 # check for a newer version
```
Run `nissia --help` to see everything.

## Updating
nissia tells you when a newer version is out: `nissia update --check` (cached for 24h, used by
the skill on startup). Run the installer again to upgrade, then re-copy the skill from this repo.

## Use it from Claude Code (plugin)
nissia browser is a Claude Code **plugin** that adds the `/nissia-browser` skill. Two steps:

1. **Install the `nissia` binary** (see [Install](#install) above) — the skill drives this binary.
2. **Add the plugin** in Claude Code:
   ```
   /plugin marketplace add itielsefer23/nissia-browser
   /plugin install nissia-browser@nissia
   ```
   Then call `/nissia-browser` (or just ask it to search/browse). It asks which mode (and which
   browser for Agent mode) and runs nissia for you, keeping it cheap. Update later with
   `/plugin update nissia-browser@nissia`.

> Pasting the repo URL alone does not register the skill — you need the two commands above
> (that is what makes `/nissia-browser` show up as a slash command).

### Codex, Cursor and other agents
Run `nissia init` in your project to drop an `AGENTS.md` so any shell-capable agent discovers
the commands; then it calls `nissia ...` directly.

## Documentation
- [docs/GUIDE.md](docs/GUIDE.md) — complete guide: the 3 modes, browser selection and default,
  Agent mode, human navigation (mouse trajectory, typed search, read-scroll, pop-up closing),
  operating forms, batch, full command reference.
- [docs/TOKEN-ECONOMY.md](docs/TOKEN-ECONOMY.md) — how it keeps token cost tiny.
- [docs/SPEED.md](docs/SPEED.md) — performance numbers and how to stay fast.

## Optional extras
You don't need any of these. The defaults (DuckDuckGo search + you, the agent, driving) work
with no setup and no keys. These are only for specific cases:

- **SearXNG for better/unlimited search.** [SearXNG](https://github.com/searxng/searxng) is a
  free, open-source metasearch engine you run yourself (one Docker container). It aggregates
  Google, Bing, etc. and has no rate limits, so search results are richer than the default. Use
  it only if the built-in DuckDuckGo search isn't enough for you. To enable:
  1. Run an instance (e.g. `docker run -d -p 8888:8080 searxng/searxng`) and turn on the JSON
     output format in its `settings.yml`.
  2. Point nissia at it: set `NISSIA_SEARXNG_URL=http://localhost:8888` (or add
     `"searxng_url": "http://localhost:8888"` to `<data-dir>/search.json`).
  3. Select it: `NISSIA_SEARCH_PROVIDER=searxng nissia search "your query"`.
- **Hands-off agent (opt-in).** `nissia agent "<goal>"` runs a small internal LLM loop and prints
  only the final answer. It needs your own API key in `NISSIA_AGENT_API_KEY`; it is off by
  default and the normal search/navigate/agent modes never use it.

## Feedback
Ideas, comments and "this site didn't work" reports are very welcome and shape the skill:
- 💡 [Open a feedback issue](https://github.com/itielsefer23/nissia-browser/issues/new?template=feedback.yml) (structured form).
- 🐞 [Report a bug](https://github.com/itielsefer23/nissia-browser/issues/new?template=bug_report.yml).
- 💬 [Discussions](https://github.com/itielsefer23/nissia-browser/discussions) for free-form comments and questions.

## License
**MIT** — free to use, change and share, including in commercial projects. The only rule is to
keep the license and copyright notice. nissia is a fork of the MIT-licensed
[snact](https://github.com/vericontext/snact) project by Kiyeon Jeon, whose original copyright
is kept in [LICENSE](LICENSE) next to ours.
