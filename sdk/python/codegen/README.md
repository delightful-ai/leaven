# Codegen — leaven-types and seam wire

## Public seam wire metadata

`generate_seam_wire.py` generates the checked-in private method metadata at
`src/leaven/_seam/_wire/methods.py`. It does not scrape Rust source or Markdown
tables. It invokes the Rust-owned export:

```bash
cargo run -q -p leaven-cli -- seam profile --root ../..
```

The export is assembled by `leaven-public-seam`, includes the locked method
table, schema bindings, capability actions, receipt flags, and Rust-computed
`fp_schema_sha256_*` fingerprints. Python uses this as drift-detectable wire
metadata for the `msgspec` codec layer; Rust remains the authority for method
semantics and capability policy.

Check drift from `sdk/python/`:

```bash
uv run python codegen/generate_seam_wire.py --check
```

## Leaven type staging

Pipeline that generates Python typed records from the locked
[public-seam-v1](../../../docs/specs/public-seam-v1/schemas/) JSON Schemas.
Generated output lands under [`src/leaven/_types/`](../src/leaven/_types/)
while iterating, but generated modules are not checked in today. The current
SDK does not import them; the eventual published `leaven-types` package will
own these records via the same pipeline with explicit public surface.

## Run

From `sdk/python/`:

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

When regenerated, the intended output is one module per locked-spec schema:

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

The current pipeline can produce large private modules, but checked-in Python
files stay below the repo line-count cap. Regenerate locally when working on
the future type projection, then either split/promote the output deliberately or
discard the generated scratch files before committing.

There are also gaps that need follow-up:

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
  are not affected — they're independent of the generated records. The
  generated records are private foundation material for the eventual `leaven-types`
   package, not a current dependency of `lv.*`.

   **Fix:** either (a) tune `datamodel-codegen` flags to keep primitive
   types as named root models, (b) post-process the output to
   re-introduce missing aliases, or (c) inline a `--collapse-root-models`
   off-mode for the specific schemas that define these primitives.
   Tracked for a follow-up iteration.

2. **Large schemas need split policy before check-in.** `plan.v1` and
   `plan_result.v1` can exceed the Python line-count cap if emitted as one
   file each. A follow-up must split them by owned concept before those records
   become committed source.

3. **`leaven-types` is currently `_types/`.** The public package will
   live separately; the SDK project stages it under `_types/` so the
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
