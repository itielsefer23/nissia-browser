# Token economy

nissia browser exists for one reason: let an AI agent browse the web while
spending as FEW tokens as possible. Browsing is where agents quietly burn their
context. Here is where the tokens go, and how nissia cuts each leak.

## The three leaks

| Leak | Typical cost | Fix in nissia |
|------|--------------|---------------|
| full-page `snap` | 2,000-4,000 tokens | always pass `--focus <selector>`; prefer `read`/`eval` when you don't need `@eN` refs |
| auto re-snap after every action | 2,000-4,000 tokens *per action* | act with `--no-snap`; observe only when you actually need to |
| base64 screenshots into the context | huge | `screenshot --file out.png` writes to disk and returns a path; the agent reads the image only if it must |

## The cheap reading ladder (use the first that answers your question)

1. `eval "<js>"` — returns exactly the data you ask for. Cheapest.
2. `read --focus "<selector>"` — text of one section as markdown.
3. `snap --focus "<selector>"` — only when you need `@eN` refs to click.
4. full `snap` — last resort.

## Acting cheaply

```
nissia click @e5 --no-snap        # don't pay 2-4k for a re-snap you didn't ask for
nissia read --focus ".result"     # ...look only at what changed, focused
```

The `examples/economy/` wrappers (`nissia-economy.ps1` / `.sh`) bake all of this
in: `peek` (cheap read), `grab` (eval), `act` (no-snap), `shot` (file), plus a
headless-first launch guard.

## The cheapest path of all: the autonomous agent

```
nissia agent "find the latest stable Rust version and its release date" \
  --url https://www.rust-lang.org
```

`agent` runs the whole snap/click/read loop INTERNALLY using a small, cheap model,
and prints ONLY the final answer. The expensive model that called `nissia` pays for
one command and one short result — not for every intermediate page. Point it at a
cheap model and navigation becomes nearly free for the caller:

```
export NISSIA_AGENT_API_KEY=...           # or OPENROUTER_API_KEY / ANTHROPIC_API_KEY
export NISSIA_AGENT_PROVIDER=openai       # openai-compatible (OpenRouter/Groq/OpenAI/local) | anthropic
export NISSIA_AGENT_BASE_URL=https://openrouter.ai/api/v1
export NISSIA_AGENT_MODEL=openai/gpt-4o-mini   # pick something cheap
```

## Replay = zero LLM cost

Record a flow once, replay it forever with no model in the loop:

```
nissia record start my-flow
# ... drive the browser ...
nissia record stop
nissia replay my-flow
```

## Why CLI and not MCP

An MCP server keeps tool schemas and every observation resident in the model's
context. A CLI returns exactly what you print, when you print it, and nothing
lingers. For raw token economy the CLI wins, which is why nissia is CLI-first.
