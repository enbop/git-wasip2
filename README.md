# git-wasip2

`git-wasip2` is an early-stage, bounded Smart HTTP Git client for Rust
applications compiled to `wasm32-wasip2`.

It extracts the provider-independent Git/WASI work proven by Plainfeed. The
crate deliberately exposes a narrow synchronization substrate instead of a
general Git porcelain:

- public and authenticated HTTPS fetch;
- bounded response, object, pack, and repository storage;
- commit, tree, reference, ancestry, and worktree inspection;
- safe snapshot export; and
- one-commit, SHA-1, fast-forward Smart HTTP push with status verification.

It does not define application path ownership, conflict policy, file formats,
staging layout, scheduling, or recovery journals. Those remain responsibilities
of each consuming application.

## Status and integration constraints

The current compatibility build pins a public `enbop` Gitoxide revision which
in turn pins the public WASIp2-compatible memmap2 fork. Consumers therefore do
not need their own Gitoxide or memmap2 dependency overrides. Tokio networking
on `wasm32-wasip2` currently requires:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --target wasm32-wasip2
```

The API and compatibility requirements may change before the first stable
release.

## Verification

Native checks cover formatting, linting, limits, repository inspection,
snapshot export, reference safety, and fast-forward finalization. The CI also
downloads a checksum-pinned Wasmtime release and starts a loopback Smart HTTP
fixture backed by `git-upload-pack` and `git-receive-pack`. A real WASIp2 guest
then fetches, creates a one-parent commit, pushes it, rejects a stale repeat,
and leaves repositories that pass independent native `git fsck --full` checks.

Run the same integration test locally with:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --locked \
  --target wasm32-wasip2 --example git-wasip2-driver
scripts/verify-smart-http-wasmtime.sh
```

## Origin

The implementation was initially developed as `plainfeed-git`. Its research
record is preserved in [`docs/plainfeed-origin.md`](docs/plainfeed-origin.md).
Development is AI-assisted and commits retain explicit attribution.

## License

MIT
