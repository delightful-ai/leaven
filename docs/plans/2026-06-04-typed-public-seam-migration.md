# Typed Public Seam Migration

Status: active design for `public_seam_v1_production_denominator`
Date: 2026-06-04

## Intent

Make the locked V1 `leaven/*` seam typed end to end for external-language SDK
workers. JSON remains the transport encoding, but method semantics must not
flow through unbranded `serde_json::Value`, Python `Any`, `dict[str, Any]`, or
msgspec `Raw` except at explicitly named and approved raw JSON islands.

This supersedes any route or SDK proof that only shows schema validation over
raw JSON. A runtime/service route is not production evidence until it carries
typed method requests and typed method results through the public seam.

## Current Problem

The public seam currently has useful semantic validators and some typed wrapper
documents, but the critical dispatch path is still erased:

- `leaven-seam-runtime` accepts a parsed JSON value and passes `&Value` params
  to `SeamService`.
- `SeamService` returns `Value`, so configured services can build results with
  `json!` and stringly `kind` checks.
- `leaven-seam-service` routes by method string and spelunks Plan IR maps for
  graph/case/workspace/provider/write details.
- Python private wire code has msgspec JSON-RPC envelopes, but generated
  payload records still use `Raw` for most nested Plan IR and Plan Result
  branches.
- Public Python and worker code still accept or return `Any`/raw dicts across
  seam-shaped builder and callback paths.

The provisional `leaven-seam-run` route crate can remain as topology
scaffolding, but it cannot count as product proof until the route consumes and
returns typed seam records.

## Non-Negotiable Rule

No unbranded raw JSON crosses or moves within Leaven's seam, runtime, service,
engine-facing, SDK, or worker protocol code.

The rule is not only an external-boundary rule. Inside Leaven, raw JSON is still
type erasure unless it is one of the named islands below or a local parse buffer
that is immediately consumed into a typed record and never exposed to another
module.

Allowed raw JSON must be represented by named types. Each type needs an owning
module, a doc comment explaining why it cannot be more specific, law/example
tests, and an allowlist entry in the relevant quality gate.

Initial allowed raw JSON islands:

- `JsonSchemaDocument`: user/provider JSON Schema payloads.
- `CaseInputJson`: externally supplied case input whose domain shape belongs to
  the user's task, not Leaven.
- `CaseTargetJson`: externally supplied case target.
- `CaseMetadataJson`: externally supplied case metadata.
- `ProviderParsedJson`: provider-returned structured output whose schema is
  selected by the user or model call.
- `OpaqueExtensionJson`: temporary escape hatch only for staged migration tests;
  every use must name a follow-up row and may not appear in final completion
  evidence.

Everything else gets a closed typed record or enum.

## Target Topology

`crates/leaven-public-seam` owns wire truth:

- locked method enum and per-method metadata;
- JSON-RPC request and response envelopes;
- typed Plan IR request AST;
- typed Plan Result, result primary, receipt, charge, and error AST;
- typed `leaven/stage.run` request/result records;
- typed capability subject, grant, resource, constraints, and limits;
- Rust export used by Python codegen.

`crates/leaven-seam-runtime` owns transport-neutral typed dispatch:

- decode parsed JSON into a typed JSON-RPC request through `leaven-public-seam`;
- deliver `SeamRequest` to the service;
- validate/encode typed `SeamResponse`;
- map parse/request/service/result failures to JSON-RPC errors.

It must not execute providers, mutate graphs, or inspect raw method payloads.

`crates/leaven-seam-service` owns configured service execution:

- match on typed method variants, not method strings;
- lower typed public-seam requests to configured LM/agent/workspace/sandbox
  providers or RunContext-bound graph effects;
- return typed public-seam results.

`crates/leaven-seam-run` owns run-bound composition only:

- bind a typed RunContext-backed service while a real run/stage is active;
- expose the typed runtime over stdio through `leaven-seam-stdio`;
- prove restored checkpoint/graph truth after typed method execution.

`sdk/python` owns generated Python wire bindings and ergonomic public wrappers:

