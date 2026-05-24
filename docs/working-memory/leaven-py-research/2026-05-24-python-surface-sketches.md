# Python Surface Sketches

Status: pre-spec design sketches.
Updated: 2026-05-24.

## Authority

This file is subordinate to:

- `docs/working-memory/leaven-py-and-acp-transport.md` (the parent research
  note this captures durable sketches for).
- `docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py` (the
  locked aspirational evaluator example).
- `docs/specs/public-seam-v1-lock-draft.archived/COMPREHENSIVE_DESIGN_PASS_NOTES.md`
  (archived design rationale; line 15 = "200 lines of glue in whatever
  language the user wants" target).

These are design sketches, not implementation contracts. The shape can
move in response to spec/code reality. Their purpose is to make the
architectural decisions concrete enough to react to at the alignment
checkpoint.

Sketches captured from the 2026-05-24 design conversation that produced
this research thread, plus the locked-spec example for reference.

## 1. The locked-spec aspirational evaluator (reference)

Already lives at `docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py`.
This is the only Python sketch that is currently *spec-locked*. Reproduced
here only to anchor the comparison to the broader sketches that follow.

```python
import dspy
import leaven as lv
from pydantic import BaseModel, Field

class JudgeResult(BaseModel):
    score: float = Field(ge=0.0, le=1.0)
    feedback: str
    verdict: str

class SkillJudge(dspy.Module):
    def __init__(self):
        super().__init__()
        self.grade = dspy.ChainOfThought(
            "task, target, pytest_report, workspace_diff, agent_report -> score: float, feedback: str, verdict: str"
        )

    def forward(self, *, task, target, pytest_report, workspace_diff, agent_report):
        return self.grade(task=task, target=target, pytest_report=pytest_report,
                          workspace_diff=workspace_diff, agent_report=agent_report)

@lv.evaluator(id="skillbank/pytest-dspy-codex",
              trust_profile="managed_sandbox",
              granularity="per_case")
async def evaluate(job: lv.EvaluationJob, cx: lv.EvalContext):
    judge = SkillJudge()
    assessments = []
    with lv.dspy_context(cx, model_role="grader", strict=True):
        for item in job.independent_cases():
            case = await cx.case.load(item.case_id,
                                       include=["input", "target", "metadata"])
            ws = await cx.workspace.materialize_candidate(
                item.candidate_id, surface="full_repo", lifetime="stage_call")
            async with cx.batch() as b:
                diff = b.workspace.git_diff(ws, against="parent",
                                             expected_data_classes=["workspace.file"])
                status = b.workspace.git_status(ws)
                tests = b.sandbox.exec(
                    workspace=ws,
                    argv=["pytest", "-q", "tests/hidden", "--json-report"],
                    timeout_s=180,
                    output=lv.output.files(["report.json", "pytest.log"],
                                           max_bytes=256_000),
                    input_classes=["case.target", "workspace.file"],
                    forbidden_input_classes=["workspace.secret"],
                )
                agent = b.agent.run(
                    runtime="codex-app-server",
                    workspace=ws,
                    instructions=lv.AgentInstructions(
                        task=f"Task: {case.input['task']}\nRubric: {case.target['rubric']}"),
                    output=lv.output.json_schema(JudgeResult),
                    input_classes=["case.input", "case.target", "workspace.file"],
                    forbidden_input_classes=["workspace.secret"],
                )
            diff, status, tests, agent = await b.run()
            report = lv.pytest.parse_json_report(tests.files["report.json"])
            with lv.dspy_call_context(
                    input_classes=["case.input", "case.target",
                                   "candidate.output", "workspace.file"],
                    forbidden_input_classes=["workspace.secret"]):
                pred = await lv.dspy_acall(
                    judge,
                    task=case.input["task"],
                    target=case.target["rubric"],
                    pytest_report=report.summary,
                    workspace_diff=diff.text,
                    agent_report=agent.parsed["feedback"])
            judged = JudgeResult.model_validate(pred.to_dict())
            output = lv.OutputRecord.text(
                summary=judged.feedback,
                visibility="optimizer_visible",
                data_classes=["optimizer.visible", "candidate.output"])
            assessments.append(lv.AssessmentWrite.independent_case(
                candidate=item.candidate_id,
                case=item.case_id,
                score=lv.Score(value=0.6 * report.pass_rate + 0.4 * judged.score,
                                output=output,
                                metrics={"pytest_pass_rate": report.pass_rate,
                                         "judge_score": judged.score}),
                evidence=lv.EvidenceEnvelope.public_private(
                    public={"feedback": judged.feedback,
                            "metrics": {"pytest_pass_rate": report.pass_rate,
                                        "judge_score": judged.score},
                            "data_classes": ["optimizer.visible"]},
                    private={"visibility": "evaluator_only",
                             "payload": {"target_ref": case.target_ref,
                                         "git_status": status.entries},
                             "data_classes": ["case.target", "evaluator.private"]},
                    target_derived=True,
                ),
                read_receipts=[case.receipt, diff.receipt, status.receipt],
                effect_receipts=[tests.receipt, agent.receipt,
                                  pred.leaven_lm_receipt],
                replayability="boundary_managed",
            ))
    return await cx.assessments.submit(job.evaluation_request_id, assessments)
```

This sketch is **evaluator-shaped only**. It does not show how the run is
configured or invoked from Python. That's what the rest of this file fills
in.

## 2. EvoSkill-shaped full repro

Sketch from the design conversation after the user pushed back on the
eval-only framing. This is the "200-line Python paper repro" target: full
optimization composition from Python, with stage bodies authored
inline, configuration and execution end-to-end.

```python
import leaven as lv
from leaven import optimizers, lm, agent, workspace, frontier, budget, cases

# --- Cases ----------------------------------------------------------------
officeqa = cases.officeqa(split="train_24", val="val_17", test="test_held_out")

# --- Environment ----------------------------------------------------------
env = lv.environment(
    workspace=workspace.local(root=".agents"),
    lm=lm.anthropic(model="claude-opus-4-7"),
    agent=agent.codex(model="gpt-5-codex"),
    trust_profile="managed_sandbox",
    budget=budget(usd=200, calls=2000),
)

# --- Optimizer ------------------------------------------------------------
opt = optimizers.gepa(
    population_size=10,
    frontier=frontier.top_k(3),
    parent_selector="round_robin",
    reflection_lm=lm.anthropic(model="claude-opus-4-7"),
    minibatch_size=4,
)

# --- Stages ---------------------------------------------------------------
@lv.runner
async def run(bank: lv.SkillBank, case: lv.Case, cx: lv.RunContext):
    ws = await cx.workspace.materialize_candidate(
        bank.candidate_id, surface="full_repo", lifetime="stage_call")
    await cx.workspace.write_skills(ws, bank)
    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task=case.input["task"],
            developer=lv.roles.EXECUTOR,
        ),
        output=lv.output.json_schema(lv.SkillExecutionAnswer),
        timeout_s=240,
    )
    return session.parsed.answer

@lv.scorer
async def score(output: str, case: lv.Case, cx: lv.RunContext):
    return lv.Score(
        value=lv.scoring.multi_tolerance(
            output, case.target["answer"],
            tolerances=[0.0, 0.01, 0.025, 0.05, 0.10]),
        output=lv.OutputRecord.text(summary=output,
                                     visibility="optimizer_visible"),
    )

# Optional — override the default skill-builder. If not provided, GEPA
# uses its built-in proposer.
@lv.proposer
async def propose(reflection: lv.ReflectionResult, cx: lv.RunContext):
    ws = await cx.workspace.materialize_candidate(
        reflection.parent_candidate_id, surface="skills_only",
        lifetime="stage_call")
    await cx.workspace.write_file(ws, "REFLECTION.md", reflection.diagnosis)
    await cx.workspace.write_file(ws, "FAILURES.json",
                                   reflection.failures.to_json())
    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task="Propose a skill-bank change that addresses the reflection.",
            developer=lv.roles.SKILL_PROPOSER,
        ),
        output=lv.output.json_schema(lv.SkillProposalAnswer),
        timeout_s=180,
    )
    return lv.ProposalBatch.from_skill_proposal(session.parsed)

# --- Run ------------------------------------------------------------------
result = await lv.optimize(
    seed=lv.SkillBank.empty(),
    train=officeqa.train,
    val=officeqa.val,
    test=officeqa.test,
    optimizer=opt,
    environment=env,
    runner=run,
    scorer=score,
    proposer=propose,  # optional
).run()

# --- Inspect --------------------------------------------------------------
print(f"Best: {result.best.summary()}")
print(f"Total cost: ${result.summary.total_cost_usd:.2f}")
print(f"Frontier: {[c.id for c in result.frontier]}")

for assessment in result.test_assessments():
    print(f"  {assessment.case.id}: {assessment.score.value:.3f}")

# Replay deterministically
replay = await result.replay(case_id="officeqa-test-0042")
assert replay.score.value == result.assessment("officeqa-test-0042").score.value
```

Total: ~70 lines, including comments. Stage bodies are ~20 lines combined.
The remaining ~50 lines are configuration and execution, which is the
substrate the Python SDK ships once and never re-pays.

Compare to current state: `examples/p5_evoskill_iteration/src/main.rs` is
2,417 lines for the equivalent single-iteration version.

## 3. Minimal prompt-optimization sketch (DSPy-flavored)

The smallest meaningful sketch: optimize a prompt for a QA task using
GEPA, with DSPy as the LM adapter and a simple exact-match scorer.

```python
import leaven as lv
import dspy

dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(model="claude-opus-4-7"))

@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.Case, cx: lv.RunContext):
    return await cx.lm.complete(
        prompt=prompt.template.format(**case.input),
        max_tokens=128,
    ).then(lambda r: r.text.strip())

@lv.scorer
async def score(output: str, case: lv.Case, cx: lv.RunContext):
    return lv.Score.exact_match(output, case.target["answer"])

result = await lv.optimize(
    seed=lv.PromptArtifact(template="Answer the question: {question}\nA:"),
    train=lv.cases.from_jsonl("train.jsonl"),
    val=lv.cases.from_jsonl("val.jsonl"),
    optimizer=lv.optimizers.gepa(population_size=8),
    environment=lv.environment.local(budget=lv.budget(usd=20)),
    runner=run,
    scorer=score,
).run()

print(result.best.artifact.template)
```

~20 lines. The Python SDK's reach is measured by how close to this
density the simplest possible repro can get.

## 4. Stage-author-only sketch (the existing aspirational shape)

If a user only wants to author a custom stage that the engine drives
under someone else's optimization config:

```python
import leaven as lv

@lv.scorer(id="my-team/llm-judge", trust_profile="managed_sandbox")
async def judge(output: str, case: lv.Case, cx: lv.StageContext):
    judgment = await cx.lm.complete(
        prompt=lv.templates.JUDGE_PROMPT.render(
            output=output,
            target=case.target["rubric"]),
        response_format=lv.LMResponseFormat.json_schema(lv.JudgeResponse),
    )
    parsed = judgment.parsed
    return lv.Score(
        value=parsed.score,
        output=lv.OutputRecord.text(
            summary=parsed.feedback,
            visibility="optimizer_visible"),
        metrics={"confidence": parsed.confidence},
    )

if __name__ == "__main__":
    lv.serve_stage(judge)  # spawns the ACP worker, listens for stage calls
```

This is the LSP-server analog: a Python process that the engine spawns
when it needs the stage, talks to over ACP, and tears down when the
stage call completes. The user writes one function and one
`serve_stage()` call.

## 5. Inspection-only sketch

A Python script that reads a completed run's RunGraph from outside the
optimization context. Same `leaven` package; no `lv.optimize()` call.

```python
import leaven as lv

run = lv.runs.open(".leaven/runs/2026-05-24-evoskill-officeqa")

print(f"Status: {run.status}")
print(f"Total cost: ${run.summary.total_cost_usd:.2f}")
print(f"Best candidate: {run.best.id}")

# Walk the lineage of the best candidate
for ancestor in run.lineage(run.best.id):
    print(f"  {ancestor.id}: {ancestor.proposal.summary()}")

# Get all assessments above 0.8 from the held-out test split
strong = run.assessments(split="test").filter(lambda a: a.score.value > 0.8)
for a in strong:
    print(f"  {a.case.id}: {a.score.value:.3f} ({a.candidate.id})")

# Inspect evidence on a specific assessment
ev = run.evidence(case_id="officeqa-test-0042", candidate_id=run.best.id)
print(ev.public["feedback"])
```

Same `leaven` package serves three different purposes (compose +
configure + run, author a stage, inspect after the fact). One install,
one mental model.

## 6. What these sketches load-bearingly assume

For these sketches to actually work, the Python SDK + `leaven-acp` +
schema codegen need to deliver:

1. **Typed records** for: `SkillBank`, `PromptArtifact`, `Case`,
   `RunContext` / `StageContext` / `EvalContext`, `Score`, `OutputRecord`,
   `EvidenceEnvelope`, `AssessmentWrite`, `ProposalBatch`, `ReflectionResult`,
   `AgentInstructions`, `LMResponseFormat`, `RunResult`, `Optimized`,
   `EvaluationJob`. The public-seam-derived ones come from JSON Schema
   codegen; the artifact-specific ones (`SkillBank`, `PromptArtifact`)
   need a per-artifact codegen story not yet specced.

2. **Decorators**: `@lv.evaluator`, `@lv.runner`, `@lv.scorer`,
   `@lv.proposer`, `@lv.reflector`, `@lv.judge`. Each wraps a function
   into a stage handler registered with `lv.serve_stage()` or
   `lv.optimize(...)`.

3. **Async fluent builders**: `cx.workspace.materialize_candidate(...)`,
   `cx.batch()` context manager, `cx.lm.complete(...).then(...)`,
   `cx.agent.run(...)`, `cx.sandbox.exec(...)`. Each constructs a
   Plan IR op; `cx.batch()` accumulates ops into one Plan IR document
   sent over a single ACP `leaven/execute_plan` call.

4. **Optimizer registry**: `lv.optimizers.gepa(...)`, `lv.optimizers.mipro(...)`,
   etc. Each returns a typed config that `lv.optimize()` passes through
   to the Rust engine for instantiation. Python users do not write
   optimizer bodies.

5. **Environment composition**: `lv.environment(workspace=..., lm=..., agent=..., trust_profile=..., budget=...)`.
   Each component is a typed Python config that the Rust engine knows
   how to instantiate.

6. **Adapter namespaces**: `lv.x.dspy.LeavenDSPyLM` (DSPy drop-in per
   `COMPREHENSIVE_DESIGN_PASS_NOTES.md:735`). Future: `lv.x.skill_bank`,
   `lv.x.git_program`, etc., for artifact-specific helpers.

7. **Run-result inspection**: `result.best`, `result.frontier`,
   `result.test_assessments()`, `result.replay()`, `result.summary`.
   These are typed handles backed by ACP read methods (`leaven/graph_query`,
   `leaven/case_query`, etc.).

8. **External script open**: `lv.runs.open(path)` — reads a completed
   run's durable state via the same ACP read paths (engine is spawned
   read-only).

