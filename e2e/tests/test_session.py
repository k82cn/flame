
import pytest
import asyncio
import pytest_asyncio
import flamepy

class MyTaskInformer(flamepy.TaskInformer):
    """Example task informer that prints task updates."""
    
    def on_update(self, task):
        pass
    
    def on_error(self, error):
        pass

@pytest.mark.asyncio
async def test_create_session():
    session = await flamepy.create_session(
        application="flmping",
        common_data=b"shared data"
    )

    await session.invoke(b"task input data", MyTaskInformer())
    await session.close()


@pytest.mark.asyncio
async def test_invoke_multiple_tasks():
    session = await flamepy.create_session(
        application="flmping",
        common_data=b"shared data"
    )

    for i in range(10):
        await session.invoke(b"task input data", MyTaskInformer())
    await session.close()


@pytest.mark.asyncio
async def test_invoke_multiple_sessions():
    for i in range(10):
        session = await flamepy.create_session(
            application="flmping",
            common_data=b"shared data"
        )

        for i in range(10):
            await session.invoke(b"task input data", MyTaskInformer())
        await session.close()

