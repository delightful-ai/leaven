# Layer 2 Evidence And Trace Selection

Status: active findings recorded.

This file audits whether GEPA customizers can select and read the trace,
feedback, score, and evidence context needed for reflection.

## Findings

### L2-008: GEPA drops feedback and trace before proposal

- severity: high
- evidence: `docs/specs/gepa_optimizer_surface.md:475-483`,
  `crates/leaven-evidence/src/feedback.rs:33`,
  `crates/leaven-gepa/src/optimizer.rs:57-78`,
  `crates/leaven-gepa/src/optimizer.rs:612-620`
- promised behavior: GEPA can select evaluator evidence, casewise scores,
  natural-language feedback, attribution, stdout/stderr, transcript refs,
  errors, and prior summaries for reflection.
- actual behavior: evaluation evidence is projected into scalar casewise
  summaries before population/proposal. Feedback and trace payloads do not
  reach the reflector.
- why it matters: ASI-style reflection and failure-attributed part selection
  cannot be implemented through the current GEPA surface.
- correction direction: add an explicit feedback/evidence selection stage that
  preserves evidence refs and selected payload/rendered views for reflection.

### L2-009: Part selection is a placeholder, not evidence-attributed selection

- severity: medium
- evidence: `crates/leaven-gepa/src/part_selector.rs:72`,
  `docs/specs/initial_library.md:3432-3457`
- promised behavior: part selection can use evidence attribution and failure
  context to choose what part of an artifact to mutate.
- actual behavior: `WorstEvidencePart` is a name without the behavior needed
  to inspect the worst evidence and select an attributed surface part.
- why it matters: users cannot swap in the GEPA behavior that focuses edits on
  failed instructions or prompt sections.
- correction direction: define the part-selection input as selected candidate,
  surface, evidence/trace selection, and scoring summary. Keep the selector
  sync only if the evidence view it receives is already complete.

### L2-010: Rendering scaffolds are exported without behavior

- severity: medium
- evidence: `docs/specs/gepa_public_private_surface.md:302`,
  `docs/specs/gepa_optimizer_surface.md:473`,
  `crates/leaven-render/src/prompt.rs:1`,
  `crates/leaven-render/src/surface.rs:1`,
  `crates/leaven-render/src/run_graph.rs:1`
- promised behavior: trace/evidence rendering is the bridge from durable graph
  evidence to reflection prompts.
- actual behavior: `ReflectionPromptRenderer`, `SurfacePartsRenderer`, and
  `RunGraphDebugRenderer` are exported empty structs.
- why it matters: even after GEPA receives richer evidence refs, there is no
  standard rendered context to feed an LM or agent.
- correction direction: implement a minimum reflection renderer or stop
  exporting renderer names as usable standard pieces.
