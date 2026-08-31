import os
from dataclasses import dataclass, field

from flamepy.runner import SessionContext
from vllm import LLM, SamplingParams
from vllm.inputs import TokensPrompt

DEFAULT_MODEL = "facebook/opt-125m"


def _model_name() -> str:
    return os.getenv("VLLM_MODEL", DEFAULT_MODEL)


@dataclass
class PrefillOutput:
    prompt_token_ids: list[int]
    first_token_ids: list[int] = field(default_factory=list)


class _VllmEngine:
    def __init__(self):
        self._llm = None

    def _engine(self) -> LLM:
        if self._llm is None:
            self._llm = LLM(model=_model_name())
        return self._llm


class PrefillEngine(_VllmEngine):
    _session_context = SessionContext(session_id="vllm-prefill")

    def prefill(self, prompt: str) -> PrefillOutput:
        outputs = self._engine().generate([prompt], SamplingParams(max_tokens=1))
        output = outputs[0]
        return PrefillOutput(
            prompt_token_ids=list(output.prompt_token_ids),
            first_token_ids=list(output.outputs[0].token_ids),
        )


class DecodeEngine(_VllmEngine):
    _session_context = SessionContext(session_id="vllm-decode")

    def decode(self, prefill: PrefillOutput, max_tokens: int = 16) -> str:
        token_ids = list(prefill.prompt_token_ids) + list(prefill.first_token_ids)
        outputs = self._engine().generate(
            [TokensPrompt(prompt_token_ids=token_ids)],
            SamplingParams(max_tokens=max_tokens),
        )
        return outputs[0].outputs[0].text
