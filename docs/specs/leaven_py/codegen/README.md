# Codegen — leaven-types

Pipeline that generates Python typed records from the locked
[public-seam-v1](../../public-seam-v1/schemas/) JSON Schemas. The output
lives at [`src/leaven/_types/`](../src/leaven/_types/) (private subpackage
prefixed with `_` per the scaffold's public/private discipline — the
eventual published `leaven-types` package will own these records via the
same pipeline with explicit public surface).

## Run

From `docs/specs/leaven_py/`:

```bash
just codegen
# or
uv run python codegen/generate_types.py
```

## Pipeline

1. **Preprocess** ([`preprocess_schemas.py`](preprocess_schemas.py)) —
   JSON Schema 2020-12 permits boolean schema values (`true` = any,
   `false` = nothing). `datamodel-code-generator` (0.34) does not handle
   these; this script normalizes them to `{}` / `{"not": {}}` only at
   actual schema positions (recursing into `properties.*`, `items`,
   `$defs.*`, `allOf/anyOf/oneOf`, etc., but **not** rewriting booleans
   at `uniqueItems`, `readOnly`, etc. where the keyword takes a boolean).

2. **Generate** ([`generate_types.py`](generate_types.py)) — runs
   `datamodel-codegen` against the entire preprocessed schema directory
   so cross-file `$ref` resolution works. Output goes to
   `src/leaven/_types/` as one module per source schema.

3. **Format** — `datamodel-codegen` runs `ruff format` + `ruff check`
   over the output for consistency.

## Generated modules

One per locked-spec schema, named after the source file with `.schema`
collapsed:

- `common_schema.py` — IDs, fingerprints, enums (shared across the others)
- `leaven_acp_profile_v1_schema.py`
- `leaven_capability_v1_schema.py`
- `leaven_evaluation_job_v1_schema.py`
- `leaven_evidence_envelope_v1_schema.py`
- `leaven_plan_v1_schema.py`
- `leaven_plan_result_v1_schema.py`
- `leaven_stage_payloads_v1_schema.py`

Skipped (per the governing spec):
- `leaven.watch.v1.schema.json` — watch.v1 runtime is deferred from V1
- `leaven.worker_protocol.v1.schema.json` — deprecated in favor of ACP

## Known gaps

The current pipeline produces 4,000+ lines of typed records, but there
are gaps that need follow-up:

1. **Primitive-typed `$ref` flattening.** Some constrained-string types
   like `ReceiptId`, `CandidateId`, etc. that are defined in
   `common.schema.json#/$defs/` as `{"type": "string", "pattern": "..."}`
   are inlined at use sites instead of becoming top-level
   `common_schema.ReceiptId` classes. Cross-file references to these
   then dangle (e.g. `common_schema.ReceiptId` is referenced from
   `leaven_plan_v1_schema.py:64` but not defined in
   `common_schema.py`).

   **Mitigation:** for now, the dangling references mean some generated
   modules don't import cleanly in isolation. The hand-written records
   in `src/leaven/` (which the scaffold uses for its public surface)
   are NOT affected — they're independent of the generated records. The
   generated records are scaffolding for the eventual `leaven-types`
   package, not a current dependency of `lv.*`.

   **Fix:** either (a) tune `datamodel-codegen` flags to keep primitive
   types as named root models, (b) post-process the output to
   re-introduce missing aliases, or (c) inline a `--collapse-root-models`
   off-mode for the specific schemas that define these primitives.
   Tracked for a follow-up iteration.

2. **Module names are verbose.** `leaven_plan_result_v1_schema.py` is
   more name than it needs to be. A follow-up rename pass (or generator
   tweak) could collapse to `plan_result.py` etc. The `SCHEMA_TO_MODULE`
   mapping in `generate_types.py` documents the intent but the
   directory-mode codegen ignores it (uses source filenames).

3. **`leaven-types` is currently `_types/`.** The public package will
   live separately; the scaffold stages it under `_types/` so the
   underscore convention keeps it private until the codegen output is
   clean enough to expose.

## Why this exists

The governing spec (`docs/specs/leaven_python.md` "The wire" section)
locks the wire as JSON Schema 2020-12 records. The Python types users
see (`lv.Score`, `lv.AssessmentWrite`, etc.) should be the SAME shapes
as the schemas — not parallel hand-written types that can drift. This
pipeline guarantees they share a source.

Per the multi-language audit
(`docs/working-memory/leaven-py-research/2026-05-24-multi-language-future-proofing.md`),
the same JSON Schemas should produce typed records for future TS / Go
SDKs via the same pattern. This Python pipeline is the prototype.
