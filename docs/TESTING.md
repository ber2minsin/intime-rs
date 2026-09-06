# Testing

## How to run

### Offline unit / integration

```bash
cargo test -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -- --test-threads=1
```

Use `--test-threads=1` because platform unit tests share a process-global event queue and some daemon tests temporarily change the process working directory for screenshot paths.

### Staged end-to-end (Docker)

```bash
./scripts/e2e.sh
```

This builds `embed-stub`, waits for health, runs the offline suite, then runs
`intime-e2e` scenario replay against real HTTP embeddings. Details: `docs/E2E.md`.

### Live OS tests (manual / linux-desktop profile)

```bash
cargo test -p intime-platform --test linux_integration -- --ignored
cargo run -p intime-platform --example linux_smoke

docker compose --profile linux-desktop run --rm linux-desktop \
  cargo test -p intime-platform --test linux_integration -- --ignored --nocapture
```

## Suite inventory

| Crate | Kind | Focus |
| --- | --- | --- |
| `intime-core` | unit + integration | models, fingerprints, JSON, timestamps |
| `intime-ai` | unit + integration | blob packing, URL validation, serde, unreachable HTTP |
| `intime-storage` | integration | `TestDatabase`, apps/companies, event variants, sqlite-vec, `Storage::connect` |
| `intime-platform` | unit + factories | frame buffer, PNG decode, shared queue, Linux capture without panic |
| `intime-daemon` | unit + in-process e2e | screenshot coalescing; pipeline with lightweight fakes |
| `intime-migrate` | integration | migrate idempotency + sqlite-vec |
| `intime-e2e` | staged scenario e2e | fixture screenshots + real `EmbeddingService` + Docker stub/CLIP |

## Coverage

```bash
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview

cargo llvm-cov -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -p intime-e2e \
  --summary-only -- --test-threads=1
```

For coverage that includes staged HTTP e2e, start embed-stub first and export
`EMBEDDING_SERVER_URL=http://127.0.0.1:8000`.

### Latest measurement (2026-09-06, with `EMBEDDING_SERVER_URL` + embed-stub)

| Metric | Covered | Total | Percent |
| --- | --- | --- | --- |
| Lines | 892 | 1398 | **63.81%** |
| Functions | 113 | 180 | **62.78%** |
| Regions | 1217 | 1949 | **62.44%** |

Near-full on core/AI helpers, storage facade, daemon orchestrator/pipeline, and staged e2e runner.
Largest remaining gaps: OS-native loops (`atspi_hooks`, full Sway event loop), binary `main`s, and DXGI/Windows paths (not instrumented on Linux hosts).
