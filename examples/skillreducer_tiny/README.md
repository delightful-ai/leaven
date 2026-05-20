## SkillReducer Tiny Loop

Status: active paper-specific example surface.

This directory is a tiny live Codex harness for the SkillReducer core loop. It
exists to preserve the paper's causal execution shape before extracting any
shared Leaven primitive.

Paper anchors:

- Skills have routing descriptions, injected bodies, references, and scripts:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:31`.
- Stage 1 compresses descriptions with a simulated oracle and real validation:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:108`.
- Stage 2 classifies body content, creates progressive-disclosure references,
  and verifies faithfulness/task quality:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:136`.
- Gate 2 compares no-skill, original Condition A, and compressed Condition C:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:170`.
- The feedback loop promotes missed non-core items into the core:
  `tmp/skill_opt_sources/arx_2603.29919/full_source.md:194`.

The live command runs one small product-marketing skill through:

1. description candidate generation and simulated routing oracle;
2. real-trigger validation over the compressed description;
3. body taxonomy classification and compressed/tiered skill materialization;
4. faithfulness verification;
5. Condition A and Condition C task execution;
6. deterministic scoring and optional feedback promotion if C regresses.

Known deviations:

- Codex/GPT-5.4-mini replaces DeepSeek-V3, DeepSeek-R1, Qwen3.5, and Claude
  Code roles from the paper.
- The simulated oracle evaluates a tiny candidate set instead of a full 600
  skill corpus and exhaustive ddmin search.
- Real-trigger validation is a Codex runtime prompt over deployed skill
  descriptions, not parsed Claude Code stream events.
- Gate 2 uses one deterministic rubric task rather than five generated tasks
  per skill across D/A/C.
- Full SkillsBench, wild-skill sampling, token accounting with `tiktoken`, and
  statistical reporting remain deferred.

Commands:

```bash
bash examples/skillreducer_tiny/scripts/run_tiny_live.sh --preflight
LEAVEN_CODEX_LIVE=1 bash examples/skillreducer_tiny/scripts/run_tiny_live.sh --live
```

