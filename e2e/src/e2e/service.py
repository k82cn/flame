import flamepy

from e2e import TestRequest, TestResponse

instance = flamepy.FlameInstance()

@instance.entrypoint
def e2e_service_entrypoint(req: TestRequest) -> TestResponse:
    return TestResponse(session_id=req.session_id, task_id=req.task_id)

@instance.context
def e2e_service_context(req: TestRequest):
    pass

if __name__ == "__main__":
    instance.run()