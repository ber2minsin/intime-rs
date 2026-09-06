# intime-e2e

Staged environment / scenario-replay tests for the intime pipeline.

## What is “real” vs staged

| Layer | Implementation |
| --- | --- |
| SQLite + sqlite-vec | Real (`TestDatabase`) |
| Daemon pipeline | Real (`handle_incoming_event`, orchestrator, embedding worker) |
| Embedding HTTP client | Real (`EmbeddingService`) |
| Embedding server | Staged Docker service (`embed-stub` or full CLIP) |
| Screenshots | Staged fixture PNGs replayed through `FixtureCapture` (recorded/authored frames) |

Live compositor capture (grim/Sway) is a separate Docker profile (`linux-desktop`), not mocked inside Rust.

## Run

```bash
# from repo root
./scripts/e2e.sh
```

Or manually:

```bash
docker compose up -d --build embed-stub
export EMBEDDING_SERVER_URL=http://127.0.0.1:8000
cargo test -p intime-e2e -- --nocapture
```

Without `EMBEDDING_SERVER_URL`, scenario tests skip (print a message) so the default unit suite stays offline-friendly.

## Authoring scenarios

Add a folder under `scenarios/<name>/`:

- `scenario.json` — ordered steps + expectations
- `fixtures/*.png` — images consumed when a step triggers a capture

See `docs/E2E.md`.
