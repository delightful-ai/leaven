"""Workspace package groups shared by repository automation scripts."""

MILESTONE_PACKAGES = [
    "p0_graph_skeleton",
    "p1_keep_best",
    "p2_pairwise_tournament",
    "p3_gepa_parity",
    "p4_meta_harness_lite",
    "p5_evoskill_iteration",
    "p5_skill_paper_reproductions",
    "p6_optimizer_policy_self_opt",
    "p7_self_optimization_kernel",
    "p8_aime_gepa",
    "trace2skill_spreadsheetbench",
]


def package_exclude_args(packages: list[str]) -> list[str]:
    args: list[str] = []
    for package in packages:
        args.extend(["--exclude", package])
    return args
