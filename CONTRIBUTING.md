# Contributing to nissia

## Getting started

```bash
git clone git@github.com:OWNER/nissia-browser.git
cd nissia
./scripts/setup-hooks.sh   # install git hooks (auto version bump)
cargo build
```

Requires:
- Rust stable (latest)
- Google Chrome (for integration testing)

## Project structure

```
crates/
  nissia-cdp/    # CDP WebSocket transport layer
  nissia-core/   # Domain logic: snap, read, action, session, record
  nissia-cli/    # Binary: CLI entry point (clap), command handlers
```

Three-crate workspace: `cdp` handles Chrome protocol, `core` is the library, `cli` is a thin shell over core.

## Development workflow

```bash
# Check everything compiles
cargo check --workspace

# Run lints
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Run tests
cargo test --workspace

# Build and test manually
cargo build
nissia browser launch --background
./target/debug/nissia snap https://example.com
nissia browser stop
```

## Commit conventions

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — new feature (triggers minor version bump)
- `fix:` — bug fix (triggers patch version bump)
- `docs:` — documentation only
- `refactor:` — code change that neither fixes a bug nor adds a feature
- `test:` — adding or updating tests
- `chore:` — build, CI, tooling changes

A `pre-push` git hook automatically bumps the version and creates a tag based on commit prefixes. Run `./scripts/setup-hooks.sh` to install it.

## Adding a new command

1. Add CDP command types in `nissia-cdp/src/commands.rs` if needed
2. Implement core logic in `nissia-core/src/<module>/`
3. Add CLI handler in `nissia-cli/src/cmd/<name>.rs`
4. Register in `nissia-cli/src/cmd/mod.rs` and `main.rs`
5. Add schema in `nissia-cli/src/cmd/schema.rs`
6. Add MCP tool in `nissia-cli/src/cmd/mcp.rs`

## Adding a CDP command

Hand-write the command struct in `commands.rs`. We intentionally avoid generated CDP bindings (~60K lines) to keep compile times fast. Only add commands nissia actually needs.

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YourCommand {
    pub some_param: String,
}

#[derive(Debug, Deserialize)]
pub struct YourCommandResponse {
    pub result_field: String,
}

impl CdpCommand for YourCommand {
    type Response = YourCommandResponse;
    fn method_name(&self) -> &'static str {
        "Domain.methodName"
    }
}
```

## Testing

- **Unit tests**: CDP serialization, filter logic, element map round-trip
- **Integration tests**: require Chrome; gated behind `#[ignore]` or feature flags
- **Manual E2E**: `nissia snap` → verify output, `nissia click` → verify DOM change

For integration tests, use `nissia browser launch --headless --background` to avoid UI popups.

## Pull requests

- Keep PRs focused: one feature or fix per PR
- Run `cargo fmt --all && cargo clippy --workspace -- -D warnings` before pushing
- Fill in the PR template
- CI must pass (check + clippy + fmt + test)
