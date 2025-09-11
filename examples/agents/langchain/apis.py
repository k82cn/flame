
import flamepy

class SysPrompt(flamepy.Request):
    prompt: str

class Question(flamepy.Request):
    question: str

class Answer(flamepy.Response):
    answer: str
