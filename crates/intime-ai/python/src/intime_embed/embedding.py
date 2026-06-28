import sys
import json
import logging
from io import BytesIO

import torch
import open_clip
from PIL import Image

logger = logging.getLogger(__name__)

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"


# ---------------------------------------------------------------------------
# Model initialisation
# ---------------------------------------------------------------------------

def init_model(model_name: str = "ViT-B-32", pretrained: str = "openai"):
    """
    Load a CLIP model and return (preprocess, model, tokenizer).

    Both `model_name` and `pretrained` must be valid open_clip identifiers.
    Use open_clip.list_pretrained() to enumerate valid (model_name, pretrained)
    pairs — not every model has an "openai" checkpoint.
    """
    logger.info("Loading %s (pretrained=%s) on %s …", model_name, pretrained, DEVICE)

    model, _, preprocess = open_clip.create_model_and_transforms(
        model_name,
        pretrained=pretrained,
    )
    tokenizer = open_clip.get_tokenizer(model_name)

    model = model.to(DEVICE)
    model.eval()

    logger.info("%s/%s ready.", model_name, pretrained)
    return preprocess, model, tokenizer


# ---------------------------------------------------------------------------
# Embedding helpers
# ---------------------------------------------------------------------------

def embed_image(source: str | BytesIO, preprocess, model) -> list[float]:
    """
    Embed an image.

    `source` can be a file-system path (str) or an in-memory BytesIO object
    so that the FastAPI route doesn't have to touch the disk.
    """
    image = preprocess(Image.open(source).convert("RGB")).unsqueeze(0).to(DEVICE)

    with torch.no_grad():
        feat = model.encode_image(image)
        feat = feat / feat.norm(dim=-1, keepdim=True)

    return feat[0].cpu().tolist()


def embed_text(text: str, tokenizer, model) -> list[float]:
    """Embed a text string."""
    tokens = tokenizer([text]).to(DEVICE)

    with torch.no_grad():
        feat = model.encode_text(tokens)
        feat = feat / feat.norm(dim=-1, keepdim=True)

    return feat[0].cpu().tolist()


# ---------------------------------------------------------------------------
# IPC loop (Rust-compatible stdin/stdout protocol)
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    preprocess, model, tokenizer = init_model()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            req = json.loads(line)

            if req["type"] == "image":
                vec = embed_image(req["image_path"], preprocess, model)
            elif req["type"] == "text":
                vec = embed_text(req["text"], tokenizer, model)
            else:
                raise ValueError(f"Unknown request type: {req['type']!r}")

            resp = {
                "backend": "ViT-B-32/openai",
                "embedding": vec,
                "embedding_type": req["type"],
            }

        except Exception as exc:
            resp = {"error": str(exc)}

        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()