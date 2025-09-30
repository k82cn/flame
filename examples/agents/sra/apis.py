
import flamepy

class Question(flamepy.Request):
    topic: str

class Answer(flamepy.Response):
    answer: str

class WebPage(flamepy.Request):
    url: str

class Script(flamepy.Request):
    language: str
    code: str

class EmbedRequest(flamepy.Request):
    inputs: str

class EmbedResponse(flamepy.Response):
    vector: list[float]