
import flamepy
import asyncio
from api import WebPage, Summary

SPIDER_AGENT_NAME = "spider-agent"

async def crawl_web_pages():
    crawler = await flamepy.create_session(SPIDER_AGENT_NAME)

    web_pages = [
        WebPage(url="https://www.google.com"),
        WebPage(url="https://www.baidu.com"),
        WebPage(url="https://www.bing.com"),
        WebPage(url="https://www.yahoo.com"),
        WebPage(url="https://www.wikipedia.org"),
        WebPage(url="https://www.youtube.com"),
    ]

    for web_page in web_pages:
        task = await crawler.invoke(web_page)
        summary = Summary.from_json(task.output)

        print(summary.summary)

if __name__ == "__main__":
    asyncio.run(crawl_web_pages())