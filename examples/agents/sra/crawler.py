
# /// script
# dependencies = [
#   "flamepy",
#   "markitdown",
#   "qdrant-client>=1.14.1",
#   "requests>=2.32.3",
# ]
# [tool.uv.sources]
# flamepy = { path = "/usr/local/flame/sdk/python" }
# ///

import markitdown
import qdrant_client
import flamepy
import requests
import io
import uuid

from qdrant_client.models import VectorParams, Distance, PointStruct

from apis import WebPage, Answer, EmbedRequest, EmbedResponse

ins = flamepy.FlameInstance()

headers = {
    'User-Agent': 'Xflops Crawler 1.0',
    'From': 'support@xflops.io'
}

@ins.entrypoint
def crawler(wp: WebPage) -> Answer:
    """
    Crawl the web and persist the content of the web page to the vector database.
    Return the content of the web page.

    Args:
        wp: WebPage object containing the url to crawl
    """

    text = requests.get(wp.url, headers=headers).text

    md = markitdown.MarkItDown()
    stream = io.BytesIO(text.encode("utf-8"))
    result = md.convert(stream).text_content

    client = qdrant_client.QdrantClient(host="qdrant", port=6333)
    if not client.collection_exists("sra"):
        client.create_collection(
            collection_name="sra",
            vectors_config=VectorParams(size=2560, distance=Distance.COSINE),
        )

    chunk_size = min(8192, len(result))

    for chunk in range(0, len(result), chunk_size):
        req = EmbedRequest(inputs=result[chunk:chunk+chunk_size])
        data = req.model_dump_json().encode("utf-8")
        resp = requests.post("http://embedding-api:8000/embed", data=data)
        if resp.status_code != 200:
            return Answer(answer=f"Failed to embed text from {wp.url}")
        vector = resp.json()["vector"]

        client.upsert(collection_name="sra", points=[
            PointStruct(
                id=f"{uuid.uuid4()}",
                vector=vector,
                payload={"url": wp.url, "chunk": chunk, "content": result[chunk:chunk+chunk_size]})
        ])

    return Answer(answer=f"Crawled {wp.url}")

if __name__ == "__main__":
    ins.run()
