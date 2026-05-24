# External-Language-Worker Prior Art

Status: research note, pre-spec.
Updated: 2026-05-24.

## Authority

This file is subordinate to:

- `docs/working-memory/leaven-py-and-acp-transport.md` (the parent research
  index; defines the four open research questions this file answers #4 of).
- `docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md` (the
  locked Leaven ACP profile that any prior-art pattern must be measured
  against, not used to override).
- `docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md`
  (archived design rationale; line 29 = pyo3 rejection, line 735 = DSPy
  drop-in adapter pattern).

This is research evidence. Do not promote claims here into product law without
the corresponding spec/code/test update.

## 1. The Optimizer-Engine Landscape

External-language stage authoring is rare. Most optimizer engines are
single-language (Python) and treat "distributed" as "same code, many
processes, shared store" rather than "different language, different worker."

### Optuna

Optimization workers are Python processes that share a centralized RDB
storage backend (MySQL/PostgreSQL); for very-large fan-out, a `GrpcStorageProxy`
(introduced in v4.2.0) sits between workers and the RDB to reduce load on
the single-point-of-failure database. The worker/server boundary is the
**storage API**, not the objective function: "by referencing data stored in
the RDB server, distributed optimization can be achieved using the same code
as a single program" — i.e. all workers run the same Python objective. There
is no cross-language worker surface. Sources:
[Easy Parallelization](https://optuna.readthedocs.io/en/stable/tutorial/10_key_features/004_distributed.html),
[gRPC Storage Proxy](https://medium.com/optuna/distributed-optimization-in-optuna-and-grpc-storage-proxy-08db83f1d608),
[FAQ](https://optuna.readthedocs.io/en/stable/faq.html).

### Ray Tune

The user authors a `Trainable` (callable or class). The driver process
serializes it via **cloudpickle** and ships it to each Ray actor worker; the
actor runs the callable on a separate thread, and `tune.report()` pauses that
thread to surface metrics back to the driver. Cross-language workers are
**not** supported — Trainables are Python only because cloudpickle is. There
is a documented friction at the `@ray.remote` boundary: nested actors lose
the ability to call `tune.report`, surfaced in
[ray#41124](https://github.com/ray-project/ray/issues/41124). Sources:
[Tune Lifecycle](https://docs.ray.io/en/latest/tune/tutorials/tune-lifecycle.html),
[Trainable](https://docs.ray.io/en/latest/tune/api/doc/ray.tune.Trainable.html).

### OSS Vizier

The only optimizer in this set with a **real cross-process** algorithm
boundary. Workers send a gRPC call asking for a suggestion; the Vizier
server spawns a "Pythia policy" worker to execute the algorithm and return
the response. The Developer API documents writing custom algorithms by
implementing the `Policy` abstract class with `suggest()` and `early_stop()`;
the `Designer` higher-level API has `update(completed, all_active)` and
`suggest(count)`. Pythia policies are nevertheless **Python in-process** on
the server side; the gRPC seam is only between client (objective) and server
(algorithm host), not between algorithm and host. No Java/Go SDK is
documented. Sources:
[Open Source Vizier paper](https://arxiv.org/abs/2207.13676),
[Pythia algorithms](https://oss-vizier.readthedocs.io/en/latest/guides/developer/writing_algorithms.html),
[google/vizier](https://github.com/google/vizier).

### DSPy

The closest analogue to what Leaven wants. `dspy.LM` (subclass of `BaseLM`)
is the canonical adapter seam: a user subclasses `BaseLM`, overrides
`forward(prompt, messages, **kwargs)` returning an OpenAI-shaped response,
and calls `dspy.configure(lm=MyLM(...))`. Everything else (`Predict`,
`ChainOfThought`, optimizers like `GEPA`, `MIPROv2`) consumes the LM through
this seam. The LM is **always Python**, in-process; "cross-language" is not
a concept DSPy has, but its adapter shape is exactly the contract Leaven
needs to expose to a Python child process. Source:
[base_lm.py](https://github.com/stanfordnlp/dspy/blob/main/dspy/clients/base_lm.py),
[lm.py](https://github.com/stanfordnlp/dspy/blob/main/dspy/clients/lm.py).
See section 6.

### OpenAI Evals

Eval authoring is **Python + YAML**, single-process. User subclasses
`evals.Eval`, overrides `run(recorder)` and `eval_sample(sample, rng)`, then
registers the class in `evals/registry/evals/<name>.yaml`. The docs
explicitly state "we are currently not accepting Evals with custom code" —
the public route is data + YAML for model-graded evals. No cross-language
worker authoring exists. Source:
[custom-eval.md](https://github.com/openai/evals/blob/main/docs/custom-eval.md),
[build-eval.md](https://github.com/openai/evals/blob/main/docs/build-eval.md).

### Inspect AI (UK AISI)

A solver is "a Python function that takes a TaskState and `generate`
function, and then transforms and returns the TaskState," registered via
`@solver`. Same story for `@scorer` and `@task`. TaskState carries
`messages`, `output`, `input`, `target`. **Inspect is Python-only**; no
non-Python solver or scorer interface is documented. Source:
[Solvers](https://inspect.aisi.org.uk/solvers.html),
[Scorers](https://inspect.aisi.org.uk/reference/inspect_ai.scorer.html),
[Tasks](https://inspect.aisi.org.uk/tasks.html).

**Landscape summary.** No optimizer engine in this set ships a cross-language
worker authoring surface today. Optuna and Vizier ship cross-process via
gRPC/RDB but with Python on both ends. DSPy ships a clean adapter
abstraction that *would* lower cleanly to JSON-RPC if anyone wired it. The
white-space Leaven is moving into is real.

## 2. The Agent-Engine Landscape

### LangChain / LangGraph

`StateGraph.add_node(name, fn)` accepts any Python callable or Runnable.
The function takes the State dict and a `RunnableConfig`; nodes can also
be other Runnables or LLM calls. There is a `RemoteGraph` primitive
(`from langgraph.pregel.remote import RemoteGraph`) that lets a graph
node call into a LangGraph deployment over HTTP — but the *node author*
is still Python. Source:
[add_node](https://reference.langchain.com/python/langgraph/graph/state/StateGraph/add_node),
[RemoteGraph](https://reference.langchain.com/python/langsmith/deployment/remote_graph/).

### CrewAI

Single-language Python. Agents are role-based Python classes with tools
attached (CrewAI Toolkit or LangChain tools). Production deployment is
"wrap the crew in a REST API and run executions in the background" — i.e.
the cross-language story is HTTP at the edge, not at the agent/tool seam.
Source: [CrewAI Agents](https://docs.crewai.com/en/concepts/agents),
[crewAI repo](https://github.com/crewAIInc/crewAI).

### Magentic-One (Microsoft AutoGen)

A team of Python agents (Orchestrator, Coder, ComputerTerminal,
MultimodalWebSurfer, FileSurfer) inside the AutoGen framework. Agents are
defined in Python; the orchestrator coordinates via AgentChat protocols.
No cross-language agent authoring surface; the closest is tool invocation
via subprocess, which is a tool-author concern, not a stage-author concern.
Source:
[autogen-magentic-one](https://github.com/microsoft/autogen/tree/main/python/packages/autogen-magentic-one),
[Magentic-One docs](https://microsoft.github.io/autogen/stable//user-guide/agentchat-user-guide/magentic-one.html).

**Agent-engine summary.** Same pattern as optimizers: cross-language is at
the *transport edge* (HTTP REST), not at the stage-author boundary. Stage
authoring is uniformly Python.

## 3. The Wire-Protocol Lineage

Three precedents matter for Leaven ACP: LSP (oldest, most successful at
scale), MCP (newest, closest in spirit), ACP (sibling, sharing infrastructure
with MCP).

### LSP — Language Server Protocol

**Strengths.** The reference design for "external workers via stdio JSON-RPC
at industry scale." LSP succeeded because the editor/server contract is
*sharp* (well-defined capability negotiation in `initialize`, well-defined
message set) and *language-neutral by construction*: every major language
ships an LSP server, and every major editor consumes them. The freecodecamp
intro captures the adoption story:
[freecodecamp LSP](https://www.freecodecamp.org/news/what-is-the-language-server-protocol-easier-code-editing-across-languages/).

**Weaknesses** (per matklad's
[LSP could have been better](https://matklad.github.io/2023/10/12/lsp-could-have-been-better.html)):
- *HTTP-like Content-Length framing is accidental complexity*; JSON Lines
  would be available out-of-the-box in every language.
- JSON-RPC noise (`jsonrpc: "2.0"`, error code `-32601` inherited from
  XML-RPC) clutters debugging.
- *Notifications prevent transmission-failure detection*: client cannot
  tell if a fire-and-forget message landed.
- *Causality is patched, not built in*: document versioning is "best effort"
  for ordering `didChangeTextDocument` against subsequent server actions.
- *Request/response instead of subscriptions* forces clients to re-query
  or show stale data; Dart's analyzer protocol is cited as the better model.
- *Multi-step interactive refactorings* (e.g. parameter reordering) cannot
  be expressed in the protocol.

**Leaven positioning.** Leaven's locked ACP profile uses **newline-delimited
JSON** ([04_stage_payloads_spec_v0.3.md is JSON-Schema-driven](../specs/public-seam-v1/)),
which sidesteps the Content-Length critique. The capability/scope handshake
in the public seam is sharper than LSP's because it carries explicit
capability tokens (per the locked spec). The notification-loss problem is
mitigated by Leaven's receipt-anchored design: stage results are receipts,
not notifications. The subscription gap is real and worth tracking — Leaven
already defers `watch` from v1 per `docs/working-memory/leaven-py-and-acp-transport.md:25`.

### MCP — Model Context Protocol

**Strengths.** Stdio JSON-RPC with broad multi-language SDKs (Python, TS,
Java, Kotlin, C#, community Rust/Go) and an inspector tool that exercises
the protocol surface. The "server in any language, host in any language"
promise is the closest to what Leaven needs. Source:
[Stainless MCP guide](https://www.stainless.com/mcp/error-handling-and-debugging-mcp-servers/),
[MCP spec](https://modelcontextprotocol.io/specification/2025-11-25).

**Weaknesses** (the catalogue from
[Why MCP's Disregard for 40 Years of RPC Best Practices Will Burn Enterprises](https://julsimon.medium.com/why-mcps-disregard-for-40-years-of-rpc-best-practices-will-burn-enterprises-8ef85ce5bc9b)
plus the framing-bug evidence):
- **Schemaless JSON with optional, non-enforced hints.** Type validation
  is runtime, if at all. Schema drift between SDK and spec is "the new
  dependency hell" per
  [dev.to: schema drift](https://dev.to/nesquikm/my-mcp-tools-broke-silently-schema-drift-is-the-new-dependency-hell-5c49).
- **Protocol version negotiation but no schema versioning**: tool
  interfaces change without warning.
- **No trace context propagation**; observability is bolted on per host.
- **stdout pollution is the #1 debugging issue**: any non-JSON byte on
  stdout corrupts the wire.
- **Windows CRLF translation bug** in the official Python SDK silently
  corrupted NDJSON for months —
  [python-sdk#2433](https://github.com/modelcontextprotocol/python-sdk/issues/2433).
- **stdio creates a fresh process per host connection**; no connection
  pooling at the protocol layer.

**Leaven positioning.** Leaven's public seam is **schema-locked** in
`docs/specs/public-seam-v1/schemas/`, with conformance gated by
`docs/specs/public-seam-v1/conformance-matrix.yaml` (32/39 rows proven per
the parent ledger). This directly inverts the MCP schemaless-drift failure
mode — but only if `leaven-acp` and the codegen pipeline actually enforce
the schemas at both ends. The CRLF/stdout-pollution failure modes are
generic stdio risks; Leaven inherits them and must enforce stderr-only
logging from `leaven serve --stdio` (the binary that the Python SDK spawns
per the parent ledger lines 41-45).

### ACP — Agent Client Protocol

**Strengths.** JSON-RPC 2.0 over stdio with clean lifecycle methods
(`initialize`, `session/new`, `session/list`, `session/resume`,
`session/close`, `authenticate`), explicit cancellation, capability
negotiation, content-block message shape borrowed from MCP, plan/tool-call
streaming. Adoption: 25+ agents and a real editor ecosystem (Zed, JetBrains,
community Neovim/Emacs/VS Code) by March 2026. Source:
[ACP introduction](https://agentclientprotocol.com/get-started/introduction),
[ACP llms.txt index](https://agentclientprotocol.com/llms.txt),
[Marc Nuri ACP intro](https://blog.marcnuri.com/agent-client-protocol-acp-introduction).

**Weaknesses.** Young (v0.11.0 in March 2026, protocol version "1"), and
the surface is editor-and-agent-shaped, not optimizer-shaped. Inheriting
the SDK directly couples Leaven to ACP's evolution velocity, which is the
explicit reason the parent ledger chose Path B
(`docs/working-memory/leaven-py-and-acp-transport.md:74-82`).

**Leaven positioning.** Leaven's profile (`leaven_acp_profile_v1_v0.3.md`)
reuses the ACP lifecycle and message envelope but constrains the
optimizer-specific method set. Path B means Leaven inherits the
*pattern*, not the *code*; it can fix the schema-drift risk and the
notification-loss risk inside `leaven-acp` while staying wire-compatible.

## 4. Patterns Worth Stealing

| # | Source | Pattern | Leaven design choice it shapes |
|---|---|---|---|
| 1 | LSP `initialize` handshake ([spec 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)) | Capability negotiation up front; both sides advertise what they support, no runtime "I don't know what to do" surprises | `leaven-acp` lifecycle must emit a single `initialize` exchange that includes capability tokens from the locked spec; the Python SDK must refuse to start a stage if a required capability is missing |
| 2 | MCP Inspector ([Stainless guide](https://www.stainless.com/mcp/error-handling-and-debugging-mcp-servers/)) | Browser/CLI harness that speaks the wire and lets a user fire arbitrary calls, see the JSON envelope, validate schemas | `leaven inspect --stdio <cmd>` subcommand (the parent ledger already commits to subcommand architecture, line 41) that wraps `leaven-acp` in a developer-facing inspector |
| 3 | DSPy `BaseLM.forward(prompt, messages, **kwargs) -> OpenAI-shaped` ([base_lm.py](https://github.com/stanfordnlp/dspy/blob/main/dspy/clients/base_lm.py)) | Single abstract method, well-typed response shape that all downstream modules consume | `leaven-lm` neutral request/response types must lower cleanly into the DSPy `BaseLM.forward` shape so `LeavenDSPyLM` is one file |
| 4 | OSS Vizier Pythia `Policy.suggest()` / `Designer.update()` ([writing_algorithms](https://oss-vizier.readthedocs.io/en/latest/guides/developer/writing_algorithms.html)) | Stateless policy that reconstructs state from full history on each call — robust to worker restarts | The Python SDK stage decorators (`@lv.reflector`, `@lv.proposer`) should treat each invocation as stateless; durable state lives in the receipts/lineage on the Rust side |
| 5 | ACP lifecycle methods ([llms.txt](https://agentclientprotocol.com/llms.txt)) | `session/new`, `session/resume`, `session/close` separate connection from work-unit | Leaven's plan-execution IR (see `docs/specs/public-seam-v1/`) already has this shape; ensure the Python SDK exposes it explicitly so a Python worker can resume a partially-completed run |
| 6 | LSP/MCP stderr discipline | Logs go to stderr, never stdout; stdout is wire-only | `leaven serve --stdio` must enforce this at the binary level. Document it in the Python SDK so a user `print()` in a stage callback does not corrupt the wire |
| 7 | DSPy `dump_state`/`load_state` with explicit class marker and `allow_custom_lm_class=False` default ([base_lm.py:175-215](https://github.com/stanfordnlp/dspy/blob/main/dspy/clients/base_lm.py)) | Serialized state is portable but won't deserialize untrusted custom subclasses without explicit opt-in | Receipt serialization in `leaven-public-seam` and replay in `leaven-acp` should refuse to instantiate arbitrary Python stage classes from receipts without explicit user opt-in |
| 8 | LangGraph `RunnableConfig` injection ([Runtime](https://reference.langchain.com/python/langgraph/runtime/Runtime)) | Node functions can pull a typed config/runtime object as a parameter | `cx` (the context object the parent ledger commits to at line 59) should be injectable into stage callbacks as a typed parameter, not a thread-local |

## 5. Failure Modes Worth Avoiding

| # | Source | What went wrong | Leaven protection |
|---|---|---|---|
| F1 | MCP schemaless JSON ([Julien Simon](https://julsimon.medium.com/why-mcps-disregard-for-40-years-of-rpc-best-practices-will-burn-enterprises-8ef85ce5bc9b)) | Tool interfaces change without warning; clients break silently | `leaven-types` is **codegen from JSON Schema 2020-12** per parent ledger line 50-54; conformance matrix is the proof gate, not vibes |
| F2 | MCP Python SDK CRLF bug ([python-sdk#2433](https://github.com/modelcontextprotocol/python-sdk/issues/2433)) | Windows TextIOWrapper silently translated `\n` to `\r\n`, corrupting NDJSON for users | `leaven-acp` and the Python SDK both open stdio in binary mode and write framed bytes; have an explicit Windows-newline regression test |
| F3 | LSP Content-Length framing ([matklad](https://matklad.github.io/2023/10/12/lsp-could-have-been-better.html)) | Custom framing requires custom parser in every implementation | Use **newline-delimited JSON** only; this is already the default of the locked spec but pin it in `leaven-acp` from day one |
| F4 | LSP notification semantics ([matklad](https://matklad.github.io/2023/10/12/lsp-could-have-been-better.html)) | Fire-and-forget notifications cannot signal transport failure | All Leaven stage results travel as request/response with receipts; reserve notifications only for progress events that are explicitly OK to drop |
| F5 | LSP request/response for highlight-style queries ([matklad](https://matklad.github.io/2023/10/12/lsp-could-have-been-better.html)) | Forces re-querying after every change | Leaven defers `watch` from v1 (good); when it lands, prefer a subscription model from the start |
| F6 | Optuna's "same Python everywhere" assumption ([Optuna FAQ](https://optuna.readthedocs.io/en/stable/faq.html)) | Cross-language is impossible because the worker IS the storage client running the user's objective in the same process | Keep the optimizer engine (Rust) and the stage worker (Python or other) on **opposite sides of the wire**; the worker never holds the storage client |
| F7 | Ray Tune cloudpickle Trainables ([ray#41124](https://github.com/ray-project/ray/issues/41124)) | Python-pickled callable cannot survive language/process boundaries; nested actors break reporting | The Leaven decorator surface (`@lv.evaluator` etc.) lowers to a JSON-RPC method name + schema-validated payload, never a serialized Python callable |
| F8 | OpenAI Evals "no custom code accepted" ([custom-eval.md](https://github.com/openai/evals/blob/main/docs/custom-eval.md)) | Eval authoring is bifurcated: YAML for the public route, Python class for advanced use; the advanced path is unmaintained | Resist the urge to ship a "YAML-only" eval surface in parallel with the Python SDK; one path, well-typed, both for trivial and advanced cases |
| F9 | MCP no trace context ([Julien Simon](https://julsimon.medium.com/why-mcps-disregard-for-40-years-of-rpc-best-practices-will-burn-enterprises-8ef85ce5bc9b)) | Cannot follow a decision path across tool calls | Receipts in `leaven-public-seam` already carry lineage/causal-input IDs; Python SDK must surface these as first-class fields on `cx.case` / `cx.proposals` so users can log them |
| F10 | MCP/CrewAI/LangChain version churn at the SDK level | Pinning a third-party SDK couples Leaven to its release cadence and breakage | Path B (own `leaven-acp`) per parent ledger line 74-82; the Python SDK depends on `leaven-types` (schema-derived) and a thin transport, not on the upstream ACP SDK |

## 6. The DSPy-as-Adapter-Namespace Pattern

The archived design pass at
`docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md:735`
proposes `dspy.configure(lm=LeavenDSPyLM(cx))` as the drop-in shape. Reading
the actual DSPy interface confirms this is achievable with a small, sharp
adapter.

### DSPy's actual `BaseLM` contract

From [`dspy/clients/base_lm.py`](https://github.com/stanfordnlp/dspy/blob/main/dspy/clients/base_lm.py):

- Subclass `dspy.BaseLM`.
- Override `forward(prompt: str | None, messages: list[dict] | None, **kwargs)`
  to return an **OpenAI chat-completion-shaped** object (or responses-API
  shape, or text-completion shape). The docstring is explicit: "the response
  should be identical to either of the following formats: [OpenAI response
  format], [OpenAI chat completion format], [OpenAI text completion format]."
- Optionally override `aforward` for async.
- Optionally expose capability hints: `supports_function_calling`,
  `supports_reasoning`, `supports_response_schema`, `supported_params`.
- Raise `dspy.ContextWindowExceededError` for context-window failures so
  adapters can trigger fallback truncation.
- Optionally override `dump_state`/`load_state` for serialization; default
  refuses untrusted custom subclasses unless `allow_custom_lm_class=True`.

The DSPy `LM` concrete subclass in `dspy/clients/lm.py` does its work
through litellm; the contract above is the **only** thing downstream DSPy
modules (`Predict`, `ChainOfThought`, `GEPA`, `MIPROv2`) require.

### Drop-in shape for Leaven

```python
# Approximate shape of x.dspy.LeavenDSPyLM
import dspy
import leaven as lv

class LeavenDSPyLM(dspy.BaseLM):
    def __init__(self, cx: lv.Context, model: str = "leaven-routed"):
        super().__init__(model=model, model_type="chat", cache=False)
        self._cx = cx

    def forward(self, prompt=None, messages=None, **kwargs):
        # Lower DSPy's OpenAI-shaped request into leaven-lm neutral
        # request, dispatch through cx, and re-pack the response into
        # OpenAI chat-completion shape.
        leaven_req = self._cx.lm.lower_request(
            messages=messages or [{"role": "user", "content": prompt}],
            **kwargs,
        )
        leaven_resp = self._cx.lm.invoke(leaven_req)
        return self._cx.lm.lift_response_openai_chat(leaven_resp)
```

`cx.lm` is the Python SDK handle to the Leaven LM seam over ACP. The
adapter is ~30 lines and depends only on the public `leaven-lm` neutral
request/response types.

### What `leaven-lm` neutral types must accommodate

To support the DSPy drop-in cleanly, `leaven-lm` request types must carry:

- **Either** a `prompt: str` **or** `messages: list[{role, content}]` (DSPy
  passes one or the other; the adapter must accept both shapes).
- `model`, `model_type` ∈ `{chat, text, responses}`, `temperature`,
  `max_tokens`, `cache`, plus pass-through `**kwargs` for provider-specific
  fields like `tools`, `tool_choice`, `response_format`, `reasoning_effort`,
  `rollout_id`, `logprobs`.
- Multi-modal content blocks (DSPy already supports image/audio/file types
  in `dspy/adapters/types/`).

And the response types must carry:

- `choices: list[{message: {content, role, tool_calls?, reasoning_content?}, finish_reason, logprobs?}]`
  to lower into OpenAI chat-completion shape.
- `usage: {prompt_tokens, completion_tokens, total_tokens}`.
- A `model` field (DSPy uses `response.model` in history entries).
- An optional cost field (DSPy reads `_hidden_params.response_cost` from
  litellm; Leaven should surface its own).

### Risk

The OpenAI chat/responses/text triple-shape is a moving target; DSPy
already absorbs the shape drift in `_process_completion` and
`_convert_chat_request_to_responses_request`. Leaven should commit to one
canonical shape on the wire (chat-completion is the obvious choice, since
that's what every DSPy adapter ultimately consumes) and do the
shape-translation work in the adapter, not in the seam.

## 7. Unknown / Underexplored

Things this research did not cover that the next agent should look at:

- **Roo Code / Continue.dev / Aider** — newer agentic coding tools that
  speak some flavor of stdio JSON-RPC and might have lessons on session
  persistence and cancellation under load.
- **Hugging Face Optimum / Hugging Face Spaces stage-author conventions** —
  community pattern for "publish a callable that the platform invokes."
- **Modal / Beam / Ray Serve** function-deployment surfaces — they all
  solve "user writes a function, platform invokes it over the wire" at
  scale; Modal in particular has interesting Python-decorator ergonomics.
- **NATS JetStream / Temporal worker SDKs** — the durable-execution-engine
  world solves the "external-language worker resumes after server
  restart" problem more rigorously than any AI tool here; worth a pass
  before locking the receipt-replay design.
- **Google ADK (Agent Development Kit)** and **Anthropic Agent SDK** for
  newer first-party agent authoring shapes.
- **DSPy 3.x roadmap and `LM.copy(rollout_id=...)` semantics** — DSPy
  uses `rollout_id` as a cache-bypass key; Leaven's caching layer
  needs to decide whether to mirror or reinterpret this.
- **Cap'n Proto / FlatBuffers** for the wire if JSON-Schema codegen
  pain becomes load-bearing — the parent ledger commits to JSON Schema
  2020-12 but does not analyze the cost of that choice at scale.
- **Vizier service authentication and multi-tenancy model** — if Leaven
  ever serves the optimizer over a network seam (beyond stdio), Vizier's
  client/server model is the closest precedent.

## Provenance

Created 2026-05-24 by the prior-art research agent answering question #4
of the parent ledger at
`docs/working-memory/leaven-py-and-acp-transport.md:107-111`. All external
citations were fetched live during this session; DSPy source citations are
from the indexed `stanfordnlp/dspy` repo as of the read timestamp on
2026-05-24.
