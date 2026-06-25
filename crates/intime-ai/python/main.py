import asyncio
import logging
from contextlib import asynccontextmanager
from io import BytesIO

from fastapi import FastAPI, File, HTTPException, UploadFile, status

import embedding
from models import (
    BackendResponse,
    CLIPModel,
    DEFAULT_MODEL,
    EmbeddingResponse,
    EmbeddingType,
    StatusResponse,
    parse_clip_model,
)

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Shared model state
# ---------------------------------------------------------------------------

class ModelState:
    """
    Holds the active CLIP model and a lock that blocks embedding requests
    while a model swap is in progress.
    """

    def __init__(self):
        self.preprocess = None
        self.model = None
        self.tokenizer = None
        self.current: CLIPModel | None = None # pyright: ignore[reportInvalidTypeForm]
        self.is_loading: bool = False
        self._lock = asyncio.Lock()

    async def load(self, clip_model: CLIPModel) -> None: # pyright: ignore[reportInvalidTypeForm]
        async with self._lock:
            self.is_loading = True
            try:
                model_name, pretrained = parse_clip_model(clip_model.value)
                loop = asyncio.get_event_loop()
                result = await loop.run_in_executor(
                    None, embedding.init_model, model_name, pretrained
                )
                self.preprocess, self.model, self.tokenizer = result
                self.current = clip_model
            finally:
                self.is_loading = False

    def require_ready(self) -> None:
        """Raise 503 if a model swap is in progress."""
        if self.is_loading:
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail="Model is currently being swapped. Please retry shortly.",
            )


_state = ModelState()


# ---------------------------------------------------------------------------
# Lifespan
# ---------------------------------------------------------------------------

@asynccontextmanager
async def lifespan(app: FastAPI):
    await _state.load(DEFAULT_MODEL)
    yield


# ---------------------------------------------------------------------------
# App
# ---------------------------------------------------------------------------

app = FastAPI(
    title="CLIP Embedding Service",
    description="Embed images and text with OpenCLIP models.",
    lifespan=lifespan,
)


# ---------------------------------------------------------------------------
# Routes
# ---------------------------------------------------------------------------

@app.get("/", response_model=StatusResponse)
def root():
    return StatusResponse(status="ok", model_name=_state.current)


@app.post("/embed_image", response_model=EmbeddingResponse)
async def embed_image_route(image_file: UploadFile = File(...)):
    _state.require_ready()

    content = await image_file.read()
    loop = asyncio.get_event_loop()
    feat = await loop.run_in_executor(
        None, embedding.embed_image, BytesIO(content), _state.preprocess, _state.model,
    )

    return EmbeddingResponse(
        embedding_type=EmbeddingType.IMAGE,
        backend=_state.current,
        embedding=feat,
    )


@app.post("/embed_text", response_model=EmbeddingResponse)
async def embed_text_route(text: str):
    """POST keeps long/sensitive strings out of server logs."""
    _state.require_ready()

    loop = asyncio.get_event_loop()
    feat = await loop.run_in_executor(
        None, embedding.embed_text, text, _state.tokenizer, _state.model,
    )

    return EmbeddingResponse(
        embedding_type=EmbeddingType.TEXT,
        backend=_state.current,
        embedding=feat,
    )


@app.get("/backend", response_model=BackendResponse)
def get_backend():
    """Return the currently loaded model."""
    return BackendResponse(model_name=_state.current)


@app.post("/backend", response_model=BackendResponse, status_code=status.HTTP_202_ACCEPTED)
async def swap_backend(model_name: CLIPModel): # pyright: ignore[reportInvalidTypeForm]
    """
    Hot-swap the CLIP model.

    `model_name` must be a valid "ModelName/pretrained_tag" value from the
    CLIPModel enum (e.g. "ViT-L-14/openai").  All embedding routes return 503
    while the new model is downloading/loading.
    """
    if model_name == _state.current and not _state.is_loading:
        return BackendResponse(model_name=_state.current)

    logger.info("Model swap: %s → %s", _state.current, model_name.value)

    try:
        await _state.load(model_name)
    except Exception as exc:
        logger.exception("Model swap failed.")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to load model '{model_name.value}': {exc}",
        ) from exc

    return BackendResponse(model_name=_state.current)