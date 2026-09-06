#!/usr/bin/env bash
set -euo pipefail

mkdir -p "${XDG_RUNTIME_DIR}"
chmod 700 "${XDG_RUNTIME_DIR}"

# Minimal sway config: one workspace, start a terminal for focus/capture targets.
cat >/tmp/sway-e2e.conf <<'EOF'
output HEADLESS-1 resolution 1280x720
exec foot
EOF

sway -c /tmp/sway-e2e.conf >/tmp/sway.log 2>&1 &
SWAY_PID=$!

cleanup() {
  kill "${SWAY_PID}" 2>/dev/null || true
}
trap cleanup EXIT

# Wait until IPC is ready.
for _ in $(seq 1 50); do
  if [[ -S "${SWAYSOCK}" ]]; then
    break
  fi
  sleep 0.1
done

if [[ ! -S "${SWAYSOCK}" ]]; then
  echo "Sway failed to start" >&2
  cat /tmp/sway.log >&2 || true
  exit 1
fi

export SWAYSOCK
cd /workspace

if [[ $# -eq 0 ]]; then
  exec bash
else
  exec "$@"
fi
