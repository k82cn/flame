# /// script
# dependencies = [
#   "huggingface_hub",
#   "transformers>=4.51.0",
#   "sentence-transformers>=2.7.0",
#   "fastapi>=0.116.0",
#   "uvicorn>=0.35.0",
#   "pydantic",
# ]
# ///

from huggingface_hub import snapshot_download
from sentence_transformers import SentenceTransformer
from fastapi import FastAPI, HTTPException
import uvicorn
import pydantic

class EmbedRequest(pydantic.BaseModel):
    inputs: str

class EmbedResponse(pydantic.BaseModel):
    vector: list[float]

app = FastAPI(name="embedding-api")

model_name = "Qwen/Qwen3-Embedding-4B"

# Download the model
snapshot_download(repo_id=model_name)

# Load the model
model = SentenceTransformer(model_name, device="cpu")

# We recommend enabling flash_attention_2 for better acceleration and memory saving,
# together with setting `padding_side` to "left":
# model = SentenceTransformer(
#     "Qwen/Qwen3-Embedding-4B",
#     model_kwargs={"attn_implementation": "flash_attention_2", "device_map": "auto"},
#     tokenizer_kwargs={"padding_side": "left"},
# )

@app.get("/health")
async def health_check():
    try:
        model.encode("embedding health check")
        return {"status": "healthy", "model": model_name}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to health check the model: {e}")

@app.post("/embed")
async def embed(request: EmbedRequest):
    try:
        embeddings = model.encode(request.inputs)
        return EmbedResponse(vector=embeddings.tolist())
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to embed the text: {e}")

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8000)