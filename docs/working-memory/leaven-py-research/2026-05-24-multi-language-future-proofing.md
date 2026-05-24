# Multi-Language Future-Proofing for `leaven-acp` and the Python SDK

Status: research note, pre-spec.
Updated: 2026-05-24.

## Authority

This note is subordinate to:

- `docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md` (locked
  Leaven ACP profile).
- `docs/specs/public-seam-v1/schemas/*.schema.json` (locked wire schemas;
  JSON Schema 2020-12).
- `docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md`
  (rejection of pyo3 at line 29; multi-language commitment at lines 15 and 23;
  extension namespace concern at line 743).
- `crates/AGENTS.md:20-24, 48-53` (the `leaven-acp` topology entry).
- Parent ledger `docs/working-memory/leaven-py-and-acp-transport.md`.

It is research, not proof of any implementation. The question being answered:
does the proposed architecture (Leaven owns the wire via `leaven-acp`, Python
SDK is the first consumer) accidentally embed Python assumptions in the wire,
or does it make TS/Go/shell workers cheap to add later?

## Headline

The wire is multi-language-safe today. The proposed architecture is
multi-language-safe *if* the Python SDK is forbidden from inventing semantic
content that hasn't been promoted into the schemas, and *if* the first TS
SDK ships within the same major release window so that the locked-v1 wire is
proved on at least one non-Python consumer before users build against it.

The biggest risks are not in the schema bytes; they are in (a) decorator
semantics, (b) async-iterator semantics, (c) error-shape semantics, and
(d) the `x.*` extension namespace becoming a Python-shaped junk drawer. Each
risk has a concrete mitigation below.

## 1. The wire's language assumptions

### 1.1 Naming convention — snake_case throughout

The wire pins `snake_case` for every field. Examples:
`stage_call_id`, `read_receipts`, `data_classes`, `target_safety`,
`capability_fingerprint`, `schema_fingerprint`, `policy_fingerprint`,
`evaluation_request_id`
(`docs/specs/public-seam-v1/schemas/leaven.stage_payloads.v1.schema.json:256-267`
and throughout).

JSON Schema `$defs` keys are PascalCase (`RunId`, `StageCallId`,
`OutputRecord`) but those are type names, not wire fields, so they are not
serialized
(`docs/specs/public-seam-v1/schemas/common.schema.json:5-86`).

**Comparison.** LSP and DAP both standardized on **camelCase**
(`textDocument`, `positionEncoding`, `threadId`). MCP also uses camelCase
(`protocolVersion`, `clientInfo`, `inputSchema`,
[modelcontextprotocol.io/docs/concepts/architecture]).
Leaven is the odd one out at this layer.

**Why this isn't a Python lock-in.** snake_case maps trivially to:
- Python: native field names, no transformation.
- Go: `json:"stage_call_id"` struct tags; mechanical for codegen.
- TS: `obj["stage_call_id"]` works; idiomatic TS code generators
  (`json-schema-to-typescript`, `quicktype`) preserve the source key by
  default, and downstream code uses `stageCallId` accessors only if the user
  explicitly wires a transformer. Either is fine; the wire bytes don't care.
- Rust: `#[serde(rename_all = "snake_case")]` is already idiomatic; this is
  what `leaven-public-seam` does today.
- Shell: keys are quoted strings to `jq` either way.

snake_case is therefore *cosmetic friction* for TS users (idiomatic TS field
access is `stageCallId`) but *zero semantic risk*. The mitigation, if any:
publish a one-paragraph naming-rationale section in the schema README so TS
SDK authors don't accidentally rename half the fields client-side and
desynchronize from the wire.

### 1.2 JSON Schema features used

`docs/specs/public-seam-v1/schemas/leaven.acp_profile.v1.schema.json` and
siblings use:

- `oneOf` (kind-discriminated unions everywhere — see `CandidateRef`,
  `InfoRef`, `ProposalRef` in `common.schema.json:313-567`, and every
  stage-payload `oneOf` in `leaven.stage_payloads.v1.schema.json:6-31`).
- `enum` with string members (closed enums like `DataClass`,
  `ErrorCode`, `Replayability`).
