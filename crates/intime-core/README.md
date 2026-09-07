# intime-core

Shared domain types used by every other crate.

## Contents

- `Event` / `EventData` — activity timeline events
- `EventMetadata` — window/UI/document context (sanitized by feature flags)
- `AppDetails` — app identity, fingerprinting, display name helpers
- `FeatureFlags` — collection / AI opt-outs (`INTIME_*` env vars)
- `context` — title parsers for documents/URLs and coarse intent guesses
- `session` — session source labels and promotion policy
- `category` — activity rules + category matching
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
| `document_path` / `document_name` | Active file when known |
| `url` | Browser URL when known |
| `workspace_path` | Editor workspace hint |

See `docs/DATA_MODEL.md` for persistence layout.

## Tests

```bash
cargo test -p intime-core
```
