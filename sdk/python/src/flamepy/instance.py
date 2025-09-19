
"""
Copyright 2025 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
"""

import inspect
import uvicorn
import os
from pydantic import BaseModel
from fastapi import FastAPI, Request as FastAPIRequest, Response as FastAPIResponse

from .service import FlameService, SessionContext, TaskContext, TaskOutput, run as run_service, ApplicationContext
from .types import Shim
import logging

FLAME_EXECUTOR_ID = "FLAME_EXECUTOR_ID"

debug_service = None

class FlameInstance(FlameService):
    def __init__(self):
        self._entrypoint = None
        self._parameter = None
        self._return_type = None
        self._input_schema = None
        self._output_schema = None

        self._context = None
        self._context_schema = None
        self._context_parameter = None

    def context(self, func):
        logger = logging.getLogger(__name__)
        logger.debug(f"context: {func.__name__}")

        sig = inspect.signature(func)
        self._context = func
        assert len(sig.parameters) == 1 or len(sig.parameters) == 0, "Context must have exactly zero or one parameter"
        for param in sig.parameters.values():
            assert param.kind == inspect.Parameter.POSITIONAL_OR_KEYWORD, "Parameter must be positional or keyword"
            if param.annotation is not inspect._empty:
                self._context_schema = param.annotation.model_json_schema()
            self._context_parameter = param

    def entrypoint(self, func):
        logger = logging.getLogger(__name__)
        logger.debug(f"entrypoint: {func.__name__}")

        sig = inspect.signature(func)
        self._entrypoint = func
        assert len(sig.parameters) == 1 or len(sig.parameters) == 0, "Entrypoint must have exactly zero or one parameter"
        for param in sig.parameters.values():
            assert param.kind == inspect.Parameter.POSITIONAL_OR_KEYWORD, "Parameter must be positional or keyword"
            if param.annotation is not inspect._empty:
                self._input_schema = param.annotation.model_json_schema()
            self._parameter = param

        if sig.return_annotation is not inspect._empty:
            self._return_type = sig.return_annotation
            self._output_schema = self._return_type.model_json_schema()

    async def on_session_enter(self, context: SessionContext):
        logger = logging.getLogger(__name__)
        logger.debug("on_session_enter")
        if self._context is None:
            logger.warning("No context function defined")
            return
        if self._context_parameter is None:
            self._context()
        else:
            obj = self._context_parameter.annotation.model_validate_json(context.common_data)
            self._context(obj)

    async def on_task_invoke(self, context: TaskContext) -> TaskOutput:
        logger = logging.getLogger(__name__)
        logger.debug("on_task_invoke")
        if self._entrypoint is None:
            logger.warning("No entrypoint function defined")
            return

        if self._parameter is not None:
            obj = self._parameter.annotation.model_validate_json(context.input)
            res = self._entrypoint(obj)
        else:
            res = self._entrypoint()

        res = self._return_type.model_validate(res).model_dump_json()
        logger.debug(f"on_task_invoke: {res}")

        return TaskOutput(data=res.encode("utf-8"))

    async def on_session_leave(self):
        logger = logging.getLogger(__name__)
        logger.debug("on_session_leave")

    def run(self):
        logger = logging.getLogger(__name__)
        try:
            # Run the service
            exec_id = os.getenv(FLAME_EXECUTOR_ID)
            if exec_id is not None:
                # If the instance was started by executor, run the service.
                logger.info("🚀 Starting Flame Instance")
                logger.info("=" * 50)

                run_service(self)
            else:
                # If the instance was started manually, run the debug service.
                logger.info("🚀 Starting Flame Debug Instance")
                logger.info("=" * 50)

                run_debug_service(self)
        
        except KeyboardInterrupt:
            logger.info("\n🛑 Server stopped by user")
        except Exception as e:
            logger.error(f"\n❌ Error: {e}")

def run_debug_service(instance: FlameInstance):
    global debug_service
    debug_service = FastAPI()
    debug_service.state.instance = instance

    if instance._context is not None:   
        context_name = instance._context.__name__
        debug_service.add_api_route(f"/{context_name}", context_api, methods=["POST"])

    if instance._entrypoint is not None:
        entrypoint_name = instance._entrypoint.__name__
        debug_service.add_api_route(f"/{entrypoint_name}", entrypoint_api, methods=["POST"])

    uvicorn.run(debug_service, host="0.0.0.0", port=5050)

async def context_api(s: FastAPIRequest):
    instance = s.app.state.instance
    body_str = await s.body()

    await instance.on_session_enter(SessionContext(
        application=ApplicationContext(
            name="test",
            shim=Shim.GRPC,
            url=None,
            command=None,
        ),
        common_data=body_str,
    ))
    return FastAPIResponse(status_code=200, content="OK")

async def entrypoint_api(s: FastAPIRequest):
    instance = s.app.state.instance
    body_str = await s.body()
    
    output = await instance.on_task_invoke(TaskContext(
        input=body_str,
    ))

    return FastAPIResponse(status_code=200, content=output.data)