## 7. Open questions surfaced by writing the sketches

Writing the sketches surfaced gaps that prose discussion didn't:

1. **`cx.lm.complete(...).then(...)` chaining style.** Does the SDK
   expose `then()` on futures, or do users `await` and then `.text.strip()`
   manually? The sketch in §3 uses `.then(...)`; the others `await` and
   inline. Pick one canonical style.

2. **Builder lifetimes inside `cx.batch()`.** The `b.workspace.git_diff(...)`
   call inside `async with cx.batch() as b:` returns... what? A
   placeholder that becomes a real value after `await b.run()`? A
   future? The current sketch destructures `diff, status, tests, agent =
   await b.run()` which suggests the placeholders are positional. Spec
   needs to make this concrete.

3. **`lv.SkillBank.empty()` and `lv.PromptArtifact(...)` artifact
   construction.** These are artifact-specific Python classes. Where do
   they live in the package? `lv.artifacts.SkillBank.empty()` or
   `lv.skill_bank.SkillBank.empty()` or just `lv.SkillBank.empty()`?
   The decision affects the import surface and the codegen story.

4. **`lv.cases.officeqa(...)` shorthand vs `lv.cases.from_jsonl(...)`.**
   The first is a benchmark-specific helper; the second is a generic
   loader. The benchmark-specific shorthand is convenient but couples
   the SDK to benchmark catalogs. Probably the right answer is
   `from leaven.cases import officeqa; cases = officeqa(...)` —
   benchmarks are discoverable but not in the top-level `lv` namespace.

