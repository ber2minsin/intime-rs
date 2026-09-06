# End-to-end testing

intime e2e tests stage an environment and **replay scenarios**. They are not
GUI-driver tests of the daemon binary against your daily desktop, and they are
not trait-mocked unit tests with empty RGB buffers.

## Layers

```
scenario.json  -->  real pipeline  -->  real SQLite + sqlite-vec
                         |
                         +--> FixtureCapture (staged PNG/JPEG frames)
                         |
                         +--> EmbeddingService HTTP --> Docker embed-stub | CLIP
```

### Embedding

- **Default (CI / local fast path):** `docker/embed-stub` — same HTTP contract as
  the Python CLIP service, deterministic 512-d vectors, no torch download.
- **Optional realism:** compose profile `clip` builds `crates/intime-ai/python`
  and serves real OpenCLIP embeddings on port 8001.

### Screenshots

- **Scenario replay:** fixture files under `scenarios/*/fixtures/` loaded by
  `FixtureCapture`. Treat these as recorded frames for a story you want to
  assert against.
- **Live Linux capture:** compose profile `linux-desktop` (headless Sway + grim).
  Use for ignored platform integration tests.

### Persistence

Always real: migrations, foreign keys, sqlite-vec search, embedding rows.

## Commands

```bash
./scripts/e2e.sh
```

Equivalent manual flow:

```bash
docker compose up -d --build embed-stub
curl -sf http://127.0.0.1:8000/
EMBEDDING_SERVER_URL=http://127.0.0.1:8000 cargo test -p intime-e2e -- --nocapture
```

Full CLIP (slow first pull):

```bash
docker compose --profile clip up -d --build intime-ai
EMBEDDING_SERVER_URL=http://127.0.0.1:8001 cargo test -p intime-e2e -- --nocapture
```

Live Sway capture container:

```bash
docker compose --profile linux-desktop build linux-desktop
docker compose --profile linux-desktop run --rm linux-desktop \
  cargo test -p intime-platform --test linux_integration -- --ignored --nocapture
```

## Scenario format

`scenarios/mail_compose/scenario.json` (excerpt):

```json
{
  "name": "mail_compose",
  "steps": [
    { "type": "app_seen", "handle": 99, "title": "Compose", "file_path": "/usr/bin/mail", "company": "MailCorp", "fixture": "compose.png" },
    { "type": "window_focus", "handle": 99, "title": "Compose", "focused_element": "To" },
    { "type": "title_change", "handle": 99, "new_title": "Writing to Alice", "fixture": "inbox.png" },
    { "type": "idle_start" }
  ],
  "expect": {
    "min_events": 4,
    "event_types": ["app_seen", "window_focus", "title_change", "idle_start"],
    "min_screenshots": 2,
    "min_embeddings": 2,
    "company": "MailCorp"
  }
}
```

Attach `fixture` only on steps that will capture under the runner’s coalescing
policy (default interval 0ms: first sight + title/handle/fingerprint changes).

## Relation to `intime-daemon` pipeline tests

`crates/intime-daemon/tests/pipeline_e2e.rs` remains a **fast in-process** suite
with lightweight fakes for coalescing and failure paths. Prefer
`intime-e2e` + Docker when validating the HTTP embedding contract and fixture
replay stories.
