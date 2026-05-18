import sys
import json
import torch
import open_clip
from PIL import Image

# -----------------------
# init model
# -----------------------
print("loading model...", file=sys.stderr)

device = "cuda" if torch.cuda.is_available() else "cpu"

model, _, preprocess = open_clip.create_model_and_transforms(
    "ViT-B-32",
    pretrained="openai"
)

tokenizer = open_clip.get_tokenizer("ViT-B-32")

model = model.to(device)
model.eval()

print("ready", file=sys.stderr)


# -----------------------
# embedding functions
# -----------------------
def embed_image(path: str):
    image = preprocess(Image.open(path).convert("RGB")).unsqueeze(0).to(device)

    with torch.no_grad():
        feat = model.encode_image(image)
        feat = feat / feat.norm(dim=-1, keepdim=True)

    return feat[0].cpu().tolist()


def embed_text(text: str):
    tokens = tokenizer([text]).to(device)

    with torch.no_grad():
        feat = model.encode_text(tokens)
        feat = feat / feat.norm(dim=-1, keepdim=True)

    return feat[0].cpu().tolist()


# -----------------------
# IPC loop (Rust-compatible)
# -----------------------
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue

    try:
        req = json.loads(line)

        if req["type"] == "image":
            vec = embed_image(req["image_path"])

        elif req["type"] == "text":
            vec = embed_text(req["text"])

        else:
            vec = None

        resp = {
            "backend": "ViT-B-32",
            "embedding": vec,
            "embedding_type": req["type"]
        }

        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()

    except Exception as e:
        sys.stdout.write(json.dumps({
            "error": str(e)
        }) + "\n")
        sys.stdout.flush()