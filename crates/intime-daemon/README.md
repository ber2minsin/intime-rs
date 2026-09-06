Long-running process that turns platform events into persisted timeline rows, screenshots, and embedding jobs.

## Flow

1. Platform thread polls `EventSource`
2. Tracker task persists apps/events and may capture a screenshot
3. Screenshot policy coalesces rapid duplicate captures (see root README)
4. Embedding worker stores vectors when a screenshot was taken

Library entrypoints (for tests and embedding):

- `ScreenshotOrchestrator`
- `handle_incoming_event`
- `start_embedding_worker`

## Run

```bash
export DATABASE_URL=sqlite:intime.db?mode=rwc
cargo run -p intime-migrate
cargo run -p intime-daemon
```

## Testing

```bash
cargo test -p intime-daemon -- --test-threads=1
```

### End-to-end approach

`tests/pipeline_e2e.rs` drives the real pipeline with fakes:

| Piece | Implementation |
| --- | --- |
| Database | `intime_storage::testing::TestDatabase` (temp file + migrations + sqlite-vec) |
| Screenshots | `FakeCapture: ScreenshotSource` |
| Embeddings | `FakeEmbedding: EmbeddingServer` |
| Under test | `handle_incoming_event`, `ScreenshotOrchestrator`, `start_embedding_worker` |

This is not a full OS session test. For live capture/focus, use `intime-platform` ignored Linux tests / `linux_smoke`.

Unit tests in `orchestrator.rs` cover coalescing rules (debounce interval, new window, title change).
