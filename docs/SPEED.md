# Speed

nissia itself is fast. Measured on a normal laptop (release build):

| Operation | Time |
|-----------|------|
| `eval` (process start + CDP connect + 1 JS) | ~110-150 ms |
| `snap` re-extract same page | ~210-380 ms |
| `snap` with navigation | ~320 ms |
| `read` with navigation | ~250 ms |
| Cold Chrome launch to ready | ~350 ms |
| `batch` of 4 steps, warm, one connection | ~170 ms |

So a multi-step browsing task is ~1 second of real nissia work. When an agentic task
feels slow, the cost is almost always **model round-trips**, not nissia.

## Keep agent mode fast

1. **One turn, not N.** Plan the whole flow and run it with `nissia batch` (steps from
   stdin, one verb per line, one persistent connection). A flow that would be 6 separate
   commands (6 model round-trips) becomes one call.
   ```bash
   printf 'goto https://example.com\nsnap main\neval document.title\n' | nissia batch
   ```
2. **No sleeps.** nissia waits for the real load/settle event; fixed `sleep`s only waste time.
3. **Warm browser.** Launch once (`browser launch --headless --background --idle-timeout 30`)
   and reuse it; cold start is cheap but relaunching every step is not.
4. **Cheap observations.** `eval` and `read --focus` beat full `snap`; act with `--no-snap`.

## Truly autonomous speed (optional)

For hands-off browsing without any round-trip back to the calling agent, the optional
`nissia agent "<goal>"` runs the loop with a cheap, fast internal model (e.g. Groq or
Haiku) and returns only the answer. It needs `NISSIA_AGENT_API_KEY`; everything else
works with no key.
