# Memento-Skills Replication Dossier

Status: partial, blocked before faithful live replication.

## Scope

Paper/source bundle: `tmp/skill_opt_sources/arx_2603.18743`.

## Paper Anchors

- Read-Write loop is Observe -> Read -> Act -> Feedback -> Write:
  `tmp/skill_opt_sources/arx_2603.18743/paperclip_content.lines:107`.
- Write is skill-level reflective update with failure attribution and
  file-level rewriting, not append-only memory:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:118`.
- Behaviour-aligned router is trained via single-step offline RL because BM25
  and semantic embeddings are insufficient:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:279`,
  `:366`.
- Router data starts from about 8k local skills and about 3k sampled skills;
  public catalog is GitHub stars >500 with deterministic dedupe:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:370`,
  `:446`.
- Router evaluation uses Qwen3-Embedding-0.6B, 140 synthetic queries, and
  Recall@1 0.32/0.54/0.60 for BM25/Qwen3/Memento-Qwen:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:456`,
  `:460`.
- GAIA uses 165 validation questions split 100 train / 65 test; HLE uses
  788 train / 342 test across 8 subjects:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:518`,
  `:522`.
- All experiments use Gemini-3.1-Flash:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:526`.
- GAIA allows up to three reflective retries and reports 66.0 test vs 52.3
  ablation; HLE reports 38.7 test vs 17.9 baseline:
  `tmp/skill_opt_sources/arx_2603.18743/full_source.md:532`,
  `:550`.

## Leaven-Side Progress

- `leaven-eval::RankedRetrievalEvaluation` now owns the generic ranked
  retrieval metric contract needed for the router report: a declared candidate
  universe, query relevance sets, one ranking per query, duplicate/missing
  refusal, and Recall@K. It is intentionally opaque over item ids, so Memento
  examples can map skill ids into this substrate without making `leaven-eval`
  depend on skill artifacts or embedding providers.

## Current Blockers

Leaven-owned remaining primitives before faithful Memento-Skills replication:

- skill catalog construction and deterministic public-skill dedupe;
- behavior-aligned router training data generation and fitted router storage;
- BM25/Qwen/Memento router adapters that emit ranked retrieval outputs over the
  same candidate universe;
- skill registry with routing goals, utility table, trigger stats, and
  skill-level failure attribution;
- write path that targets one skill, rewrites files, validates, retries, and
  rolls back;
- multi-round retry execution with per-round feedback and learned-library
  state;
- GAIA/HLE harness adapters and exact split manifests.

External/spend blockers:

- HLE and GAIA access, Gemini-3.1-Flash availability, and router training
  compute have not been approved.

## Verification

- `cargo nextest run -p leaven-eval --test retrieval_contract` passed on
  2026-05-20 for Recall@K, candidate-universe validation, missing rankings,
  empty relevant sets, and duplicate ranked-item refusal.
