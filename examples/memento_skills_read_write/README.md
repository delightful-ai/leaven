# Memento-Skills Tiny Read-Write Loop

Status: active paper-specific example surface.

This directory is a small, live Codex harness for the Memento-Skills core loop.
It exists to preserve the paper's causal execution shape before extracting any
generic Leaven abstraction.

## Paper Anchors

- Read-Write loop: Observe -> Read -> Act -> Feedback -> Write
  (`tmp/skill_opt_sources/arx_2603.18743/full_source.md:319`).
- Initial state: skill library, tip memory, utility table, utility threshold,
  minimum samples, and feedback rounds
  (`tmp/skill_opt_sources/arx_2603.18743/full_source.md:323`).
- Write is skill-level failure attribution and file-level rewriting with a
  unit-test gate and rollback on failure
  (`tmp/skill_opt_sources/arx_2603.18743/all_text_sources.md:1182`).
- Full paper evaluation uses GAIA 100/65 train/test, HLE 788/342 train/test,
  Gemini-3.1-Flash, and up to three reflective retries
  (`tmp/skill_opt_sources/arx_2603.18743/full_source.md:518`,
  `:522`, `:526`, `:532`).

## Tiny Loop

The live command runs one valid train case:

1. Observe a user task and persistent library state.
2. Read by selecting one skill from descriptions.
3. Act with the selected skill mounted under `.agents/skills`.
4. Feedback through an exact judge.
5. Write by attributing failure to the selected skill and rewriting `SKILL.md`.
6. Validate the rewritten skill with a tiny gate.
7. Retry the task with the updated skill.

## Known Deviations

- Codex/GPT-5.4-mini replaces Gemini-3.1-Flash.
- The router is a Codex description-only selector over one skill, not the
  paper's trained Memento-Qwen behaviour-aligned retriever.
- The judge is exact-match over one synthetic training task, not GAIA/HLE.
- The unit-test gate is a tiny fixture assertion, not generated test suites.
- Full router training, catalog construction, GAIA/HLE splits, and benchmark
  reporting remain deferred.

## Commands

```bash
bash examples/memento_skills_read_write/scripts/run_tiny_live.sh --preflight
LEAVEN_CODEX_LIVE=1 bash examples/memento_skills_read_write/scripts/run_tiny_live.sh --live
```

Generated artifacts land under `tmp/memento_skills_read_write/`.
