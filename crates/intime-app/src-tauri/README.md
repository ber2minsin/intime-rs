# intime-app (Tauri)

Rust side of the desktop application. Workspace member path: `crates/intime-app/src-tauri`.

## Role

- Boots the Tauri runtime
- Exposes commands such as embedding search against `intime-storage`
- Shares models/clients from `intime-ai` and `intime-storage`

## Build

Prefer the app-level scripts from `crates/intime-app`. Direct cargo usage:

```bash
cargo check -p intime-app
```

Requires desktop/system libraries expected by Tauri on the host OS.
