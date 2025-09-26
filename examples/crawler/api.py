import flamepy

class WebPage(flamepy.Request):
    url: str

class Summary(flamepy.Response):
    links: list[str]
    content: str
