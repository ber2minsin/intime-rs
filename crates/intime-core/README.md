# intime-core

Shared domain types used by every other crate.

## Contents

- `Event` / `EventData` — activity timeline events (`window_focus`, `title_change`, `text_changed`, `app_seen`, idle/gap/background)
- `EventMetadata` — richer context stored alongside each event (and in `event.payload` JSON)
- `AppDetails` — app identity, fingerprinting, display name helpers
- `Timestamp` — UTC timestamps and screenshot-safe filename formatting

## EventMetadata fields

| Field | Meaning |
| --- | --- |
| `window_title` | Window title when known |
| `process_id` | Owning PID |
| `executable_path` | Binary path |
| `focused_element` | Focused control name (UI Automation / a11y) |
| `focused_element_class` | Control class |
| `focused_control_type` | Control type string |
| `automation_id` | Toolkit automation id |
| `text_changed` | Text activity flag (content not stored) |

## Design notes

Fingerprints are stable blake3 digests preferring AUMID, then company+product, then executable basename. They are intended to survive reinstalls and path changes when possible.

## Tests

```bash
cargo test -p intime-core
```
