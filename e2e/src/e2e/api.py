import flamepy

class TestRequest(flamepy.Request):
    pass

class TestResponse(flamepy.Response):
    session_id: str
    task_id: str

    def __init__(self, session_id: str, task_id: str):
        self.session_id = session_id
        self.task_id = task_id