- `const` for tagged-union discriminators (`"kind": { "const": "candidate"
  }` pattern, ubiquitous).
- `$ref` to relative `common.schema.json#/$defs/...`.
- `additionalProperties: false` on closed objects.
- `pattern` for string IDs and fingerprints.
- `minItems` / `uniqueItems` on arrays.
- `format: date-time` on `Timestamp`.
- One `JsonValue: true` open schema (`common.schema.json:86`).

This is the **boring subset**: discriminated `oneOf` + `enum` + `$ref` +
closed objects. It is what `quicktype`, `json-schema-to-typescript`, Google's
new `jsonschema-go`, and `omissis/go-jsonschema` all handle correctly.

**Known multi-language codegen pain that Leaven *avoids*.** No `if/then/else`,
no `dependentSchemas`, no `unevaluatedProperties`, no `propertyNames`, no
`$dynamicRef`, no schema composition through `allOf` mixin. These are the
2020-12 features that trip codegen tools and force them to emit either
sum-of-products explosions or runtime-only validation. The Leaven schemas
chose the right subset.

**One sharp edge.** `oneOf` discriminated by a `const` field is the OpenAPI
"discriminator" pattern; Google/MS codegen will produce sum types or tagged
classes from it, but `quicktype` for Go has historically generated weak
`map[string]interface{}` shapes for unions unless every variant has the same
discriminator key in the same position. Every Leaven `oneOf` does have a
`kind: const "..."` field in the same position
(`common.schema.json:319-336`), so this works. The codegen mitigation: pick
**one** codegen tool per language and write a small wrapper that asserts
discriminator presence at decode time; do not try to support five codegen
tools in parallel.

### 1.3 Number precision

`UsdMicro` is `integer, minimum: 0` with the explicit comment
"Integer micro-dollars. No floating-point money."
(`common.schema.json:87-91`). `wall_ms`, `input_tokens`, `output_tokens`,
`lm_calls`, etc. are all `integer, minimum: 0`
(`common.schema.json:659-695`).

**This is the most important multi-language win in the schemas.** JSON numbers
are float64 in JS; an unbounded integer dollar amount would silently lose
precision past 2^53 (~9 PB tokens or ~$9M in micro-dollars). By pinning the
unit to micro-dollars and the type to `integer`, every consumer (Python `int`,
Go `int64`, TS `number` or `bigint`, Rust `u64`) can roundtrip the same value
without rounding.

**Open risk.** TS `JSON.parse` returns a `number` for any integer literal,
which silently truncates `Number.MAX_SAFE_INTEGER` overflows. A TS SDK must
either (a) parse with a JSON library that yields `bigint` for integers
exceeding the safe range, or (b) document the safe range and have the engine
refuse to emit larger values. The Leaven mitigation: add a per-field
documented max (e.g. `maximum: 9007199254740991`) or push large counters to
strings. **Recommend**: add an explicit `maximum: 9007199254740991` to the
small set of integer fields that could plausibly exceed `2^53 - 1` (token
counts on long horizons; cumulative `usd_micro` on a multi-year project).
This is a one-line ratchet that prevents a class of TS-only bug.

### 1.4 Date/time encoding

`Timestamp` is `"format": "date-time"` (RFC 3339,
`common.schema.json:82-85`). Universally interpretable. Python `datetime`,
Go `time.Time`, JS `Date`, Rust `chrono::DateTime` all parse this directly.
No Unix-epoch-seconds-versus-millis ambiguity. Good.

### 1.5 Optional-vs-required field semantics

The schemas list `required: [...]` arrays explicitly per object, and rely on
field absence for optionality. They never use `null` for missing — the
schemas almost never declare `nullable: true` (none in the files surveyed).

**Multi-language fit.**
- Python (`pydantic`): `Optional[T] = None` maps cleanly to "field omitted."
- Go: zero value (`""`, `0`, `nil` slice/map) for absent; struct tag
  `omitempty` is idiomatic. Mechanical.
- TS: optional via `field?: T`. The
  "missing-vs-`undefined`-vs-`null`" three-state problem becomes a two-state
  problem (missing or present), which is the easy case. Codegen tools handle
  it without ambiguity.
