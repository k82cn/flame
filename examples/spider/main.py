
import markitdown
from transformers import pipeline
import flamepy

from api import WebPage, Summary

ins = flamepy.FlameInstance()

@ins.entrypoint
def spider_agent(wp: WebPage) -> Summary:
    md = markitdown.MarkItDown()
    result = md.convert(wp.url)

    return Summary(url=wp.url, summary=result.text_content)

if __name__ == "__main__":
    ins.run()
