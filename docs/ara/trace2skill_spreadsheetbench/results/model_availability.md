# Model Availability Research

This note supports the approval packet in `full_run_plan.md`. It records
availability facts only; it does not approve model use, hardware spend, or a
full paper-denominator run.

## Current Finding

The two paper model names have public model repositories and OpenAI-compatible
serving paths. The Leaven run is still blocked because no local endpoint,
hardware, credentials, budget, or artifact-retention approval has been filled.

| Model | Availability finding | Serving finding | Approval status |
|-------|----------------------|-----------------|-----------------|
| `Qwen3.5-122B-A10B` | Public Hugging Face repository `Qwen/Qwen3.5-122B-A10B` exists and provides model weights/configs. | Hugging Face documents `vllm serve Qwen/Qwen3.5-122B-A10B --tensor-parallel-size 8`; Lambda documents 4x B200, 8x H100, or 8x A100 as load targets for the 122B model. | Not approved or provisioned. |
| `Qwen3.5-35B-A3B` | Public Hugging Face repository `Qwen/Qwen3.5-35B-A3B` exists and provides model weights/configs. | Hugging Face documents `vllm serve Qwen/Qwen3.5-35B-A3B --tensor-parallel-size 8`; Hugging Face also notes an official Qwen API service path for managed inference. | Not approved or provisioned. |

## Local Upstream Reproduction Hooks

| Hook | Local source | Relevance |
|------|--------------|-----------|
| OpenAI-compatible API | `tmp/repros/trace2skill-upstream/README.md` | Upstream runners use `OPENAI_API_KEY` and `OPENAI_BASE_URL`, with local serving supported by `--api-key EMPTY --base-url http://localhost:8000/v1` for some entrypoints. |
| Instruct generation config | `tmp/repros/trace2skill-upstream/gen_config/qwen3.5_35B_122B_instruct_reasoning.json` | Disables thinking with `chat_template_kwargs.enable_thinking=false`, timeout `600`, `temperature=1.0`, `top_p=1.0`, and `presence_penalty=2.0`. |
| Thinking generation config | `tmp/repros/trace2skill-upstream/gen_config/qwen3.5_35B_122B_thinking_reasoning.json` | Enables thinking with timeout `1800`, `temperature=1.0`, `top_p=0.95`, and `presence_penalty=1.5`. |
| Spreadsheet runner | `tmp/repros/trace2skill-upstream/run_spreadsheetbench.py` | Exposes `--model`, `--generation_config`, `--seeds`, `--start_idx`, `--end_idx`, `--workers`, and `--max_turns`. |
| Skill evolution runner | `tmp/repros/trace2skill-upstream/skill_evolver/run_parallel_skill_evolution.py` | Exposes `--model`, `--base-url`, `--api-key`, `--generation-config`, `--seed`, `--merge-batch-size`, and `--max-workers`. |

## External Sources Checked

| Source | URL | Notes |
|--------|-----|-------|
| Hugging Face `Qwen/Qwen3.5-122B-A10B` | `https://huggingface.co/Qwen/Qwen3.5-122B-A10B` | Model repo and vLLM/SGLang/Transformers serving instructions. |
| Hugging Face `Qwen/Qwen3.5-35B-A3B` | `https://huggingface.co/Qwen/Qwen3.5-35B-A3B` | Model repo, vLLM serving instructions, and managed Qwen API note. |
| Lambda model card for `Qwen3.5-122B-A10B` | `https://lambda.ai/inference-models/qwen/qwen3.5-122b-a10b` | Hardware sizing and example vLLM/SGLang launch commands for 122B. |

## Remaining Approval Fields

These fields remain unresolved in `full_run_plan.md`:

- exact model source or endpoint to use for each model;
- vLLM version or managed API provider;
- host/GPU allocation;
- API credentials and redaction policy;
- cost and runtime limit;
- artifact retention and promotion policy;
- approval of `src/configs/tolerance.md`.

The machine-checkable packet is in `full_run_plan.md` and is intentionally
expected to fail `scripts/check_trace2skill_approval_packet.py` until those
fields are filled.

No Qwen/vLLM execution should start until those fields are filled and approved.
