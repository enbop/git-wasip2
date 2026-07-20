#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEMO_ROOT="$ROOT/target/git-wasip2-demo"
DEMO_WORKSPACE="$DEMO_ROOT/work"
ENV_FILE="$ROOT/target/git-wasip2-demo.env"
REMOTE="$DEMO_ROOT/remote.git"
SEED="$DEMO_ROOT/seed"
READY="$DEMO_ROOT/server.port"
SERVER_PID=

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$ENV_FILE"
    rm -rf "$DEMO_ROOT"
}
trap cleanup EXIT HUP INT TERM

if [ -e "$DEMO_ROOT" ] || [ -e "$ENV_FILE" ]; then
    printf 'demo state already exists; remove these paths before retrying:\n' >&2
    printf '  %s\n  %s\n' "$DEMO_ROOT" "$ENV_FILE" >&2
    exit 1
fi

mkdir -p "$DEMO_WORKSPACE"
git init --quiet --bare --initial-branch=main "$REMOTE"
git init --quiet --initial-branch=main "$SEED"
git -C "$SEED" config user.name "git-wasip2 demo"
git -C "$SEED" config user.email "demo@git-wasip2.invalid"
printf 'hello from the local Git remote\n' >"$SEED/README.txt"
git -C "$SEED" add README.txt
git -C "$SEED" commit --quiet -m "demo: seed local remote"
git -C "$SEED" remote add origin "$REMOTE"
git -C "$SEED" push --quiet origin main

python3 "$ROOT/tests/support/git-smart-http-server.py" \
    "$REMOTE" --ready-file "$READY" &
SERVER_PID=$!

attempt=0
while [ ! -s "$READY" ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 100 ] || ! kill -0 "$SERVER_PID" 2>/dev/null; then
        printf 'local Smart HTTP server did not become ready\n' >&2
        exit 1
    fi
    sleep 0.05
done

PORT=$(cat "$READY")
REMOTE_URL="http://127.0.0.1:$PORT/repo.git"
WASM="$ROOT/target/wasm32-wasip2/release/examples/git-wasip2.wasm"
cat >"$ENV_FILE" <<EOF
export DEMO_ROOT='$DEMO_ROOT'
export DEMO_WORKSPACE='$DEMO_WORKSPACE'
export REMOTE_REPOSITORY='$REMOTE'
export REMOTE_URL='$REMOTE_URL'
export GIT_WASIP2_WASM='$WASM'
EOF

printf '\nLocal test repository is ready. Keep this terminal open.\n\n'
printf 'Remote:      %s\n' "$REMOTE_URL"
printf 'Environment: %s\n\n' "$ENV_FILE"
printf 'In a second terminal, run:\n\n'
printf '  source %s\n\n' "$ENV_FILE"
printf 'Then follow the README commands. Press Ctrl+C here to stop and clean up.\n\n'

wait "$SERVER_PID"
