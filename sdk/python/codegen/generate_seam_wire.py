"""Generate private Python seam wire metadata from a Rust public-seam export."""

import argparse
import json
import subprocess
from pathlib import Path

from expression_codegen import render_expressions
from method_codegen import MethodCodegenRow, render_methods
from payload_codegen import PayloadMethodRow, render_payloads
from ref_codegen import render_refs
from result_codegen import ReceiptExpectation, ResultMethodRow, render_results

REPO_ROOT = Path(__file__).resolve().parents[3]
WIRE_DIR = REPO_ROOT / "sdk/python/src/leaven/_seam/_wire"
METHODS_OUTPUT = WIRE_DIR / "methods.py"
EXPRESSIONS_OUTPUT = WIRE_DIR / "expressions.py"
PAYLOADS_OUTPUT = WIRE_DIR / "payloads.py"
REFS_OUTPUT = WIRE_DIR / "refs.py"
RESULTS_OUTPUT = WIRE_DIR / "results.py"


class MethodRow(PayloadMethodRow):
    """One exported locked public-seam method row."""

    method: str
    params_schema: str
    result_schema: str
    required_action: str
    params_schema_fingerprint: str
    result_schema_fingerprint: str
    produces_receipt: bool
    primary_kinds: list[str]
    receipt_expectation: ReceiptExpectation


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if generated output differs")
    args = parser.parse_args()

    generated = generated_files()
    if args.check:
        stale = [path for path, content in generated.items() if path.read_text() != content]
        if stale:
            raise SystemExit(f"generated seam wire files are stale: {stale!r}")
        return
    for path, content in generated.items():
        path.write_text(content, encoding="utf-8")


def generated_files() -> dict[Path, str]:
    rows = export_profile_rows()
    return {
        METHODS_OUTPUT: render_methods(method_rows(rows)),
        REFS_OUTPUT: render_refs(),
        EXPRESSIONS_OUTPUT: render_expressions(),
        PAYLOADS_OUTPUT: render_payloads(rows),
        RESULTS_OUTPUT: render_results(result_rows(rows)),
    }


def export_profile_rows() -> list[MethodRow]:
    output = subprocess.check_output(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "leaven-cli",
            "--",
            "seam",
            "profile",
            "--root",
            str(REPO_ROOT),
        ],
        cwd=REPO_ROOT,
        text=True,
    )
    document = json.loads(output)
    if document["schema_version"] != "leaven.seam_profile_export.v1":
        raise ValueError("unexpected seam profile export schema_version")
    return [
        {
            "method": row["method"],
            "params_schema": row["params_schema"],
            "result_schema": row["result_schema"],
            "required_action": row["required_action"],
            "params_schema_fingerprint": row["params_schema_fingerprint"],
            "result_schema_fingerprint": row["result_schema_fingerprint"],
            "produces_receipt": row["produces_receipt"],
            "primary_kinds": row["primary_kinds"],
            "receipt_expectation": row["receipt_expectation"],
        }
        for row in document["extension_methods"]
    ]


def result_rows(rows: list[MethodRow]) -> list[ResultMethodRow]:
    return [
        ResultMethodRow(
            {
                "method": row["method"],
                "primary_kinds": row["primary_kinds"],
                "receipt_expectation": row["receipt_expectation"],
            }
        )
        for row in rows
    ]


def method_rows(rows: list[MethodRow]) -> list[MethodCodegenRow]:
    return [
        {
            "method": row["method"],
            "params_schema": row["params_schema"],
            "result_schema": row["result_schema"],
            "required_action": row["required_action"],
            "params_schema_fingerprint": row["params_schema_fingerprint"],
            "result_schema_fingerprint": row["result_schema_fingerprint"],
            "produces_receipt": row["produces_receipt"],
        }
        for row in rows
    ]



if __name__ == "__main__":
    main()
