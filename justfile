set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

export DATABASE_URL := "sqlite://crates/intime-storage/dev.db?mode=rwc"

# Always works from zero — no cache, no prior state needed
reset-db:
    cargo sqlx database drop -y
    just setup

setup:
    cargo sqlx database create
    cargo run -p intime-migrate

# Run after adding/changing any query! macro, commit the .sqlx dir
prepare:
    cargo sqlx prepare --workspace -- --all-targets

[working-directory: 'crates\intime-app']
tauri-dev:
    bun install
    bun tauri dev