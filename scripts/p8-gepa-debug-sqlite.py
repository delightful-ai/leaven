#!/usr/bin/env python3
"""Export P8/GEPA optimizer report facts into an inspectable SQLite database."""

from __future__ import annotations

import argparse
import json
import pathlib
import sqlite3
import sys
from typing import Any


def json_text(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def default_output_path(report_path: pathlib.Path) -> pathlib.Path:
    run_dir = report_path.parent.parent.name
    if not run_dir:
        run_dir = report_path.stem
    return pathlib.Path("target") / "gepa-debug" / f"{run_dir}.sqlite"


def source_maps(report: dict[str, Any]) -> tuple[dict[str, str], dict[str, int]]:
    case_to_source: dict[str, str] = {}
    source_to_val_index: dict[str, int] = {}
    for row in report.get("cases", []):
        case_id = row.get("case_id")
        source_id = row.get("source_id")
        if isinstance(case_id, str) and isinstance(source_id, str):
            case_to_source.setdefault(case_id, source_id)
    validation_sources = sorted(
        {
            row.get("source_id")
            for row in report.get("cases", [])
            if row.get("split") == "validation" and isinstance(row.get("source_id"), str)
        }
    )
    for index, source_id in enumerate(validation_sources):
        source_to_val_index[source_id] = index
    return case_to_source, source_to_val_index


def load_aime_validation_sources(cache_path: pathlib.Path | None) -> list[str]:
    if cache_path is None:
        return []
    cache = load_json(cache_path)
    return [row["source_id"] for row in cache.get("validation", [])]


def connect(path: pathlib.Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.execute("PRAGMA foreign_keys = ON")
    return connection


def create_schema(db: sqlite3.Connection) -> None:
    db.executescript(
        """
        DROP TABLE IF EXISTS metadata;
        DROP TABLE IF EXISTS runs;
        DROP TABLE IF EXISTS candidates;
        DROP TABLE IF EXISTS validation_scores;
        DROP TABLE IF EXISTS proposal_attempts;
        DROP TABLE IF EXISTS case_deltas;
        DROP TABLE IF EXISTS upstream_candidates;
        DROP TABLE IF EXISTS upstream_validation_scores;
        DROP TABLE IF EXISTS upstream_trace;
        DROP TABLE IF EXISTS validation_comparison;

        CREATE TABLE metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE runs (
            run_id TEXT PRIMARY KEY,
            report_path TEXT NOT NULL,
            run_dir TEXT,
            proof_classification TEXT,
            run_profile TEXT,
            gepa_profile TEXT,
            solver_model TEXT,
            reflection_model TEXT,
            baseline_validation REAL,
            optimized_validation REAL,
            baseline_test REAL,
            optimized_test REAL,
            search_metric_calls_spent INTEGER,
            final_report_metric_calls_spent INTEGER,
            total_metric_calls INTEGER,
            best_index INTEGER,
            validation_best_index INTEGER,
            candidate_count INTEGER,
            proposal_attempt_count INTEGER,
            accepted_count INTEGER,
            full_validation_evals INTEGER
        );

        CREATE TABLE candidates (
            run_id TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            candidate_id TEXT,
            parents_json TEXT NOT NULL,
            validation_score REAL,
            discovery_metric_calls INTEGER,
            prompt_chars INTEGER,
            prompt_source TEXT,
            system_prompt TEXT,
            PRIMARY KEY (run_id, candidate_index)
        );

        CREATE TABLE validation_scores (
            run_id TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            val_index INTEGER,
            case_id TEXT,
            source_id TEXT,
            score REAL,
            PRIMARY KEY (run_id, candidate_index, case_id)
        );

        CREATE TABLE proposal_attempts (
            run_id TEXT NOT NULL,
            attempt_index INTEGER PRIMARY KEY,
            iteration INTEGER,
            parent_index INTEGER,
            child_index INTEGER,
            accepted INTEGER NOT NULL,
            admitted INTEGER NOT NULL,
            parent_score REAL,
            child_score REAL,
            child_validation_score REAL,
            reflective_example_count INTEGER,
            child_cases_json TEXT NOT NULL,
            child_case_sources_json TEXT NOT NULL,
            reflection_model TEXT,
            proposed_chars INTEGER,
            assistant_chars INTEGER,
            proposed_text TEXT,
            assistant_text TEXT
        );

        CREATE TABLE case_deltas (
            run_id TEXT NOT NULL,
            split TEXT NOT NULL,
            case_id TEXT,
            source_id TEXT,
            baseline_score REAL,
            optimized_score REAL,
            score_delta REAL,
            outcome TEXT,
            PRIMARY KEY (run_id, split, source_id)
        );

        CREATE TABLE upstream_candidates (
            upstream_name TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            validation_score REAL,
            prompt_chars INTEGER,
            parents_json TEXT NOT NULL,
            discovery_metric_calls INTEGER,
            system_prompt TEXT,
            PRIMARY KEY (upstream_name, candidate_index)
        );

        CREATE TABLE upstream_validation_scores (
            upstream_name TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            val_index INTEGER NOT NULL,
            source_id TEXT,
            score REAL,
            PRIMARY KEY (upstream_name, candidate_index, val_index)
        );

        CREATE TABLE upstream_trace (
            upstream_name TEXT NOT NULL,
            iteration INTEGER NOT NULL,
            selected_program_candidate INTEGER,
            new_program_idx INTEGER,
            old_train_score REAL,
            new_train_score REAL,
            subsample_ids_json TEXT NOT NULL,
            PRIMARY KEY (upstream_name, iteration)
        );

        CREATE TABLE validation_comparison (
            run_id TEXT NOT NULL,
            upstream_name TEXT NOT NULL,
            val_index INTEGER NOT NULL,
            source_id TEXT,
            leaven_candidate_index INTEGER NOT NULL,
            upstream_candidate_index INTEGER NOT NULL,
            leaven_score REAL,
            upstream_score REAL,
            category TEXT NOT NULL,
            PRIMARY KEY (run_id, upstream_name, val_index)
        );

        CREATE INDEX validation_scores_source_idx ON validation_scores(source_id);
        CREATE INDEX upstream_validation_scores_source_idx ON upstream_validation_scores(source_id);
        CREATE INDEX proposal_attempts_parent_idx ON proposal_attempts(parent_index);
        CREATE INDEX case_deltas_outcome_idx ON case_deltas(split, outcome);
        """
    )


def insert_report(
    db: sqlite3.Connection,
    report_path: pathlib.Path,
    report: dict[str, Any],
    validation_sources: list[str],
) -> str:
    run = report.get("run", {})
    scores = report.get("scores", {})
    budget = report.get("budget", {})
    gepa_report = report.get("gepa_report", {})
    lm_roles = report.get("lm_roles", [])
    solver_model = next((role.get("model") for role in lm_roles if role.get("role") == "solver"), None)
    reflection_model = next((role.get("model") for role in lm_roles if role.get("role") == "reflection"), None)
    run_id = run.get("id") or report_path.parent.parent.name

    db.execute(
        """
        INSERT INTO runs VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
        )
        """,
        (
            run_id,
            str(report_path),
            run.get("run_dir"),
            report.get("proof_classification"),
            report.get("run_profile"),
            report.get("gepa_profile"),
            solver_model,
            reflection_model,
            scores.get("baseline_validation"),
            scores.get("validation"),
            scores.get("baseline_test"),
            scores.get("test"),
            budget.get("search_metric_calls_spent"),
            budget.get("final_report_metric_calls_spent"),
            budget.get("total_metric_calls"),
            gepa_report.get("best_index"),
            gepa_report.get("validation_best_index"),
            len(gepa_report.get("candidates", [])),
            len(gepa_report.get("proposal_attempts", [])),
            gepa_report.get("accepted_count"),
            gepa_report.get("full_validation_evals"),
        ),
    )

    case_to_source, source_to_val_index = source_maps(report)
    if validation_sources:
        source_to_val_index = {source_id: index for index, source_id in enumerate(validation_sources)}

    for candidate in gepa_report.get("candidates", []):
        prompt = candidate.get("system_prompt")
        candidate_index = candidate.get("index")
        db.execute(
            """
            INSERT INTO candidates VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                run_id,
                candidate_index,
                candidate.get("candidate"),
                json_text(candidate.get("parents", [])),
                candidate.get("validation_score"),
                candidate.get("discovery_metric_calls"),
                len(prompt) if isinstance(prompt, str) else None,
                candidate.get("system_prompt_source"),
                prompt,
            ),
        )
        for score_row in candidate.get("validation_subscores", []):
            case_id = score_row.get("case")
            source_id = case_to_source.get(case_id)
            db.execute(
                """
                INSERT INTO validation_scores VALUES (?, ?, ?, ?, ?, ?)
                """,
                (
                    run_id,
                    candidate_index,
                    source_to_val_index.get(source_id),
                    case_id,
                    source_id,
                    score_row.get("score"),
                ),
            )

    for attempt in gepa_report.get("proposal_attempts", []):
        child_cases = attempt.get("child_cases", [])
        child_sources = [case_to_source.get(case_id) for case_id in child_cases]
        reflection = attempt.get("reflection") or {}
        proposed = reflection.get("proposed_text")
        assistant = reflection.get("assistant_text")
        db.execute(
            """
            INSERT INTO proposal_attempts VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            """,
            (
                run_id,
                attempt.get("attempt_index"),
                attempt.get("iteration"),
                attempt.get("parent_index"),
                attempt.get("child_index"),
                int(bool(attempt.get("accepted"))),
                int(bool(attempt.get("admitted"))),
                attempt.get("parent_score"),
                attempt.get("child_score"),
                attempt.get("child_validation_score"),
                attempt.get("reflective_example_count"),
                json_text(child_cases),
                json_text(child_sources),
                reflection.get("model"),
                len(proposed) if isinstance(proposed, str) else None,
                len(assistant) if isinstance(assistant, str) else None,
                proposed,
                assistant,
            ),
        )

    for delta in report.get("case_deltas", {}).get("cases", []):
        db.execute(
            """
            INSERT INTO case_deltas VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                run_id,
                delta.get("split"),
                delta.get("case_id"),
                delta.get("source_id"),
                delta.get("baseline_score"),
                delta.get("optimized_score"),
                delta.get("score_delta"),
                delta.get("outcome"),
            ),
        )

    return str(run_id)


def load_upstream_state(run_dir: pathlib.Path, upstream_src: pathlib.Path | None) -> Any:
    if upstream_src is not None:
        sys.path.insert(0, str(upstream_src))
    from gepa.core.state import GEPAState  # type: ignore[import-not-found]

    return GEPAState.load(str(run_dir))


def candidate_prompt(candidate: dict[str, str]) -> str:
    if "current_candidate" in candidate:
        return candidate["current_candidate"]
    if "system" in candidate:
        return candidate["system"]
    if "prompt" in candidate:
        return candidate["prompt"]
    return next(iter(candidate.values()))


def insert_upstream(
    db: sqlite3.Connection,
    upstream_name: str,
    state: Any,
    validation_sources: list[str],
) -> None:
    for index, candidate in enumerate(state.program_candidates):
        scores = list(state.prog_candidate_val_subscores[index].values())
        prompt = candidate_prompt(candidate)
        parents = state.parent_program_for_candidate[index]
        discovery_calls = state.num_metric_calls_by_discovery[index]
        db.execute(
            """
            INSERT INTO upstream_candidates VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            (
                upstream_name,
                index,
                sum(scores) / len(scores) if scores else None,
                len(prompt),
                json_text(parents),
                discovery_calls,
                prompt,
            ),
        )
        for raw_val_index, score in state.prog_candidate_val_subscores[index].items():
            val_index = int(raw_val_index)
            source_id = validation_sources[val_index] if val_index < len(validation_sources) else None
            db.execute(
                """
                INSERT INTO upstream_validation_scores VALUES (?, ?, ?, ?, ?)
                """,
                (upstream_name, index, val_index, source_id, score),
            )

    for trace in state.full_program_trace:
        old_scores = trace.get("subsample_scores") or []
        new_scores = trace.get("new_subsample_scores") or []
        new_program_idx = trace.get("new_program_idx")
        db.execute(
            """
            INSERT INTO upstream_trace VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            (
                upstream_name,
                trace.get("i"),
                trace.get("selected_program_candidate"),
                int(new_program_idx) if new_program_idx is not None else None,
                sum(old_scores) / len(old_scores) if old_scores else None,
                sum(new_scores) / len(new_scores) if new_scores else None,
                json_text(trace.get("subsample_ids", [])),
            ),
        )


def insert_validation_comparison(
    db: sqlite3.Connection,
    run_id: str,
    upstream_name: str,
    leaven_candidate_index: int,
    upstream_candidate_index: int,
) -> None:
    leaven_rows = {
        row[0]: row[1]
        for row in db.execute(
            """
            SELECT val_index, score FROM validation_scores
            WHERE run_id = ? AND candidate_index = ? AND val_index IS NOT NULL
            """,
            (run_id, leaven_candidate_index),
        )
    }
    upstream_rows = {
        row[0]: (row[1], row[2])
        for row in db.execute(
            """
            SELECT val_index, source_id, score FROM upstream_validation_scores
            WHERE upstream_name = ? AND candidate_index = ?
            """,
            (upstream_name, upstream_candidate_index),
        )
    }
    for val_index, leaven_score in leaven_rows.items():
        upstream = upstream_rows.get(val_index)
        if upstream is None:
            continue
        source_id, upstream_score = upstream
        if leaven_score == 1.0 and upstream_score == 1.0:
            category = "both_correct"
        elif leaven_score == 1.0 and upstream_score == 0.0:
            category = "leaven_only"
        elif leaven_score == 0.0 and upstream_score == 1.0:
            category = "upstream_only"
        else:
            category = "both_wrong"
        db.execute(
            """
            INSERT INTO validation_comparison VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                run_id,
                upstream_name,
                val_index,
                source_id,
                leaven_candidate_index,
                upstream_candidate_index,
                leaven_score,
                upstream_score,
                category,
            ),
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=pathlib.Path, help="P8 reports/p8-aime.json path")
    parser.add_argument("--output", type=pathlib.Path, help="SQLite output path")
    parser.add_argument("--aime-cache", type=pathlib.Path, help="Materialized AIME cache JSON")
    parser.add_argument("--upstream-gepa-run-dir", type=pathlib.Path, help="Directory containing gepa_state.bin")
    parser.add_argument("--upstream-gepa-src", type=pathlib.Path, help="Path to upstream gepa/src")
    parser.add_argument("--upstream-name", default="cais", help="Label for optional upstream tables")
    args = parser.parse_args()

    report_path = args.report
    output = args.output or default_output_path(report_path)
    report = load_json(report_path)
    validation_sources = load_aime_validation_sources(args.aime_cache)

    db = connect(output)
    try:
        create_schema(db)
        db.execute("INSERT INTO metadata VALUES (?, ?)", ("report_path", str(report_path)))
        db.execute("INSERT INTO metadata VALUES (?, ?)", ("output_path", str(output)))
        if args.aime_cache is not None:
            db.execute("INSERT INTO metadata VALUES (?, ?)", ("aime_cache", str(args.aime_cache)))
        run_id = insert_report(db, report_path, report, validation_sources)

        if args.upstream_gepa_run_dir is not None:
            state = load_upstream_state(args.upstream_gepa_run_dir, args.upstream_gepa_src)
            insert_upstream(db, args.upstream_name, state, validation_sources)
            leaven_best = report.get("gepa_report", {}).get("validation_best_index")
            upstream_scores = [
                (row[0], row[1])
                for row in db.execute(
                    """
                    SELECT candidate_index, validation_score FROM upstream_candidates
                    WHERE upstream_name = ?
                    ORDER BY validation_score DESC, candidate_index ASC
                    """,
                    (args.upstream_name,),
                )
            ]
            if isinstance(leaven_best, int) and upstream_scores:
                insert_validation_comparison(db, run_id, args.upstream_name, leaven_best, upstream_scores[0][0])

        db.commit()
    finally:
        db.close()

    print(f"wrote {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
