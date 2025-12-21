# /// script
# dependencies = [
#   "flamepy",
# ]
# [tool.uv.sources]
# flamepy = { path = ".." }
# ///

"""
Example usage of the Flame Python SDK instance functionality.
"""

import flamepy


class SysPrompt(flamepy.Request):
    prompt: str


class Blog(flamepy.Request):
    url: str


class Summary(flamepy.Response):
    url: str
    summary: str


ins = flamepy.FlameInstance()

sys_prompt = """
You are a helpful assistant.
"""


@ins.context
def sys_context(sp: SysPrompt):
    global sys_prompt
    sys_prompt = sp.prompt


@ins.entrypoint
def summarize_blog(bl: Blog) -> Summary:
    global sys_prompt

    summary = f"Summary of {bl.url}: {sys_prompt}"

    return Summary(url=bl.url, summary=summary)


ins.run()
