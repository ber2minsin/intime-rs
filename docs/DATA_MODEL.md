# Data model

How intime stores activity so later search / AI has enough context without relying
only on screenshots.

## Layers

```
raw Event (+ EventMetadata)
        |
        v
normalized identity tables (company, aumid, app, version, signature)
        |
        v
queryable event columns (title, document, url, focused UI, …)
        |
        v
session merges (category + context_key; rules now; optional LLM later)
```

## Identity tables

App details that used to live only inside `AppSeen` JSON are normalized:

| Table | Role |
| --- | --- |
| `company` | Unique company / publisher name |
| `aumid` | Unique app user model id / desktop app id (e.g. `brave-browser`) |
| `app` | Fingerprint hub; links `company_id`, `aumid_id`, path, display name |
| `app_version_info` | Version resource fields |
| `app_signature` | Code-signing publisher / subject |

`ensure_app()` upserts these on `AppSeen` so later `WindowFocus` rows only need a
fingerprint → `app_id` join.

## Event rows

Each `event` still keeps a JSON `payload`, but also stores denormalized columns:

- window handle / title / process / executable
- focused element, class, control type, automation id
- document path / name, url, workspace path
- `text_changed` flag (never stores typed key content)
- optional `session_id`

Discrete verbs (`form_submit`, `play_media`, `finish`, …) are ordinary
`event_type = ui_action` rows — there is no separate `action` table.

A focus event without a screenshot should still be searchable by title, app,
document, and UI focus fields.

## Categories, rules, and sessions

| Table | Role |
| --- | --- |
| `category` | Flat taxonomy (~48 seeds: coding, BI, HR, social short/long, official/unofficial streaming, …) |
| `activity_rule` | 1000+ matchers on app / title / url / path / company (`source`: seed, user, llm); regenerate via `scripts/generate_activity_seed.py` |
| `session` | Merge of same `category_id` + `context_key` (+ `app_id`) after promotion |

Heuristic `SessionTracker` (daemon):

1. Match each event with `activity_rule` → category + `context_key`
2. Buffer until meaningful duration / event count (or high-value UiAction such as `play_media`)
3. Open one session row; attach later events with the same context
4. Keep capturing screenshots on that page (2s heartbeat while focus is stable)

Media contexts (`media_streaming_*`, `media_local`, `music_listening`, …) merge on
normalized media title. Browser MPRIS players match via `automation_id` (e.g. brave)
plus title/URL — not bare `focused_control_type=mpris` (that is only a low-priority fallback).

`UiAction` events never become session children of their own — they attach to the
open session when one exists, and `play_media` can promote pending activity early.
`pause_media` does not open a new session by itself.

Source is `heuristic` today. `INTIME_SESSION_LLM` reserves on-demand / idle LLM
rule adjustments without enabling a model by default.

## Linux app identity

Linux fills company / version the same storage path Windows uses:

- `.desktop` Name / StartupWMClass / X-Flatpak
- AppStream / Flatpak `.metainfo.xml` developer + release version
- Path heuristics (e.g. `/opt/brave.com/…` → Brave Software)

## Feature flags

| Env | Default | Effect |
| --- | --- | --- |
| `INTIME_SCREENSHOTS_ENABLED` | true | Capture screenshots |
| `INTIME_EMBEDDINGS_ENABLED` | true | Queue embedding HTTP jobs |
| `INTIME_RICH_UI_METADATA` | true | Persist focused UI fields |
| `INTIME_DOCUMENT_CONTEXT` | true | Parse titles into document/url/workspace |
| `INTIME_SESSION_GROUPING` | true | Write session rows from category/context merges |
| `INTIME_SESSION_LLM` | false | Future LLM activity-rule / session labeling |

Flags are loaded via `FeatureFlags::from_env()` and sanitize metadata before
persistence when the user opts out.
