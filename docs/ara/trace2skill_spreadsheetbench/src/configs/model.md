# Model and Serving Configuration

| Field | Value | Rationale | Search range | Sensitivity | Source |
|-------|-------|-----------|--------------|-------------|--------|
| Skill author/user model | Qwen3.5-122B-A10B | Paper model for trajectory generation, analysis, skill editing, and inference. | Not specified in paper. | Different model invalidates 1:1 parity claim. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Skill author/user model | Qwen3.5-35B-A3B | Paper model for smaller-model author/user conditions. | Not specified in paper. | Different model invalidates 1:1 parity claim. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Serving backend | vLLM | Paper states both models are served through vLLM. | Not specified in paper. | Serving/generation differences can change results. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Multi-turn mode | instruct mode | Paper uses instruct mode for multi-turn ReAct-style agentic tasks. | Not specified in paper. | Wrong mode changes agent behavior. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Single-call mode | thinking mode | Paper uses thinking mode for hierarchical merging, success analysis, and patch conversion. | Not specified in paper. | Wrong mode changes patch and merge behavior. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Generation config | `gen_config/qwen3.5_35B_122B_instruct_reasoning.json` | Upstream reproduction variable for agent runs. | Not specified in paper. | Required for faithful upstream run shape. | `tmp/repros/trace2skill-upstream/README.md` |
| Thinking generation config | `gen_config/qwen3.5_35B_122B_thinking_reasoning.json` | Upstream reproduction variable for analysis/evolution calls. | Not specified in paper. | Required for faithful upstream run shape. | `tmp/repros/trace2skill-upstream/README.md` |
