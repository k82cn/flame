# vLLM Example

This example uses `flamepy.runner.Runner` to run a small vLLM model across two sessions: one prefill engine and one decode engine.

Each `rr.service(...)` call creates a Flame session. Object instances are stateful, so each engine loads the model once on its executor. Prefill returns a `PrefillOutput` object (prompt token ids plus the first generated token). Decode continues from that object via `TokensPrompt`. This is a session-level prefill/decode split, not vLLM KV-connector disaggregation; decode recomputes KV on the same small model.

Requires a Flame cluster and a runtime that can start vLLM. Override the model with `VLLM_MODEL` (default `facebook/opt-125m`).

## Run

From the Flame console:

```bash
cd /opt/examples/vllm
uv run main.py
```

## Files

- `main.py`: Opens a Runner, creates prefill and decode services, prints the completion.
- `engine.py`: Stateful `PrefillEngine` and `DecodeEngine` used as Runner services.
- `pyproject.toml`: Package dependencies (`vllm`). The Flame environment provides `flamepy`.
