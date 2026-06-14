# Claims

## C01: Trace2Skill improves skill quality on the paper's spreadsheet target
- **Statement**: The paper reports that evolved Trace2Skill skills improve SpreadsheetBench-Verified metrics over their corresponding Human-Written or Parametric baselines in multiple author/user/condition rows.
- **Status**: supported by paper evidence; not yet reproduced by Leaven.
- **Falsification criteria**: A faithful rerun under the paper denominator fails to reproduce the reported improvement pattern within the agreed statistical/reproduction tolerance, or source transcription is shown to be wrong.
- **Proof**: [E01]
- **Evidence basis**: Table `tab:main_v1` contains exact paper target values and deltas for SpreadsheetBench, WikiTQ, and Avg.
- **Interpretation**: The paper argues that trajectory-local lessons can become transferable declarative skills. Leaven has not yet reproduced that causal claim.
- **Dependencies**: C07
- **Tags**: SpreadsheetBench, skill evolution, paper target

## C02: Parallel consolidation is reported stronger and faster than sequential editing
- **Statement**: The paper reports Parallel (ours) as best on 122B Vrf/Soft/Hard and 35B Vrf while using about 3 minutes, compared with about 15 minutes for Seq-B=4 and about 60 minutes for Seq-B=1.
- **Status**: supported by paper evidence; not yet reproduced by Leaven.
- **Falsification criteria**: A faithful rerun shows sequential editing matching or exceeding the reported parallel score/time relationship, or source transcription is wrong.
- **Proof**: [E02]
- **Evidence basis**: Table `tab:seq_parallel` reports exact Vrf/Soft/Hard cells and approximate runtime cells.
- **Interpretation**: The paper attributes this to many-to-one consolidation over frozen initial-skill patches and fewer sequential LLM-call rounds.
- **Dependencies**: C07
- **Tags**: parallelism, sequential baseline, runtime

## C03: Distilled portable skills outperform retrieval-memory baseline in the paper
- **Statement**: The paper reports Human-Written+Combined (ours) outperforming ReasoningBank on all six same-model SpreadsheetBench cells in Table `tab:reasoning_bank`.
- **Status**: supported by paper evidence; not yet reproduced by Leaven.
- **Falsification criteria**: A faithful rerun shows ReasoningBank matching or exceeding the distilled skill under the same model/data protocol, or source transcription is wrong.
- **Proof**: [E03]
- **Evidence basis**: Table `tab:reasoning_bank` gives the direct same-model Deepening comparison.
- **Interpretation**: This supports the paper's claim that a single portable skill can be a better use of trajectory evidence than per-query retrieval.
- **Dependencies**: C07
- **Tags**: retrieval baseline, ReasoningBank, portable skill

## C04: Agentic error analysis is reported more transferable than a single LLM call
- **Statement**: The paper reports that +Error (ours) beats +Error LLM in Avg across all four Author--Mode combinations in Table `tab:agentic_ablation`.
- **Status**: supported by paper evidence; not yet reproduced by Leaven.
- **Falsification criteria**: A faithful rerun shows +Error LLM matching or exceeding +Error (ours) in Avg under the paper protocol, or source transcription is wrong.
- **Proof**: [E04]
- **Evidence basis**: Table `tab:agentic_ablation` reports exact metrics for +Error (ours) and +Error LLM.
- **Interpretation**: The paper links transfer to artifact access and causal diagnosis, but Leaven must prove live analyst behavior before claiming this.
- **Dependencies**: C07
- **Tags**: agentic analysis, ablation, transfer

## C05: Trace2Skill paper reports positive math and VQA transfer beyond spreadsheets
- **Statement**: The paper reports positive deltas for math table rows and mixed VQA deltas depending on author/user model.
- **Status**: supported by paper evidence; not yet reproduced by Leaven.
- **Falsification criteria**: Source transcription is wrong, or a faithful rerun fails to reproduce the reported non-spreadsheet pattern.
- **Proof**: [E05, E06]
- **Evidence basis**: Tables `tab:math` and `tab:vqa`.
- **Interpretation**: These tables expand the paper claim beyond SpreadsheetBench; the current Leaven goal remains centered on Trace2Skill / SpreadsheetBench unless explicitly expanded.
- **Dependencies**: C07
- **Tags**: math, DocVQA, generalization

## C06: Current Leaven assets are mechanics-smoke evidence, not full reproduction
- **Statement**: Existing Leaven Trace2Skill work proves selected lowering/replay/scoring mechanics but does not prove full SpreadsheetBench/Qwen/vLLM paper parity.
- **Status**: supported by local dossier evidence.
- **Falsification criteria**: Current repo state contains full held-out split, seed aggregate, model-matched result overlays and closeout evidence that this ARA has not recorded.
- **Proof**: [E07]
- **Evidence basis**: `docs/working-memory/trace2skill-replication.md`, `docs/working-memory/skill-paper-replication.md`, and `evidence/leaven_mechanics_tests.md` classify the current state as mechanics/proxy/one-case evidence rather than full paper parity.
- **Interpretation**: This is the core anti-proxy guardrail for the goal.
- **Dependencies**: none
- **Tags**: Leaven, proof classification, proxy refusal

## C07: Full 1:1 reproduction remains pending until Leaven produces denominator-labeled result overlays
- **Statement**: Leaven has not reproduced Trace2Skill 1:1 until result records distinguish paper targets from Leaven outputs and prove the relevant denominator.
- **Status**: hypothesis for the reproduction workflow; current status pending.
- **Falsification criteria**: A completed closeout artifact proves all handoff acceptance items and updates this claim with evidence.
- **Proof**: [E08]
- **Evidence basis**: Goal handoff artifact, execution plan, and `evidence/leaven_mechanics_tests.md` define the denominator and forbidden proxy completions.
- **Interpretation**: This claim governs closeout language more than the paper's scientific contribution.
- **Dependencies**: C01, C02, C03, C04, C05, C06
- **Tags**: reproduction denominator, Leaven result overlay, closeout
