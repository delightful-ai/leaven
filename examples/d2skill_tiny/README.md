## D2Skill Tiny Loop

Status: active paper-specific example surface.

This directory is a small live Codex harness for the D2Skill core loop. It
exists to preserve the paper's causal execution shape before extracting any
shared Leaven primitive.

Paper anchors:

- D2Skill maintains task skills and step skills in a persistent skill bank:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:29`.
- Training samples paired baseline and skill-injected trajectory groups:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:104`.
- Hindsight utility is computed from the skill/baseline performance gap:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:119`.
- Reflection triggers on low skill-group performance and generates at most one
  task skill and one step skill:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:183`.
- Two-stage retrieval combines similarity and utility/UCB ranking:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:196`.
- Bank management prunes low-utility skills when a pool exceeds capacity:
  `tmp/skill_opt_sources/arx_2603.28716/full_source.md:212`.

The live command runs one train task through:

1. a baseline rollout without skill injection;
2. a skill-injected rollout using the current retrieved task and step skills;
3. deterministic trajectory evaluation;
4. reflection from the failed skill trajectory into one task skill and one step
   skill;
5. next-iteration retrieval and skill-injected rollout consuming the updated
   bank;
6. utility update from the skill/baseline performance gap;
7. capacity-based pruning of stale skills.

Known deviations:

- Codex/GPT-5.4-mini replaces Qwen policy models and Gemini/O3 reflector
  models.
- The environment is a tiny textual decision task rather than ALFWorld or
  WebShop.
- The RL policy update is represented by logged hindsight return/advantage
  terms; no model parameters are trained.
- Similarity is a paper-faithful executable substitute over retrieval keys
  instead of learned embedding cosine similarity.
- Full GRPO, large grouped rollouts, validation curves, and benchmark tables
  remain deferred.

Commands:

```bash
bash examples/d2skill_tiny/scripts/run_tiny_live.sh --preflight
LEAVEN_CODEX_LIVE=1 bash examples/d2skill_tiny/scripts/run_tiny_live.sh --live
```

