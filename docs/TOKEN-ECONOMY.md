# Token economy

nissia browser exists for one reason: let an AI agent browse the web while spending as
FEW tokens as possible. The agent (for example Claude Code) is the brain; nissia just
executes its steps and returns the smallest useful output. No internal LLM, no API key.

## The three leaks

| Leak | Typical cost | Fix in nissia |
|------|--------------|---------------|
| full-page `snap` | 2,000-4,000 tokens | always pass `--focus <selector>`; prefer `read`/`eval` when you don't need `@eN` refs |
| auto re-snap after every action | 2,000-4,000 tokens per action | act with `--no-snap`; observe only when you actually need to |
| base64 screenshots into the context | huge | `screenshot --file out.png` writes to disk and returns a path; read the image only if you must |

## The cheap reading ladder (use the first that answers your question)

1. `eval "<js>"` — returns exactly the data you ask for. Cheapest.
2. `read --focus "<selector>"` — text of one section as markdown.
3. `snap --focus "<selector>"` — only when you need `@eN` refs to click.
4. full `snap` — last resort.

## Acting cheaply

```
nissia click @e5 --no-snap        # don't pay 2-4k for a re-snap you didn't ask for
nissia read --focus ".result"     # then look only at what changed, focused
```

The `examples/economy/` wrappers (`nissia-economy.ps1` / `.sh`) bake all of this in:
`peek` (focused read), `grab` (eval), `act` (no-snap), `shot` (screenshot to file),
plus a headless-first launch guard.

## Built-in search (no key)

```
nissia search "rust async runtime" --n 5
```

Returns a compact list instead of dumping a search results page into context. Default
backend is Mojeek over plain HTTP (no key, no headless fingerprint). Set
`NISSIA_SEARCH_API_KEY` (+ `NISSIA_SEARCH_PROVIDER=brave|serper|tavily`) for higher volume.

## Replay = zero LLM cost

Record a flow once, replay it forever with no model in the loop:

```
nissia record start my-flow
# ... drive the browser ...
nissia record stop
nissia replay my-flow
```

## Why CLI and not MCP

An MCP server keeps tool schemas and every observation resident in the model's context.
A CLI returns exactly what you print, when you print it, and nothing lingers. For raw
token economy the CLI wins, which is why nissia is CLI-first.