- generated msgspec records from Rust-owned seam export;
- typed JSON-RPC codec with missing-vs-null handling and id-routed batches;
- public Pydantic/user-facing types only above the wire layer;
- no Python public or wire `Any`.

## Migration Phases

### 1. Closed Method Identity

Add a public-seam `LockedMethod` enum for all retained V1 methods. Derive the
locked method table, schema bindings, required actions, expected primary kinds,
and receipt expectations from this enum.

Acceptance:

- no locked method table is authored as loose string rows;
- profile tests prove every method round-trips through `LockedMethod`;
- unknown strings refuse before service dispatch.

### 2. Typed JSON-RPC Envelopes

Move request/response envelope parsing into public-seam typed records:

- `JsonRpcId`;
- `JsonRpcRequest<T>`;
- `JsonRpcSuccess<T>`;
- `JsonRpcFailure`;
- `JsonRpcResponse<T>`;
- typed notification handling where allowed or refused.

Acceptance:

- runtime receives typed envelopes, not `&Value`;
- tests kill both-result-and-error, neither-result-nor-error, missing id,
  null id where forbidden, notification misuse, extra top-level members, and
  method/profile mismatch.

### 3. Typed Plan IR

Replace schema-valid `PlanDocument` summaries with a typed Plan IR AST:

- consistency, mode, commit, return names;
- let/call/write operations;
- graph/case/workspace query expressions;
- LM/agent/sandbox/workspace call bodies;
- proposal, apply, assessment, evaluation-request, and event writes;
- explicit raw JSON islands only for case values, user JSON schemas, and
  provider parsed output.

Acceptance:

- public-seam execution hosts receive typed request structs;
- configured service and run-bound service stop parsing writes from raw maps;
- Plan IR tests include law/example coverage for each operation family.

### 4. Typed Results And Receipts

Replace Plan Result map inspection with typed result records:

- primary value enum by method/result kind;
- query/call/write receipt enums;
- charge records;
- typed error records;
- data-class projections;
- result hash preimage helpers over typed records.

Acceptance:

- `AcpExtensionResultDocument` carries a typed result variant;
- runtime validates typed service output before JSON serialization;
- result tests kill forged receipt kinds, missing charges, omitted cost,
  mismatched method/result kind, and hash-bound primary tampering.

### 5. Typed Capability Documents

Replace capability raw subject/resource/constraint maps with typed records:

- capability subject enum;
- grant action enum;
- resource selectors;
- constraints;
- per-grant and aggregate limits.

Acceptance:

- capability authorization takes typed grant requests;
- runner/reflector target-denial and evaluation-stage constraints are typed;
- budget projection stays tied to kernel cost/budget primitives.

### 6. Runtime And Service Cutover

Change `SeamService` from raw params/results to typed requests/results.

Acceptance:

- `leaven-seam-runtime` has no method semantic `Value` inspection;
- `leaven-seam-service` has no method string dispatch for locked methods;
- configured service tests still prove every retained method executes or is
  explicitly removed from locked V1.

### 7. Run-Bound Route Cutover

Convert provisional `leaven-seam-run` and run-bound service proofs to typed
requests/results.

Acceptance:

- typed proposal apply, evaluation request, assessment submit, and event emit
  mutate only through `RunContext`;
- restored checkpoint/graph readback proves candidate, evaluation request,
  assessment, and event truth;
- no JSON map spelunking remains in run-bound effect selection.

### 8. Rust Export And Python Codegen

Export typed seam metadata from Rust and regenerate Python msgspec modules.

Acceptance:

- generated Python modules are split under 650 LOC;
- msgspec records cover typed method payload/result branches, not only outer
  envelopes;
- codec decodes method-specific results after outer envelope parsing;
- batch responses route by id;
- no public/wire Python `Any`.

### 9. Python SDK And Worker Cutover

Replace handwritten Python seam dict builders and worker protocol dicts with
generated typed records plus ergonomic wrappers.

Acceptance:

