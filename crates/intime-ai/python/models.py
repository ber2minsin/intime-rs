import enum


import open_clip
from pydantic import BaseModel


class EmbeddingType(str, enum.Enum):
    """Serialises as "text" / "image" in JSON."""
    TEXT = "text"
    IMAGE = "image"


def _build_clip_model_enum() -> type[enum.Enum]:
    """
    Build a string enum of every valid (model, pretrained) pair from open_clip.

    Only pairs listed by open_clip.list_pretrained() are included — models that
    exist in list_models() but have no downloadable weights are silently excluded
    to prevent runtime errors on load.

    Member name  (Python identifier): "ViT_B_32__openai"
    Member value (JSON / API string):  "ViT-B-32/openai"

    The "/" separator in the value is unambiguous because open_clip model names
    and pretrained tags never contain it.  The "__" in the member name clearly
    separates model from tag in Python code.
    """
    members: dict[str, str] = {}
    for model_name, pretrained_tag in open_clip.list_pretrained():
        key = (
            f"{model_name}__{pretrained_tag}"
            .replace("-", "_")
            .replace(".", "_")
            .replace(" ", "_")
        )
        members[key] = f"{model_name}/{pretrained_tag}"

    return enum.Enum("CLIPModel", members, type=str)  # type: ignore[return-value]


CLIPModel = _build_clip_model_enum()

# Convenience: the default model served on startup
DEFAULT_MODEL: CLIPModel = CLIPModel("ViT-B-32/openai")  # type: ignore[call-arg]


def parse_clip_model(value: str) -> tuple[str, str]:
    """
    Split a CLIPModel value ("ViT-B-32/openai") into (model_name, pretrained_tag).
    Safe to call on any member of the enum.
    """
    model_name, _, pretrained_tag = value.partition("/")
    return model_name, pretrained_tag


class EmbeddingResponse(BaseModel):
    embedding_type: EmbeddingType
    backend: CLIPModel # pyright: ignore[reportInvalidTypeForm]
    embedding: list[float]


class BackendResponse(BaseModel):
    model_name: CLIPModel # pyright: ignore[reportInvalidTypeForm]


class StatusResponse(BaseModel):
    status: str
    model_name: CLIPModel # pyright: ignore[reportInvalidTypeForm]