
import flamepy
from concurrent.futures import wait

from apis import WebPage, Summary

CRAWLER_APP_NAME = "crawler-app"

class CrawlerInformer(flamepy.TaskInformer):
    def on_update(self, task: flamepy.Task):
        if task.is_failed():
            print(task.events)
        elif task.is_completed():
            summary = task.output
            with open(f"task_{task.id}.txt", "w") as f:
                f.write("\n".join(summary.links))
                f.write("\n")
                f.write("\n")
                f.write(summary.content)
                f.write("\n")

    def on_error(self):
        print("Error")

def crawl_web_pages():
    crawler = flamepy.create_session(CRAWLER_APP_NAME)

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

    # Run all crawler tasks in parallel using the run() API
    futures = [crawler.run(web_page, CrawlerInformer()) for web_page in web_pages]

    # Wait for all tasks to complete
    wait(futures)

    print(f"Crawled {len(web_pages)} web pages successfully")

    crawler.close()

if __name__ == "__main__":
    crawl_web_pages()