- `_seam.client` and `_seam_worker.protocol` encode/decode typed records;
- public builders project typed Plan Results into user-facing Pydantic records;
- worker callbacks record typed receipts;
- Python quality gate bans unapproved `Any`, raw dicts, and msgspec `Raw` in
  public/wire boundaries.

### 10. Product Proof

Only after typed backbone rows pass:

- run `leaven seam serve --stdio` against the retained method matrix;
- prove Python inspection reads Rust-owned run/checkpoint/export state with blob
  byte retrieval;
- prove live Codex `gpt-5.4-mini` reflection/proposal/materialization/apply
  loop mutates a skill or system prompt and a later Codex stage consumes the
  applied child.

## Map/Reduce Execution Plan

Use subagents in two waves.

Wave 1 maps the typed denominator and returns reports only:

- Rust public-seam type inventory;
- runtime/service erasure inventory;
- Python codegen/wire erasure inventory;
- test/proof inventory.

Wave 2 executes disjoint patches after the reducer writes the phase checklist:

- Agent A: `LockedMethod` and method metadata in `leaven-public-seam`.
- Agent B: typed JSON-RPC envelope records in `leaven-public-seam` and runtime
  decode/encode migration.
- Agent C: typed Plan IR AST in `leaven-public-seam`.
- Agent D: typed Plan Result, primary values, receipts, charges, and errors.
- Agent E: typed capability documents and grant requests.
- Agent F: `leaven-seam-service` configured service migration.
- Agent G: `leaven-seam-service` run-bound service plus `leaven-seam-run`
  route migration.
- Agent H: Rust export plus Python msgspec generator.
- Agent I: Python SDK/client/worker cutover and quality gate enforcement.

Each execution agent must state its write set before editing. No two agents may
edit the same source module in the same wave. The reducer integrates in the
phase order above and runs focused tests after each merge.

## Test Placement

Rust source-level laws/examples live in the owning crate's mirrored test module
or nearest existing contract test:

- `leaven-public-seam`: `tests/public_seam_contract/**` for public wire laws.
- `leaven-seam-runtime`: `tests/runtime_contract.rs` for dispatch scenarios.
- `leaven-seam-stdio`: `tests/stdio_contract.rs` for framing scenarios.
- `leaven-seam-service`: crate-local tests for private lowerers; crate tests
  for public configured-service scenarios once private helpers are not needed.
- `leaven-seam-run`: integration scenarios over engine/run/service/runtime.

Python source-mirrored tests live outside `src` under the path-matching layout
already documented by `sdk/python/tests/AGENTS.md`. Process and product flows
belong under explicit integration locations.

## Focused Gates

Iteration gates:

```bash
cargo test -p leaven-public-seam --test public_seam_contract
cargo test -p leaven-seam-runtime --test runtime_contract
cargo test -p leaven-seam-stdio --test stdio_contract
cargo test -p leaven-seam-service
cargo test -p leaven-seam-run
cargo test -p leaven-cli --test seam_stdio_server
cargo test -p leaven --test topology_contract

cd sdk/python && uv run python codegen/generate_seam_wire.py --check
cd sdk/python && uv run pytest tests/_seam tests/_runs tests/builders
cd sdk/python && uv run ruff check
cd sdk/python && uv run ty check
cd sdk/python && uv run python scripts/check_quality_contract.py
```

Completion gates remain the goal denominator: method matrix, Python package and
example matrix, live Codex/Firkin proof where claimed, `just check`, and
coverage unless explicitly deferred in the live ledger.

Do not use `CARGO_INCREMENTAL=0` for ordinary iteration.

## Proxy Risks

These do not close typed-seam work:

- schema validation over raw JSON;
- a typed method enum while runtime/service still accept `&Value`;
- generated Python outer envelopes with nested `Raw` method bodies;
- Python dict builders that happen to match current schemas;
- route tests that assert graph truth from raw Plan IR maps;
- examples that exercise only prompt consumption or Python projections;
- `proposal.submit_batch` without typed apply and RunContext-owned mutation.
