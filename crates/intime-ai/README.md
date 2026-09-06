# intime-ai

HTTP client for the local embedding FastAPI service.

## API

- `EmbeddingServer` trait
- `EmbeddingService` — POST `/embed/text` and multipart `/embed/image`
- `vec_to_blob` / `blob_to_vec` — little-endian f32 packing for sqlite-vec

Python service sources live in `python/`.

## Testing

```bash
cargo test -p intime-ai
```
