import flamepy

class WebPage(flamepy.Request):
    url: str

class Summary(flamepy.Response):
    url: str
    summary: str
