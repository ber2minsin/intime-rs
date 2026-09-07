# intime-storage

SQLite persistence with sqlite-vec embeddings.

## Modules

- `storage::Storage` — facade over app/event/embedding/session repositories
- `testing::TestDatabase` — temp DB with migrations + sqlite-vec for tests
- SQL migrations under `migrations/`

## Schema

- `company`, `aumid`, `app`, `app_version_info`, `app_signature` — normalized identity
- `event` — timeline rows with JSON payload **and** queryable context columns
- `session`, `category`, `activity_rule` — category+context merges and editable matchers
- `embedding` + `vec_embedding` — vector index (float[512])
- `setting` — key/value knobs for future UI-persisted prefs

See `docs/DATA_MODEL.md`.

## Testing

```bash
cargo test -p intime-storage
```

`TestDatabase` creates an on-disk temporary sqlite file (more reliable than `:memory:` with sqlite-vec and pooled connections), registers the extension, and runs migrations. See `docs/TESTING.md` for suite inventory and coverage.
