"""API-compatible embedding stub for staged e2e tests.

Speaks the same HTTP contract as `intime-ai` FastAPI (`/embed/text`, `/embed/image`)
but returns deterministic 512-d vectors without loading CLIP/torch. Use this in CI and
local scenario replay; swap to the full CLIP service via compose profile `clip`.
"""

from __future__ import annotations

import hashlib
import math
from io import BytesIO

from fastapi import FastAPI, File, UploadFile
from pydantic import BaseModel, Field

DIM = 512
BACKEND = "stub/deterministic-v1"

app = FastAPI(title="intime embed stub", version="1.0.0")


class StatusResponse(BaseModel):
    status: str = "ok"
    model_name: str = BACKEND


class EmbeddingRequestText(BaseModel):
    type: str = "text"
    text: str


class EmbeddingResponse(BaseModel):
    embedding_type: str
    backend: str = BACKEND
    embedding: list[float] = Field(min_length=DIM, max_length=DIM)


def _vector_from_digest(digest: bytes) -> list[float]:
    # Expand SHA-256 into DIM floats in [-1, 1], then L2-normalize.
    raw: list[float] = []
    seed = digest
    while len(raw) < DIM:
        seed = hashlib.sha256(seed).digest()
        for b in seed:
            raw.append((b / 127.5) - 1.0)
            if len(raw) >= DIM:
                break
    norm = math.sqrt(sum(v * v for v in raw)) or 1.0
    return [v / norm for v in raw]


@app.get("/", response_model=StatusResponse)
def root() -> StatusResponse:
    return StatusResponse()


@app.get("/healthz", response_model=StatusResponse)
def healthz() -> StatusResponse:
    return StatusResponse()


@app.post("/embed/text", response_model=EmbeddingResponse)
def embed_text(req: EmbeddingRequestText) -> EmbeddingResponse:
    digest = hashlib.sha256(req.text.encode("utf-8")).digest()
    return EmbeddingResponse(
        embedding_type="text",
        embedding=_vector_from_digest(digest),
    )


@app.post("/embed/image", response_model=EmbeddingResponse)
async def embed_image(image_file: UploadFile = File(...)) -> EmbeddingResponse:
    content = await image_file.read()
    digest = hashlib.sha256(content).digest()
    return EmbeddingResponse(
        embedding_type="image",
        embedding=_vector_from_digest(digest),
    )
