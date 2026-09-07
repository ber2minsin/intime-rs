<p align="center">
  <img src="banner.svg" alt="intime Banner" width="45%">
</p>
<p align="center">
  <img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/ber2minsin/intime-rs">
  <img alt="GitHub contributors" src="https://img.shields.io/github/contributors/ber2minsin/intime-rs">
  <img alt="Issues" src="https://img.shields.io/github/issues/ber2minsin/intime-rs">
  <img alt="Rust Version" src="https://img.shields.io/badge/rust-1.85+-orange">
  <img alt="License" src="https://img.shields.io/github/license/ber2minsin/intime-rs">
</p>

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
| `intime-e2e` | Staged scenario replay / Docker e2e |
| `intime-app` | Tauri desktop UI (optional for backend work) |

## Quick start (backend)

```bash
# system deps (Fedora — pick portal for your desktop)
sudo dnf install dbus-devel at-spi2-core
# Sway: grim xdg-desktop-portal-wlr
# GNOME: xdg-desktop-portal-gnome
# KDE:   xdg-desktop-portal-kde

cp .env.example .env   # set DATABASE_URL
cargo run -p intime-migrate
cargo run -p intime-daemon
```

Linux capture prefers **Sway IPC** when available, otherwise **AT-SPI** (GNOME, KDE, Hyprland, generic Wayland). Screenshots use `grim` on Sway and the XDG portal elsewhere.

Embedding service (optional, for image/text vectors): see `crates/intime-ai/python`.

## Event metadata

Beyond the typed `EventData` payload (focus/title/text/app_seen/idle/etc.), every event stores an `EventMetadata` blob (also serialized into `event.payload`) and denormalized SQL columns:

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
| `document_path` / `document_name` | File being edited when known |
| `url` | Browser URL when extractable |
| `workspace_path` | Editor workspace / project hint |

App identity (AUMID, company, version, signature) is normalized into dedicated tables — see `docs/DATA_MODEL.md`.

### Feature flags

Opt out of intrusive or AI-related behavior via env (see `.env.example`):

- `INTIME_SCREENSHOTS_ENABLED` / `INTIME_EMBEDDINGS_ENABLED`
- `INTIME_RICH_UI_METADATA` / `INTIME_DOCUMENT_CONTEXT`
- `INTIME_SESSION_GROUPING` / `INTIME_SESSION_LLM`

## Screenshot policy

`ScreenshotOrchestrator` coalesces captures so rapid UI churn does not stall the desktop.

Stored JPEGs under `data/screenshots/` are age-managed by a daemon retention sweep
(default: full quality 2 days → compact → delete after 7 days; soft 512MB budget).
Embeddings remain after files are removed. Mark a session `important` to preserve
its screenshots — see `docs/DATA_MODEL.md` and `.env.example`.


1. Only events with a `window_handle` are candidates.
2. Capture key = `(handle, app fingerprint, title)`.
3. Always capture on first sight of a key, or when the window handle / app fingerprint changes.
4. For the same window+app, capture again only after a minimum interval (default **750ms**) **and** only if the title changed.
5. Capture failures are logged; the event is still persisted without a screenshot.

This replaced “screenshot every focus/title event,” which caused lag under busy UI Automation / AT-SPI streams.

## Testing

### Offline suite

```bash
cargo test -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -- --test-threads=1
```

### Staged e2e (Docker + scenario replay)

```bash
./scripts/e2e.sh
```

Uses real SQLite/sqlite-vec, real daemon pipeline code, real HTTP embedding client, fixture screenshot frames, and a staged embedding server (`embed-stub`). See `docs/E2E.md`.

### Coverage

```bash
cargo llvm-cov -p intime-core -p intime-ai -p intime-storage -p intime-platform -p intime-daemon -p intime-migrate -p intime-e2e \
  --summary-only -- --test-threads=1
```

Live desktop (ignored by default; needs Sway + grim):

```bash
cargo test -p intime-platform -- --ignored
```

Full details: `docs/TESTING.md`, `docs/E2E.md`.

## Platform notes

- Linux/Sway: Sway IPC + `grim`
- Linux/GNOME and others: AT-SPI events + XDG portal screenshots
- Windows: UI Automation focus/text metadata + DXGI capture

Each crate has its own `README.md` with API/module details.
