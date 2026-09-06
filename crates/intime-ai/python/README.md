# intime-ai (Python)

FastAPI CLIP embedding service used by the Rust `EmbeddingService` client.

## Endpoints

| Method | Path | Body |
| --- | --- | --- |
| GET | `/` | status + loaded model |
| POST | `/embed/text` | JSON `{"type":"text","text":"..."}` (Rust client shape) |
| POST | `/embed/image` | multipart field `image_file` |
| GET/POST | `/backend` | inspect / hot-swap OpenCLIP model |

## Run

```bash
# from repo root — lightweight stub for e2e
docker compose up -d --build embed-stub

# full CLIP (compose profile)
docker compose --profile clip up -d --build intime-ai
```

Local uv:

```bash
uv sync
uv run fastapi run src/intime_embed/main.py --port 8000
```