- Rust: `Option<T>` + `#[serde(skip_serializing_if = "Option::is_none")]`.

**Risk.** If the Python SDK ever serializes `{"field": None}` instead of
omitting, the wire receivers in other languages will need to accept both,
which forks the contract. Mitigation: the Python SDK's serializer must
canonicalize to "omit when None" before sending. This is `model_dump(exclude_none=True)` in pydantic terms — trivial, but worth a contract test in
`leaven-public-seam` that asserts `null` never appears in a serialized
request.

### 1.6 Open-shape fields

A few fields are `{}` (JSON Schema "any" — see
`stage_payloads.v1.schema.json:39-40` `input` / `output` /
`side_info` on `ReflectiveExample`, also `value` on `OutputRecord`
`common.schema.json:717`, and `payload` on adapter request). These are
deliberate untyped holes for adapter content. They are governed by separate
`schema_fingerprint` fields and the adapter's own schema.

This is fine for multi-language adoption because every language already has
a JSON-value type (`json.RawMessage`, `serde_json::Value`,
`Record<string, unknown>`, `dict[str, Any]`). It does mean that **all SDKs
need a uniform escape hatch** to pass raw JSON through. The Python decorator
`@lv.evaluator` already passes pydantic models for typed fields and `dict`
for untyped — that pattern translates cleanly.

## 2. TypeScript SDK story

### 2.1 Typed records

Pick `quicktype` or `json-schema-to-typescript` and run it in CI against
`docs/specs/public-seam-v1/schemas/`. The output is a `leaven-types` npm
package (mirror of the Python `leaven-types` package). One-day job.

