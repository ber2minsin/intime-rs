# intime-storage

SQLite persistence with sqlite-vec embeddings.

## Modules

- `storage::Storage` — facade over app/event/embedding repositories
- `testing::TestDatabase` — temp DB with migrations + sqlite-vec for tests
- SQL migrations under `migrations/`

## Schema

- `company`, `app` — app identity keyed by fingerprint
- `event` — timeline rows with JSON payload (full `Event` including metadata) and optional screenshot path
- `embedding` + `vec_embedding` — vector index (float[512])

## Testing

```bash
cargo test -p intime-storage
```

`TestDatabase` creates an on-disk temporary sqlite file (more reliable than `:memory:` with sqlite-vec and pooled connections), registers the extension, and runs migrations. See `docs/TESTING.md` for suite inventory and coverage.
