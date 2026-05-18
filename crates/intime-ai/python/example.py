import torch
import open_clip
from PIL import Image

labels = [
    "a desktop screenshot",
    "source code",
    "a video game",
    "a web browser",
    "food",
    "minecraft",
    "fork"
]

print("loading model...")

model, _, preprocess = open_clip.create_model_and_transforms(
    "ViT-B-32",
    pretrained="openai"
)

tokenizer = open_clip.get_tokenizer("ViT-B-32")
model.eval()

print("loading image...")

image = preprocess(
    Image.open("test.jpg").convert("RGB")
).unsqueeze(0)

text = tokenizer(labels)

print("running inference...")

with torch.no_grad():
    image_features = model.encode_image(image)
    text_features = model.encode_text(text)

    image_features /= image_features.norm(dim=-1, keepdim=True)
    text_features /= text_features.norm(dim=-1, keepdim=True)

    similarity = image_features @ text_features.T

    # raw scores
    print("\nRAW SCORES:")
    for label, score in zip(labels, similarity[0]):
        print(f"{label:20s} {score.item():.4f}")

    # ranking
    values, indices = similarity[0].sort(descending=True)

    print("\nTOP MATCH:")
    print(labels[indices[0]], float(values[0]))

    print("\nCONFIDENCE (margin):")
    print(float(values[0] - values[1]))

print("done")