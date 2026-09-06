# intime-migrate

Applies sqlx migrations from `intime-storage/migrations` and registers sqlite-vec before connecting.

## Library

`intime_migrate::run_migrations(database_url)` is the shared entrypoint used by the CLI and tests.

## Run

```bash
export DATABASE_URL=sqlite:intime.db?mode=rwc
cargo run -p intime-migrate
```

## Testing

```bash
cargo test -p intime-migrate
```

Creates a temporary database, runs migrations twice (idempotency), and verifies schema tables plus `vec_distance_cosine`.
