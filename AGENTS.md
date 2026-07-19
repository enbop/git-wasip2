# Agent guidance

## Scope

- Keep this crate independent of Plainfeed and any other application data format.
- Expose bounded Git primitives, not merge, rebase, or general porcelain.
- Keep Git hosting provider details out of the API.
- Preserve `wasm32-wasip2` compatibility and verify runtime networking under Wasmtime.
- Do not expose credentials through `Debug`, errors, logs, command arguments, or Git configuration.
- Treat response, object, pack, and repository limits as part of the public safety contract.

## Checks

```text
cargo fmt --all -- --check
cargo test --locked
RUSTFLAGS="--cfg tokio_unstable" cargo build --locked --target wasm32-wasip2
```

## Commits

Use Conventional Commits in English. Add `Assisted-by: Codex` to non-trivial
AI-assisted commits. Do not commit or push unless the user explicitly asks.
