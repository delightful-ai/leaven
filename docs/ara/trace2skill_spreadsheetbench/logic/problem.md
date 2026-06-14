# Problem Specification

## Observations

### O1: Manual skills do not transfer reliably
- **Statement**: The paper reports that a human-written `xlsx` skill scores 48.33 Vrf for the Qwen3.5-122B-A10B skill user on SpreadsheetBench-Verified but 9.67 Vrf for the Qwen3.5-35B-A3B skill user.
- **Evidence**: `evidence/tables/table_main_spreadsheetbench.md`; source `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_main_v1.tex`.
- **Implication**: A high-quality manual skill can be model-sensitive, so the reproduction must test model/user combinations rather than one successful path.

### O2: Parametric skill creation is weak in the spreadsheet domain
- **Statement**: The paper reports a Parametric reference score of 26.17 Vrf for 122B and 20.17 Vrf for 35B on SpreadsheetBench-Verified.
- **Evidence**: `evidence/tables/table_main_spreadsheetbench.md`.
- **Implication**: A reproduction must distinguish trajectory-grounded skill creation from a generic LLM-drafted skill.

### O3: Parallel consolidation is a central claimed advantage
- **Statement**: The paper reports Parallel (ours) at 65.83 Vrf for 122B and about 3 minutes, compared with Seq-B=1 at 61.83 Vrf and about 60 minutes, and Seq-B=4 at 59.00 Vrf and about 15 minutes.
- **Evidence**: `evidence/tables/table_parallel_vs_sequential.md`.
- **Implication**: Runtime and merge topology are part of the reproduction denominator, not optional presentation details.

### O4: Retrieval memory is a named baseline
- **Statement**: The paper reports Human-Written+Combined (ours) outperforming ReasoningBank on all six same-model SpreadsheetBench cells in Table `tab:reasoning_bank`.
- **Evidence**: `evidence/tables/table_reasoningbank.md`.
- **Implication**: A 1:1 reproduction must keep portable-skill and retrieval-memory baselines separate.

### O5: Agentic analysis is a claimed source of transfer
- **Statement**: The paper reports qualitative evidence from 33 shared error cases: agreement on 4 cases (12.1%), disagreement in 18 cases (54.5%), and parse-error over-attribution in 57% of LLM-only cases versus 14% for agentic analysis.
- **Evidence**: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:820`; `evidence/tables/table_agentic_ablation.md`.
- **Implication**: A Leaven mechanics replay of saved patches does not reproduce the agentic-analysis claim unless live analyst behavior or faithful upstream artifacts are included.

## Gaps

### G1: Existing Leaven mechanics do not yet prove paper parity
- **Statement**: The current Leaven dossier says Trace2Skill has mechanics-smoke coverage for manifest lowering, upstream artifact import, JSON patch lowering/application, merge replay, and saved-output directory loading, while the tiny live shell harness is a proxy and not SpreadsheetBench/Qwen/vLLM parity.
- **Caused by**: O3, O5.
- **Existing attempts**: `examples/trace2skill_spreadsheetbench` and `examples/trace2skill_tiny_live`.
- **Why they fail**: They do not run the full paper model, splits, worker count, seed aggregation, or held-out denominator.

### G2: Target plots can masquerade as results
- **Statement**: Paper tables can be plotted before any Leaven run exists.
- **Caused by**: O1, O2, O3, O4.
- **Existing attempts**: Scratch plot sheet generated from paper numbers.
- **Why they fail**: Target plots are the scoreboard, not reproduced evidence.

### G3: Full live reproduction has compute and approval dependencies
- **Statement**: The paper uses Qwen3.5-122B-A10B and Qwen3.5-35B-A3B through vLLM, 128 sub-agents, merge batch size 32, seeds 41/42/43, and 100 ReAct turns.
- **Caused by**: O3.
- **Existing attempts**: No full-denominator run is recorded in this ARA.
- **Why they fail**: Large model availability, hardware, and cost must be approved before scale execution.

## Key Insight

- **Insight**: Use an ARA as the reproduction denominator: first bind every paper claim to exact source evidence, then require Leaven result overlays to declare the exact denominator they prove.
- **Derived from**: O1-O5 and G1-G3.
- **Enables**: Honest incremental progress from target plots to mechanics, one-case live proof, subset proof, held-out proof, seed aggregation, and only then full paper parity.

## Assumptions

- A1: `tmp/skill_opt_sources/arx_2603.25158` is the local source bundle for the Trace2Skill paper.
- A2: `tmp/repros/trace2skill-upstream` and `/Users/darin/vendor/github.com/Qwen-Applications/Trace2Skill` are the available upstream code references.
- A3: Live Qwen/vLLM-scale execution requires explicit approval before running.
- A4: Until Leaven result overlays exist, all numeric tables in this ARA are paper targets.
