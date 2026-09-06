#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

COMPOSE=(docker compose)
if ! docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker-compose)
fi

echo "==> Starting embed-stub"
"${COMPOSE[@]}" up -d --build embed-stub

echo "==> Waiting for embed-stub health"
for _ in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:8000/" >/dev/null; then
    break
  fi
  sleep 1
done
curl -sf "http://127.0.0.1:8000/" >/dev/null

echo "==> Unit / integration suite"
cargo test -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -- --test-threads=1

echo "==> Staged scenario e2e (real HTTP embeddings + fixture screenshots)"
INTIME_E2E_REQUIRED=1 EMBEDDING_SERVER_URL="http://127.0.0.1:8000" \
  cargo test -p intime-e2e -- --test-threads=1 --nocapture

echo "==> Done"
