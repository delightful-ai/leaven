---
title: "Trace2Skill: Distill Trajectory-Local Lessons into Transferable Agent Skills"
authors:
  - "Jingwei Ni"
  - "Yihao Liu"
  - "Xinpeng Liu"
  - "Yutao Sun"
  - "Mengyu Zhou"
  - "Pengyu Cheng"
  - "Dexin Wang"
  - "Erchao Zhao"
  - "Xiaoxi Jiang"
  - "Guanjun Jiang"
year: 2026
venue: "arXiv preprint"
doi: "arXiv:2603.25158"
ara_version: "1.0"
domain: "agent skill optimization"
keywords:
  - "agent skills"
  - "trajectory distillation"
  - "SpreadsheetBench"
  - "skill evolution"
  - "multi-agent patch proposal"
  - "hierarchical consolidation"
  - "Leaven reproduction"
claims_summary:
  - "Trace2Skill distills many trajectory-local lessons into one portable skill directory."
  - "Paper tables report improved SpreadsheetBench, WikiTQ, math, and DocVQA results for evolved skills."
  - "Parallel consolidation is reported as faster and stronger than sequential online editing on SpreadsheetBench."
  - "The Leaven reproduction is not proven until result overlays are produced against the paper denominator."
abstract: "Trace2Skill proposes parallel trajectory analysis and hierarchical consolidation to evolve agent skill directories without parameter updates or retrieval modules. This ARA captures the paper's claims, source anchors, result tables, configs, and reproduction constraints for Leaven's 1:1 Trace2Skill / SpreadsheetBench goal."
---

# Trace2Skill

## Overview

Trace2Skill treats an agent skill as a structured directory rooted at `SKILL.md`.
The paper's core contribution is a three-stage loop: generate trajectories with a
frozen initial skill, ask independent success/error analyst agents to propose
patches, then hierarchically merge those patches into a single conflict-checked
skill update. The main Leaven objective is to reproduce the paper denominator
honestly, not merely reproduce this shape with a proxy task.

This ARA is the scoreboard and denominator for the Leaven reproduction. Paper
target values live in `evidence/`; future Leaven result overlays must be stored
separately and labeled by denominator.

## Layer Index

### Cognitive Layer (`/logic`)

| File | Description |
|------|-------------|
| [problem.md](logic/problem.md) | Observations, gaps, key insight, and assumptions. |
| [claims.md](logic/claims.md) | Falsifiable paper and Leaven-reproduction claims. |
| [concepts.md](logic/concepts.md) | Formal terms: skill, trajectory corpus, patch pool, merge tree, and denominator. |
| [experiments.md](logic/experiments.md) | Declarative experiments with directional expected outcomes only. |
| [solution/architecture.md](logic/solution/architecture.md) | Component graph for Trace2Skill and the Leaven reproduction path. |
| [solution/algorithm.md](logic/solution/algorithm.md) | Algorithm and pseudocode for trajectory-to-skill consolidation. |
| [solution/constraints.md](logic/solution/constraints.md) | Limits, blocker classes, and proxy-refusal boundaries. |
| [solution/heuristics.md](logic/solution/heuristics.md) | Reproduction-critical heuristics and source-backed sensitivities. |
| [related_work.md](logic/related_work.md) | Typed related-work map from the paper and reproduction context. |

### Physical Layer (`/src`)

| File | Description | Claims |
|------|-------------|--------|
| [configs/training.md](src/configs/training.md) | Dataset, seed, worker, merge, and turn-budget configuration. | C01, C02, C03 |
| [configs/model.md](src/configs/model.md) | Model and serving configuration. | C01, C02, C03 |
| [configs/tolerance.md](src/configs/tolerance.md) | Proposed reproduction tolerance and failure-accounting policy. | C01, C02, C07 |
| [execution/trace2skill_pipeline.py](src/execution/trace2skill_pipeline.py) | Typed stub of the paper loop and Leaven result denominator records. | C01, C07 |
| [environment.md](src/environment.md) | Dependencies, hardware, code paths, and approval notes. | C07 |

### Exploration Graph (`/trace`)

| File | Description |
|------|-------------|
| [exploration_tree.yaml](trace/exploration_tree.yaml) | Research DAG reconstructed from explicit paper sections and marked inferences. |

### Evidence (`/evidence`)

| File | Description |
|------|-------------|
| [README.md](evidence/README.md) | Evidence index mapping tables and figures to claims. |
| [tables/table_main_spreadsheetbench.md](evidence/tables/table_main_spreadsheetbench.md) | Raw paper Table `tab:main_v1`. |
| [tables/table_parallel_vs_sequential.md](evidence/tables/table_parallel_vs_sequential.md) | Raw paper Table `tab:seq_parallel`. |
| [tables/table_reasoningbank.md](evidence/tables/table_reasoningbank.md) | Raw paper Table `tab:reasoning_bank`. |
| [tables/table_agentic_ablation.md](evidence/tables/table_agentic_ablation.md) | Raw paper Table `tab:agentic_ablation`. |
| [tables/table_math.md](evidence/tables/table_math.md) | Raw paper Table `tab:math`. |
| [tables/table_vqa.md](evidence/tables/table_vqa.md) | Raw paper Table `tab:vqa`. |
| [figures/figure_trace2skill_framework.md](evidence/figures/figure_trace2skill_framework.md) | Source figure path and caption for the Trace2Skill pipeline. |
| [leaven_mechanics_tests.md](evidence/leaven_mechanics_tests.md) | Leaven mechanics-test proof classifications and limits. |
| [prompt_templates.md](evidence/prompt_templates.md) | Upstream prompt-template family index and reproduction boundary. |

### Plot Targets and Validation

| File | Description |
|------|-------------|
| [plots/README.md](plots/README.md) | Explains target-plot meaning and proxy limits. |
| [plots/trace2skill_targets.png](plots/trace2skill_targets.png) | Paper target sheet generated from ARA evidence tables. |
| [results/README.md](results/README.md) | Leaven result-record rules, proof classifications, and overlay binding. |
| [results/leaven_result_schema.md](results/leaven_result_schema.md) | JSONL schema for real Leaven metrics that can be plotted against paper targets. |
| [results/denominator_status.md](results/denominator_status.md) | Handoff acceptance audit and current denominator status. |
| [results/dataset_manifest.json](results/dataset_manifest.json) | Deterministic local manifest for the 400-row SpreadsheetBench-Verified dataset. |
| [results/one_case_live.md](results/one_case_live.md) | Deterministic one-case ACP worker proof and artifact manifest. |
| [results/full_run_plan.md](results/full_run_plan.md) | Approval gate for Qwen/vLLM paper-denominator execution. |
| [results/model_availability.md](results/model_availability.md) | Current public model/serving availability research for the approval packet. |
| [reviews/rigor_review.md](reviews/rigor_review.md) | Seal Level 2 semantic rigor review and blockers. |
| [level2_report.json](level2_report.json) | Machine-readable Seal Level 2 review report. |
| [coverage.md](coverage.md) | Coverage pass notes and known gaps. |
| [validation.md](validation.md) | Local Seal Level 1 validation commands and results. |