Recommendation: **`json-schema-to-typescript`**. It tracks JSON Schema
2020-12, preserves snake_case keys verbatim, and produces clean discriminated
unions for `oneOf` with `const` kind. `quicktype` is more flexible but
heavier and has weaker discriminated-union output for some 2020-12 patterns.
(Both are surveyed in
[quicktype.io/typescript](https://quicktype.io/typescript)
and [json-schema-to-typescript on npm](https://www.npmjs.com/package/json-schema-to-typescript).)

### 2.2 Transport

Same as Python: spawn `leaven serve --stdio` as a child process, exchange
ACP JSON-RPC over stdin/stdout. Node has first-class `child_process.spawn`
with stdio streams. ACP's reference TS SDK already does this (see
`@agentclientprotocol/sdk` in the ACP repo) — copy the framing module
verbatim, then wire the Leaven-method extension layer on top.

### 2.3 Equivalent of `@lv.evaluator`

TypeScript has experimental stage-3 decorators, but they are *not* the
idiomatic TS surface. The idiomatic TS surface is:

```ts
import { leaven } from "@leaven/sdk";

leaven.evaluator({
  id: "skillbank/pytest-dspy-codex",
  trust_profile: "managed_sandbox",
  granularity: "per_case",
}, async function evaluate(job, cx) {
  for await (const item of job.independentCases()) {
    const ws = await cx.workspace.materializeCandidate(item.candidate_id, {
      surface: "full_repo",
      lifetime: "stage_call",
    });
    // ...
  }
});
```

Plain function registered with a builder, async/await throughout,
`AsyncIterable` for batched cases. Decorators are a footgun in TS because the
TC39 stage and TypeScript `experimentalDecorators` flag are still drifting;
plain function registration avoids that entirely.

This shape is **strictly more general** than Python decorators. Python's
`@lv.evaluator(...)` could in fact be implemented internally as
`lv.evaluator(args)(func)` — the decorator is sugar. Keep the underlying
registration call symmetric across languages so the wire registration
payload is identical.

### 2.4 Distribution

The npm `optionalDependencies` per-platform pattern (as used by `esbuild`,
`swc`, `Rollup`, `Vite` — see
[esbuild PR #1621](https://github.com/evanw/esbuild/pull/1621) and
[evanw/esbuild platform-specific binaries](https://deepwiki.com/evanw/esbuild/6.2-platform-specific-binaries))
ships a stub npm package (`@leaven/sdk`) that declares one
`optionalDependency` per `os`/`cpu` combination
(`@leaven/sdk-linux-x64`, `@leaven/sdk-darwin-arm64`,
`@leaven/sdk-win32-x64`, etc.), each containing only the platform's
`leaven` binary. npm installs only the matching one. Symmetric to the
Python wheel-per-platform model. Confirmed viable; this is a well-trodden
path.

## 3. Go SDK story

### 3.1 Typed records

Two viable codegen paths:
- **`omissis/go-jsonschema`**
  ([github.com/omissis/go-jsonschema](https://github.com/omissis/go-jsonschema)):
  produces idiomatic Go structs with validation methods. Handles 2020-12.
- **`google/jsonschema-go`** (released 2026, used inside Google's official
  MCP Go SDK
  [opensource.googleblog.com/2026/01/a-json-schema-package-for-go.html](https://opensource.googleblog.com/2026/01/a-json-schema-package-for-go.html)):
  newer, comprehensive, actively maintained.

Recommendation: **`google/jsonschema-go`**, because (a) it's actively
maintained by the same org shipping the Go MCP SDK, (b) it tracks 2020-12
fully, (c) it produces sum-type-like discriminated unions for `oneOf` with
const kinds. `omissis/go-jsonschema` is a viable fallback.

### 3.2 Decorator equivalent

Go has no decorators. The idiomatic shape is:

```go
import "github.com/leaven/leaven-go/lv"

func main() {
    eng := lv.NewEngine()
    eng.RegisterEvaluator(lv.EvaluatorSpec{
        ID:           "skillbank/pytest-dspy-codex",
        TrustProfile: "managed_sandbox",
        Granularity:  lv.PerCase,
    }, func(ctx context.Context, job *lv.EvaluationJob, cx *lv.EvalContext) error {
        for item := range job.IndependentCases() {
            ws, err := cx.Workspace.MaterializeCandidate(ctx, item.CandidateID, lv.MaterializeOpts{
                Surface:  "full_repo",
                Lifetime: "stage_call",
            })
            // ...
        }
        return eng.Serve(ctx)
    })
}
```

`context.Context` plumbing replaces async/await; channels replace
async iterators; the `lv.EvaluatorSpec{}` value plays the role of the
Python decorator arguments. Same wire registration.

### 3.3 Distribution

`go install github.com/leaven/leaven-cmd/leaven@latest` builds from source —
this is the idiomatic Go path and assumes a Rust toolchain on the user's
machine, which is wrong for Leaven.

Two options:
- **Vendored binary in module via `go:embed`**: ship the `leaven` binary
  inside the Go module as an embedded asset, extract on first run. This is
  hacky and bloats the module.
- **`leaven-go` is just the SDK; the user installs the binary separately**
  (`brew install leaven`, `pip install leaven`, `npm i -g @leaven/cli`, or
  GitHub Release tarball). The SDK looks up `leaven` on `$PATH` or
  `$LEAVEN_BINARY`. This is the path taken by, e.g., `terraform-go`
  consumers; the user installs Terraform separately.

**Recommendation: option 2.** Go users are accustomed to having auxiliary
binaries installed out-of-band. Document the binary install path; don't try
to be clever.

## 4. Shell-worker story

The archived design notes (`COMPREHENSIVE_DESIGN_PASS_NOTES.md:23` and `:29`)
explicitly call out "shell" as a future consumer alongside Python and TS.
What can shell honestly do?

### 4.1 What shell can do trivially

Read-only CLI subcommands: `leaven query lineage`, `leaven runs list`,
`leaven artifact show`, `leaven case load`. These are JSON-out, no JSON-RPC
session — the user can `leaven case load case_xyz | jq '.target'` from a
script. The CLI subcommands listed in the parent ledger
(`leaven-py-and-acp-transport.md:42-44`) cover this directly.

### 4.2 What shell cannot do honestly

A *stateful ACP worker session* in bash is technically possible — `coproc`
or `mkfifo` to set up bidirectional pipes, `jq` to construct JSON-RPC
frames, `dd` to read length-prefixed messages — but it is hostile. The
session-lifecycle obligations (capability_fingerprint binding, bounded
update queues, ACP cancellation, schema-fingerprint pinning per call)
require more bookkeeping than bash should be asked to do.

A shell-friendly *one-shot stage* could work: `leaven stage exec --role
scorer --candidate cand_x --case case_y < /tmp/score.json` where stdin is
the score payload and stdout is the assessment write. This is a *CLI
subcommand* that internally spins up an ACP session, calls one extension
method, tears the session down, and emits the result. The user never sees
JSON-RPC.

### 4.3 Recommendation

- Honest path: ship CLI subcommands for the read side and for one-shot
  stage execution. Do not promise "shell worker" as a long-lived stage
  process.
- Smallest convenience layer: a tiny `leaven exec` subcommand that takes
  `--role`, `--input <file|->`, and emits stdout. This buys us 80% of the
  shell story without forcing shell users to speak JSON-RPC.
- If a power user really wants to write a long-lived bash worker, they
  can; the wire is open. We just don't market it.

## 5. The LSP / MCP / DAP lesson

### 5.1 What LSP got right

- **A machine-readable metamodel** drives every language binding. The
  `metaModel.json` in
  [microsoft/vscode-languageserver-node](https://github.com/microsoft/vscode-languageserver-node/blob/main/protocol/metaModel.json)
  is parsed by per-language plugins
  ([microsoft/lsprotocol](https://github.com/microsoft/lsprotocol)) to
  generate Python, Rust, .NET, and community Go/Crystal bindings from one
  source. Each language has its own package, all generated from the same
  metamodel.
- **camelCase wire**, but client code in each language uses idiomatic field
  names because the generated bindings rename. snake_case-on-the-wire is the
  symmetric inverse choice.

### 5.2 What LSP got wrong

- **The TypeScript reference implementation predated the spec.** For
  ~2 years, the de-facto spec was "whatever vscode-languageserver-node
  did." Non-TS implementers had to reverse-engineer behavior. The metamodel
  came later and is still incomplete in places.
- **Lesson for Leaven.** The wire is already locked in JSON Schema, not
  defined-by-Python. This is the right shape. The risk is that the first SDK
  to ship (Python) becomes the de facto behavior reference for ambiguities
  the schema doesn't fully pin. Mitigation: write the contract tests in
  `leaven-public-seam` such that they cover the schema-ambiguity gaps
  *before* the Python SDK lands. The `conformance-matrix.yaml` is the right
  shape for this.

### 5.3 What MCP got right

- **Simultaneous TS + Python launch.** When MCP shipped in late 2024, both
  the TS and Python SDKs were official Day-1, by Anthropic. This avoided
  the LSP single-language-bias trap. Java, Kotlin, C#, Go, Rust followed
  ([modelcontextprotocol.io/docs/sdk](https://modelcontextprotocol.io/docs/sdk)).
- **JSON Schema available as a build artifact** even though the spec is
  authored as TypeScript types. From the MCP spec repo description: "defined
  in TypeScript first, but made available as JSON Schema as well, for wider
  compatibility."

### 5.4 What MCP got wrong (and Leaven should avoid)

- **The TS SDK leaked into the spec for a while.** Specifically, the
  `FastMCP` decorator-driven Python pattern was retrofitted later, and the
  `inputSchema` requirements were initially ambiguous because the TS
  pydantic-equivalent (`zod`) is more permissive about default-value
  encoding than pydantic. Multi-SDK testing caught this; single-SDK testing
  would not have.
- **Sampling/elicitation extensions came late.** Server-to-client calls were
  a TS-first add-on; Python adoption lagged. The lesson: if a Leaven
  callback channel (engine -> worker) gets added post-v1, it will hit the
  same lag.

### 5.5 What DAP got right

- **One JSON wire spec, many adapters.** DAP shipped with adapters in
  Python, C++, Java, TypeScript, Go simultaneously because the wire is
  trivially generatable.
- **camelCase**. Boring.

### 5.6 The specific lessons for Leaven

1. **Ship the JSON Schemas as the canonical artifact, not Rust types or
   Python types.** The schemas are already the canonical artifact; keep
   them that way and codegen everywhere.
2. **Do not let the Python SDK be the only reference implementation when
   v1.0 of the wire goes public.** Ship a minimal TS SDK in the same
   release window (even if it's read-only at first) so the wire has been
   exercised by a second language *before* external users start writing
   their own bindings.
3. **Treat the conformance matrix as a multi-language test suite.** Each
   row in `conformance-matrix.yaml` should be runnable against any
   compliant worker, in any language. The Rust `leaven-public-seam`
   already does this for the wire side; mirror it for at least one
   non-Rust client.

## 6. Specific risks of "Python first"

If `leaven-py` v1.0 ships and `leaven-ts` arrives 6 months later, the
following could quietly bake in:

### 6.1 Optional-field encoding

- Risk: Python serializer emits `{"field": None}` when omitting would be
  correct. JS reads `null !== undefined` and TS code special-cases the
  difference; Go reads zero value; behavior diverges per language.
- Mitigation: contract test in `leaven-public-seam` asserts `null` never
  appears in a serialized request; Python SDK uses `exclude_none=True`
  pydantic dumps.

### 6.2 Streaming idioms

- Risk: Python evaluator surface uses `async for item in
  job.independent_cases():` and the wire happens to expose ACP `session/update`
  notifications with a Python-shaped pagination cursor. TS prefers
  `AsyncIterable` with `Symbol.asyncIterator`; Go prefers `<-chan Item`.
  All three work on top of ACP notifications **if** the notification shape
  is stateless and resumable.
- Mitigation: design the cursor as an opaque string the worker pages
  through; never use a Python-iterator-shaped continuation (e.g. a generator
  function name or a frame ID). The current `Cursor` schema
  (`common.schema.json:58-61`) is `^cur_[A-Za-z0-9_.:-]+$` — opaque string,
  language-neutral. Good.

### 6.3 Error types

- Risk: Python raises exceptions; TS prefers `Result`-like discriminated
  unions; Go returns `(value, error)`. The wire's `PlanError`
  (`common.schema.json:810-838`) is already a typed value, not an exception.
  Good. Risk is *only* in the SDK surface layer.
- Mitigation: Python `lv.PlanError` is **not** a Python `Exception`
  subclass on the wire boundary; it's a typed value. The SDK may *raise*
  in user code (idiomatic Python) but it must serialize to/deserialize
  from the wire's typed `PlanError` shape. TS SDK does the dual — produces
  a `Result<T, PlanError>`; never throws across the wire boundary.

### 6.4 Sync vs async

- Risk: Python `asyncio` is the obvious choice given that ACP is async by
  nature. TS is async-everywhere, no conflict. Go is sync with
  `context.Context` cancellation. The wire is async (notifications,
  cancellation), so the SDK surfaces should all expose async-shaped APIs;
  Go's "sync-but-cancellable" is the closest analogue.
- Mitigation: the wire is already async. Don't expose a sync-only Python
  surface; doing so would force the engine to fake notifications via
  polling.

### 6.5 The `x.*` extension namespace

- Risk: per `COMPREHENSIVE_DESIGN_PASS_NOTES.md:743`, `x.dspy.*`,
  `x.skill_bank.*`, `x.inspect.*`, `x.git.*`, etc. could become a junk
  drawer of Python-shaped adapters. If `x.dspy.signature` payloads are
  pydantic-model-dumped and the Go SDK has no parallel structured-output
  library, Go users get raw `map[string]interface{}` everywhere in
  practice.
- Mitigation: every `x.*` namespace must publish its own JSON Schema
  alongside the core schemas. Python being the dominant ML language means
  `x.dspy.*` will inevitably be Python-first, but the *schemas* are
  language-neutral. The danger is `x.*` schemas that lean on Python-only
  conventions (e.g. positional argument lists, kwargs maps, pydantic
  validator names) leaking into the wire. The Leaven contract should
  require: `x.*` namespaces submit a JSON Schema + a non-Python smoke
  test before they can be ratified.

### 6.6 Decorator surface vs builder surface

- Risk: `@lv.evaluator(...)` is Python idiom. If the SDK only exposes the
  decorator and not an explicit registration call, the TS/Go SDKs will have
  to invent their own registration surface and the cross-language
  documentation drifts.
- Mitigation: even in Python, expose **both** `@lv.evaluator(...)` and
  `lv.register_evaluator(spec, func)`. Document the decorator as sugar over
  the registration call. The wire payload is identical.

## 7. Recommendation

**The proposed architecture is multi-language-safe.** The biggest single
reason: the wire is *already* JSON Schema, not pyo3 bindings, not a Python
class hierarchy, not even Rust serde derives that bleed Rust-isms. The
locked v1 wire uses the boring subset of JSON Schema 2020-12 that codegen
tools handle well in every target language.

Required mitigations to keep it safe:

1. **Ship a TS SDK in the same release window as the Python SDK** (or at
   least a read-only TS client that proves the schemas codegen cleanly to
   TS). Six months of Python-only is the LSP failure mode. Three months is
   probably fine; same release window is best.

2. **Make the Python SDK serializer canonical-omit on None.** Add a
   `leaven-public-seam` contract test that decodes a synthetic Python
   request and asserts no `null` literals survive. This kills the
   undefined-vs-null wire ambiguity at the schema-test layer.

3. **Cap large integers at `2^53 - 1`** for any field that could grow
   unbounded (cumulative `usd_micro`, lifetime token counts). Add a
   `maximum` to the schemas. One-line per field.

4. **Expose both decorator and explicit-registration surfaces in Python.**
   Document them as equivalent. This guarantees TS/Go SDKs map cleanly to
   the registration surface and the wire payload is identical regardless
   of which Python surface the user picked.

5. **Require `x.*` extension namespaces to publish JSON Schemas alongside
   their adapters.** The DSPy adapter inevitably ships Python-first, but
   the schema must be language-neutral. Make this a public-seam contract
   rule, not a convention.

6. **Document the snake_case decision in the schema README.** TS users
   will be tempted to camelCase-rename client-side. Tell them not to (or
   tell them the right transformer to use, consistently).

7. **Forbid the `null` literal in serialized requests** as a wire law (not
   a stylistic guideline). Add it to the ACP profile spec.

If those mitigations land, Path B (Leaven owns the transport via
`leaven-acp`, Python SDK first) is *better* than Path A (depend on the
third-party ACP SDK) for multi-language adoption — because Path B keeps
the schema-driven codegen path uniform across languages, instead of
inheriting whatever language-specific quirks the upstream SDK happens to
have today.

## Next actions

1. Add the seven mitigations to the parent ledger
   (`docs/working-memory/leaven-py-and-acp-transport.md`) as decisions to
   bake into the spec write.
2. When the spec write happens, encode mitigations 2, 3, and 7 as
   contract tests / schema additions in `leaven-public-seam`.
3. Encode mitigations 1, 4, 5, 6 in the eventual implementation plan
   (`docs/plans/2026-05-24-leaven-py-and-acp-transport.md`).

## External references

- [MCP architecture](https://modelcontextprotocol.io/docs/concepts/architecture)
- [MCP SDKs index](https://modelcontextprotocol.io/docs/sdk)
- [MCP Python SDK (FastMCP decorator surface)](https://github.com/modelcontextprotocol/python-sdk)
- [LSP overview](https://microsoft.github.io/language-server-protocol/overviews/lsp/overview/)
- [LSP 3.17 spec](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- [LSP metamodel + lsprotocol codegen](https://github.com/microsoft/lsprotocol)
- [LSP metamodel JSON](https://github.com/microsoft/vscode-languageserver-node/blob/main/protocol/metaModel.json)
- [DAP overview](https://microsoft.github.io/debug-adapter-protocol/overview)
- [ACP repo (multi-language SDKs)](https://github.com/zed-industries/agent-client-protocol)
- [esbuild PR #1621 (optionalDependencies pattern)](https://github.com/evanw/esbuild/pull/1621)
- [json-schema-to-typescript](https://www.npmjs.com/package/json-schema-to-typescript)
- [quicktype](https://github.com/glideapps/quicktype)
- [omissis/go-jsonschema](https://github.com/omissis/go-jsonschema)
- [Google jsonschema-go](https://opensource.googleblog.com/2026/01/a-json-schema-package-for-go.html)
- [Maturin (ruff-style Rust-in-Python wheel)](https://github.com/PyO3/maturin)
