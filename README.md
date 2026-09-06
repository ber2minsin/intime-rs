# intime-rs

Local-first activity timeline and semantic search for desktop work.

The daemon captures window activity, optional screenshots, and embeddings so past work can be queried later (which app, which window/title, focused UI context, and similar screenshots).

## Workspace crates

| Crate | Role |
| --- | --- |
| `intime-core` | Shared event/app models |
| `intime-platform` | OS event sources and screenshot backends |
| `intime-storage` | SQLite + sqlite-vec persistence |
| `intime-ai` | Embedding client (FastAPI service) |
| `intime-daemon` | Runtime orchestrator |
| `intime-migrate` | Database migrations CLI |
| `intime-app` | Tauri desktop UI (optional for backend work) |

## Quick start (backend)

```bash
# system deps (Fedora example)
sudo dnf install dbus-devel grim at-spi2-core xdg-desktop-portal-wlr

cp .env.example .env   # set DATABASE_URL
cargo run -p intime-migrate
cargo run -p intime-daemon
```

Embedding service (optional, for image/text vectors): see `crates/intime-ai/python`.

## Event metadata

Beyond the typed `EventData` payload (focus/title/text/app_seen/idle/etc.), every event stores an `EventMetadata` blob (also serialized into `event.payload`):

| Field | Purpose |
| --- | --- |
| `window_title` | Current window title when known |
| `process_id` | Owning process id |
| `executable_path` | Path to the binary |
| `focused_element` | Accessibility / UI Automation name of the focused control |
| `focused_element_class` | Control class name |
| `focused_control_type` | Control type (e.g. Edit, Button) |
| `automation_id` | Stable automation id when the toolkit exposes one |
| `text_changed` | UI reported text activity without storing typed content |

Windows fills the focused-element fields via UI Automation. Linux currently focuses on title/process/path from Sway or AT-SPI; richer AT-SPI element fields can be extended later.

## Screenshot policy

`ScreenshotOrchestrator` coalesces captures so rapid UI churn does not stall the desktop:

1. Only events with a `window_handle` are candidates.
2. Capture key = `(handle, app fingerprint, title)`.
3. Always capture on first sight of a key, or when the window handle / app fingerprint changes.
4. For the same window+app, capture again only after a minimum interval (default **750ms**) **and** only if the title changed.
5. Capture failures are logged; the event is still persisted without a screenshot.

This replaced “screenshot every focus/title event,” which caused lag under busy UI Automation / AT-SPI streams.

## Testing

### Run the suite

```bash
# recommended: backend crates, single-threaded (shared platform queue / cwd)
cargo test -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -- --test-threads=1
```

Per crate:

```bash
cargo test -p intime-core
cargo test -p intime-ai
cargo test -p intime-storage
cargo test -p intime-platform
cargo test -p intime-daemon
cargo test -p intime-migrate
```

Live desktop (ignored by default; needs Sway + grim):

```bash
cargo test -p intime-platform --test linux_integration -- --ignored
cargo run -p intime-platform --example linux_smoke
```

### What the e2e tests use

Daemon pipeline tests (`crates/intime-daemon/tests/pipeline_e2e.rs`) are **library-level end-to-end** tests, not a full GUI session:

- `intime_storage::testing::TestDatabase` — temporary on-disk SQLite, migrations applied, **sqlite-vec** registered
- `FakeCapture` implementing `ScreenshotSource` — returns tiny RGB frames (no grim/DXGI/portal)
- `FakeEmbedding` implementing `EmbeddingServer` — returns a 512-dim vector without HTTP
- Real `handle_incoming_event` + `ScreenshotOrchestrator` + `start_embedding_worker`

That exercises app registration, event persistence (full JSON payload), screenshot coalescing, embedding queueing/storage, and failure paths.

Storage integration tests use the same `TestDatabase` helper for repository/sqlite-vec coverage.

### Coverage

Install once:

```bash
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
```

Then:

```bash
cargo llvm-cov -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate \
  --summary-only -- --test-threads=1
```

HTML report:

```bash
cargo llvm-cov -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate \
  --html --output-dir target/llvm-cov -- --test-threads=1
```

See `docs/TESTING.md` for the latest measured totals and suite inventory.

## Platform notes

- Linux/Sway: Sway IPC + `grim`
- Linux/GNOME and others: AT-SPI events + XDG portal screenshots
- Windows: UI Automation focus/text metadata + DXGI capture

Each crate has its own `README.md` with API/module details.
