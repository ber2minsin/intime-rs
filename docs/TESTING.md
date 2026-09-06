# Testing

## How to run

```bash
cargo test -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -- --test-threads=1
```

Use `--test-threads=1` because platform unit tests share a process-global event queue and some daemon e2e tests temporarily change the process working directory for screenshot paths.

### Live OS tests (manual)

```bash
cargo test -p intime-platform --test linux_integration -- --ignored
cargo run -p intime-platform --example linux_smoke
```

## Suite inventory

Automated tests currently: **55 passed**, **1 ignored** (live Sway), excluding `intime-app`.

| Crate | Tests | Focus |
| --- | --- | --- |
| `intime-core` | 13 | models, fingerprints, JSON, timestamps |
| `intime-ai` | 10 | blob packing, URL validation, serde, unreachable HTTP |
| `intime-storage` | 15 | `TestDatabase`, apps/companies, all event variants, sqlite-vec search |
| `intime-platform` | 4 (+1 ignored) | shared queue; factories; Linux capture without panic |
| `intime-daemon` | 12 | screenshot coalescing units; pipeline e2e with fakes |
| `intime-migrate` | 1 | migrate idempotency + sqlite-vec |

## End-to-end tests

Daemon e2e tests (`crates/intime-daemon/tests/pipeline_e2e.rs`) are **in-process pipeline tests**:

| Piece | Implementation |
| --- | --- |
| Database | `intime_storage::testing::TestDatabase` (temp SQLite file + migrations + sqlite-vec) |
| Screenshots | `FakeCapture` implementing `ScreenshotSource` |
| Embeddings | `FakeEmbedding` implementing `EmbeddingServer` |
| Under test | `handle_incoming_event`, `ScreenshotOrchestrator`, `start_embedding_worker` |

They cover DB writes, app/company registration, screenshot coalescing, embedding queueing/storage, and failure paths without starting the daemon binary or a live desktop.

Live capture/focus belongs in ignored `intime-platform` Linux tests / `linux_smoke`.

## Coverage

```bash
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview

cargo llvm-cov -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate \
  --summary-only -- --test-threads=1
```

HTML:

```bash
cargo llvm-cov -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate \
  --html --output-dir target/llvm-cov -- --test-threads=1
```

### Latest measurement (2026-09-06)

`cargo llvm-cov` over the packages above:

| Metric | Covered | Total | Percent |
| --- | --- | --- | --- |
| Lines | 616 | 1109 | **55.55%** |
| Functions | 87 | 152 | **57.24%** |
| Regions | 835 | 1537 | **54.33%** |

Interpretation:

- Near-full coverage on `intime-core`, `intime-ai` models/helpers, storage repositories exercised by `TestDatabase`, and daemon `pipeline` / `orchestrator`.
- Low coverage on OS-native paths (`linux/atspi_hooks`, `linux/sway`, portal/grim branches) and binary `main` entrypoints — expected without a live desktop session or CLI invocation in CI.

Re-run the command above after large test changes and update this table.
