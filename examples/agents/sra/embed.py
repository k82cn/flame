import requests
import os
import logging

logger = logging.getLogger(__name__)


class EmbeddingClient:
    """
    A client for the Embedding API.
    """

    def __init__(self, api_key: str = None, model: str = "Qwen/Qwen3-Embedding-0.6B"):
        """
        Initialize the EmbeddingClient.

        Args:
            api_key: The API key for the Embedding API.
            model: The model to use for the embedding.
        """

        if api_key is None:
            api_key = os.getenv("SILICONFLOW_API_KEY")

        self.api_key = api_key
        self.model = model
        self.headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        }
        self.api_url = "https://api.siliconflow.cn/v1/embeddings"

    def embed(self, text: str) -> list[float]:
        """
        Embed the given text using the Embedding API.

        Args:
            text: The text to embed.

        Returns:
            A list of floats representing the embedding.
        """
        if text is None or text == "":
            return []

        logger.info(
            "Embedding text: %s", text[:100] + "..." if len(text) > 100 else text
        )

        # Make API call to Embedding API
        payload = {
            "model": self.model,
            "input": text,
            "encoding_format": "float",
            "dimensions": 1024,
        }

        response = requests.post(self.api_url, headers=self.headers, json=payload)

        if response.status_code != 200:
            raise RuntimeError(
                f"Embedding API Error: {response.status_code} - {response.json()}"
            )

        result = response.json()

        return result["data"][0]["embedding"] if result["data"] else []
