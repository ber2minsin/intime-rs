# intime-platform

OS abstraction for activity events and screenshots.

## Public API

- `create_event_source()` — blocking `EventSource::poll()` producer
- `create_capture_engine()` — `ScreenshotSource::capture(handle)`
- `CapturedImage` — RGB8 pixels

## Backends

### Linux

- Events: Sway IPC when `SWAYSOCK` is set, otherwise AT-SPI2
- Screenshots: `grim` region capture on Sway/wlroots, otherwise XDG Desktop Portal

See `src/linux/README.md` for package dependencies.

### Windows

- Events: UI Automation focus handler plus WinEvent hooks for title/value changes
- Screenshots: DXGI desktop duplication cropped to the window

## Testing

```bash
cargo test -p intime-platform
cargo run -p intime-platform --example linux_smoke   # Sway live smoke
cargo test -p intime-platform --test linux_integration -- --ignored
```
