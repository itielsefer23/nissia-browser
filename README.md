<p align="center">
  <strong>nissia browser</strong><br>
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

## Use it from Claude Code, Codex or Cursor
This repo ships a `/nissia-browser` skill (in `.claude/skills/`). When you ask your agent to
search or browse, it picks the right mode and runs nissia for you, keeping it cheap.

## Optional extras
- **Better search results** with a self-hosted SearXNG instance (`NISSIA_SEARXNG_URL`). Not
  required: the default DuckDuckGo search needs no setup and no key.
- **Hands-off agent** with a cheap AI key (`nissia agent "<goal>"`), off by default.

## License
**MIT** — free to use, change and share, including in commercial projects. The only rule is to
keep the license and copyright notice. nissia is a fork of the MIT-licensed
[snact](https://github.com/vericontext/snact) project by Kiyeon Jeon, whose original copyright
is kept in [LICENSE](LICENSE) next to ours.
