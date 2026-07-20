#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
WASMTIME=${WASMTIME:-wasmtime}
WASM="$ROOT/target/wasm32-wasip2/debug/examples/git-wasip2.wasm"
TEMPORARY=$(mktemp -d "${TMPDIR:-/tmp}/git-wasip2-integration.XXXXXX")
REMOTE="$TEMPORARY/remote.git"
SEED="$TEMPORARY/seed"
CLIENT="$TEMPORARY/work/client"
WORKTREE="$TEMPORARY/work/worktree"
READY="$TEMPORARY/server.port"
SERVER_LOG="$TEMPORARY/server.log"
SERVER_PID=

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    if [ "${KEEP_GIT_WASIP2_FIXTURE:-0}" = 1 ]; then
        printf 'fixture retained at %s\n' "$TEMPORARY"
    else
        rm -rf "$TEMPORARY"
    fi
}
trap cleanup EXIT HUP INT TERM

if [ ! -f "$WASM" ]; then
    printf 'missing WASIp2 example CLI: %s\n' "$WASM" >&2
    printf 'build it with RUSTFLAGS="--cfg tokio_unstable" cargo build --locked --target wasm32-wasip2 --example git-wasip2\n' >&2
    exit 1
fi

mkdir -p "$TEMPORARY/work"
git init --quiet --bare --initial-branch=main "$REMOTE"
git init --quiet --initial-branch=main "$SEED"
git -C "$SEED" config user.name "git-wasip2 integration test"
git -C "$SEED" config user.email "integration@git-wasip2.invalid"
printf 'initial content\n' >"$SEED/initial.txt"
git -C "$SEED" add initial.txt
git -C "$SEED" commit --quiet -m "test: seed smart HTTP remote"
git -C "$SEED" remote add origin "$REMOTE"
git -C "$SEED" push --quiet origin main
INITIAL_TIP=$(git --git-dir="$REMOTE" rev-parse refs/heads/main)

python3 "$ROOT/tests/support/git-smart-http-server.py" \
    "$REMOTE" --ready-file "$READY" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!
attempt=0
while [ ! -s "$READY" ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ] || ! kill -0 "$SERVER_PID" 2>/dev/null; then
        printf 'Smart HTTP server did not become ready\n' >&2
        cat "$SERVER_LOG" >&2
        exit 1
    fi
    sleep 0.05
done
PORT=$(cat "$READY")
URL="http://127.0.0.1:$PORT/repo.git"

run_guest() {
    "$WASMTIME" run -S inherit-network=y \
        --dir "$TEMPORARY/work::/work" "$WASM" "$@"
}

run_guest fetch "$URL" /work/client main >"$TEMPORARY/fetch.log"
grep -F "remote_tip=$INITIAL_TIP" "$TEMPORARY/fetch.log" >/dev/null
test "$(git -C "$CLIENT" rev-parse refs/remotes/origin/main)" = "$INITIAL_TIP"
git -C "$CLIENT" fsck --full

run_guest checkout /work/client refs/remotes/origin/main /work/worktree \
    >"$TEMPORARY/checkout.log"
test "$(cat "$WORKTREE/initial.txt")" = "initial content"
printf 'written and committed inside Wasmtime\n' >"$WORKTREE/wasi-push.txt"
run_guest status /work/client refs/remotes/origin/main /work/worktree \
    >"$TEMPORARY/status.log"
grep -F "changed=wasi-push.txt" "$TEMPORARY/status.log" >/dev/null

run_guest commit /work/client refs/remotes/origin/main /work/worktree \
    refs/git-wasip2/integration-candidate \
    "test: create WASIp2 push candidate" >"$TEMPORARY/create.log"
CANDIDATE=$(git -C "$CLIENT" rev-parse refs/git-wasip2/integration-candidate)
test "$(git -C "$CLIENT" rev-parse "$CANDIDATE^")" = "$INITIAL_TIP"
test "$(run_guest show-ref /work/client refs/git-wasip2/integration-candidate)" = \
    "$CANDIDATE"

run_guest push "$URL" /work/client refs/git-wasip2/integration-candidate \
    refs/heads/main >"$TEMPORARY/push.log"
grep -F "pushed_commit=$CANDIDATE" "$TEMPORARY/push.log" >/dev/null
test "$(git --git-dir="$REMOTE" rev-parse refs/heads/main)" = "$CANDIDATE"
test "$(git --git-dir="$REMOTE" show refs/heads/main:wasi-push.txt)" = \
    "written and committed inside Wasmtime"
git --git-dir="$REMOTE" fsck --full

if run_guest push "$URL" /work/client refs/git-wasip2/integration-candidate \
    refs/heads/main >"$TEMPORARY/stale-push.log" 2>&1; then
    printf 'a stale one-commit push unexpectedly succeeded\n' >&2
    exit 1
fi
grep -F "StaleRemote" "$TEMPORARY/stale-push.log" >/dev/null

printf 'WASIp2 Smart HTTP fetch, commit, push, stale rejection, and fsck passed\n'
