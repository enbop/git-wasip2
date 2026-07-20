# git-wasip2

Bounded Git operations for Rust applications running on `wasm32-wasip2`.

## Supported operations

- Smart HTTP fetch, including HTTPS Basic authentication
- commit, tree, reference, ancestry, and worktree inspection
- full or selected tree checkout into a standalone directory
- status comparison between a commit and a standalone directory
- one-parent commits from a directory snapshot
- one-commit, SHA-1, fast-forward Smart HTTP push
- response, object, pack, and repository-size limits

The current scope does not include SSH, SHA-256 repositories, submodules,
merge, rebase, or general Git porcelain.

## Try it locally with Wasmtime

Requirements: Rust, Git, Python 3, and Wasmtime.

Build the example CLI:

```bash
rustup target add wasm32-wasip2
RUSTFLAGS="--cfg tokio_unstable" cargo build --release --locked \
  --target wasm32-wasip2 --example git-wasip2
```

In the first terminal, create a temporary Git remote and keep its loopback
Smart HTTP server running:

```bash
scripts/serve-local-test-repo.sh
```

In a second terminal, load the paths printed by the server:

```bash
source target/git-wasip2-demo.env
```

Fetch `main` inside Wasmtime:

```bash
wasmtime run -S inherit-network=y \
  --dir "$DEMO_WORKSPACE::/demo" \
  "$GIT_WASIP2_WASM" \
  fetch "$REMOTE_URL" /demo/client main
```

Check out the fetched commit into a separate, editable directory:

```bash
wasmtime run \
  --dir "$DEMO_WORKSPACE::/demo" \
  "$GIT_WASIP2_WASM" \
  checkout /demo/client refs/remotes/origin/main /demo/worktree
```

Change the checkout from the host terminal:

```bash
printf 'written during the Wasmtime demo\n' >"$DEMO_WORKSPACE/worktree/hello.txt"
```

Inspect the change inside Wasmtime:

```bash
wasmtime run \
  --dir "$DEMO_WORKSPACE::/demo" \
  "$GIT_WASIP2_WASM" \
  status /demo/client refs/remotes/origin/main /demo/worktree
```

Create a one-parent candidate commit from the complete checkout:

```bash
wasmtime run \
  --dir "$DEMO_WORKSPACE::/demo" \
  "$GIT_WASIP2_WASM" \
  commit /demo/client refs/remotes/origin/main /demo/worktree \
  refs/git-wasip2/candidate "demo: add hello"
```

Push the candidate to the local remote:

```bash
wasmtime run -S inherit-network=y \
  --dir "$DEMO_WORKSPACE::/demo" \
  "$GIT_WASIP2_WASM" \
  push "$REMOTE_URL" /demo/client \
  refs/git-wasip2/candidate refs/heads/main
```

Use native Git to independently inspect the result:

```bash
git --git-dir="$REMOTE_REPOSITORY" log --oneline --decorate
git --git-dir="$REMOTE_REPOSITORY" show main:hello.txt
git --git-dir="$REMOTE_REPOSITORY" fsck --full
```

Press Ctrl+C in the first terminal to stop the server and remove the temporary
repositories.

For HTTPS authentication, pass `GIT_WASIP2_USERNAME` and
`GIT_WASIP2_PASSWORD` into the guest environment together. Credentials are not
accepted as command arguments.

## License

MIT
