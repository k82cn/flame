
import flamepy
import asyncio

from api import WebPage, Summary

CRAWLER_APP_NAME = "crawler-app"

class CrawlerInformer(flamepy.TaskInformer):
    def on_update(self, task: flamepy.Task):
        if task.is_failed():
            print(task.events)
        elif task.is_completed():
            summary = Summary.from_json(task.output)
            with open(f"task_{task.id}.txt", "w") as f:
                f.write("\n".join(summary.links))
                f.write("\n")
                f.write("\n")
                f.write(summary.content)
                f.write("\n")

    def on_error(self):
        print("Error")

async def crawl_web_pages():
    crawler = await flamepy.create_session(CRAWLER_APP_NAME)

    web_pages = [
        WebPage(url="https://www.nvidia.com"),
        WebPage(url="https://www.microsoft.com"),
        WebPage(url="https://www.apple.com"),
        WebPage(url="https://www.amazon.com"),
        WebPage(url="https://www.google.com"),
        WebPage(url="https://www.facebook.com"),
        WebPage(url="https://www.oracle.com"),
        WebPage(url="https://www.meta.com/"),
        WebPage(url="https://www.tsmc.com/"),
    ]

    tasks = []

    for web_page in web_pages:
        task = crawler.invoke(web_page, CrawlerInformer())
        tasks.append(task)

    await asyncio.gather(*tasks)

    await crawler.close()

if __name__ == "__main__":
    asyncio.run(crawl_web_pages())