5. **Stage-author packaging for `lv.serve_stage()`.** A `judge.py`
   file with `lv.serve_stage(judge)` in `__main__` — does that become a
   subprocess the engine spawns by command, or does it need to be
   installed/registered first? The §4 sketch implies the former.
   Spec needs to lock the launch contract.

6. **Result type variance.** `result.test_assessments()` returns an
   iterable of `Assessment`s. `result.best` is a `Candidate`. Both
   appear in §2. The Python type signatures need to be solid enough
   that an IDE can autocomplete `result.best.summary()` without
   guessing.

7. **`lv.scoring.multi_tolerance(...)` as a Python helper.** Scorers
   are user code, but some scoring math is paper-shared. Where does it
   live? `lv.scoring.*` as a module of helpers? Per-paper helper
   packages? Probably `lv.scoring` with the most-common ones built in
   and an extension surface for custom.

## Provenance

Sketches captured from the 2026-05-24 design conversation that produced
`docs/working-memory/leaven-py-and-acp-transport.md` and the four
research files under `docs/working-memory/leaven-py-research/`. Section
1 is reproduced verbatim from
`docs/specs/public-seam-v1/examples/evaluator_dspy_codex.v0.3.py`.
Sections 2-5 were drafted during the conversation; sections 6-7 are
synthesis of what those sketches assume and the gaps they surface.
