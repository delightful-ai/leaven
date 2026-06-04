"""Check local Python SDK quality invariants that linters do not express."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAX_PYTHON_LINES = 650

CHECKED_ROOTS = (
    ROOT / "src" / "leaven",
    ROOT / "tests",
    ROOT / "examples",
    ROOT / "codegen",
    ROOT / "scripts",
)

SKIPPED_PARTS = {
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "__pycache__",
    "build",
}

TYPED_BOUNDARY_ROOTS = (
    ROOT / "src" / "leaven" / "_seam" / "_wire",
    ROOT / "src" / "leaven" / "_seam_worker",
)
MIRRORED_TESTS = {
    ROOT / "src" / "leaven" / "_runs" / "rust_export.py": ROOT
    / "tests"
    / "_runs"
    / "test_rust_export.py",
    ROOT / "src" / "leaven" / "_seam" / "plans.py": ROOT / "tests" / "_seam" / "test_plans.py",
    ROOT / "src" / "leaven" / "_seam" / "_wire" / "codec.py": ROOT
    / "tests"
    / "_seam"
    / "_wire"
    / "test_codec.py",
    ROOT / "src" / "leaven" / "_seam" / "_wire" / "methods.py": ROOT
    / "tests"
    / "_seam"
    / "_wire"
    / "test_methods.py",
    ROOT / "src" / "leaven" / "_seam" / "_wire" / "payloads.py": ROOT
    / "tests"
    / "_seam"
    / "_wire"
    / "test_payloads.py",
    ROOT / "src" / "leaven" / "_seam_worker" / "callbacks.py": ROOT
    / "tests"
    / "_seam_worker"
    / "test_callbacks.py",
    ROOT / "src" / "leaven" / "_seam_worker" / "protocol.py": ROOT
    / "tests"
    / "_seam_worker"
    / "test_protocol.py",
    ROOT / "src" / "leaven" / "builders" / "case.py": ROOT / "tests" / "builders" / "test_case.py",
    ROOT / "src" / "leaven" / "run_inspection.py": ROOT / "tests" / "test_run_inspection.py",
}


def main() -> None:
    failures = list(check_line_counts())
    failures.extend(check_future_annotations())
    failures.extend(check_wire_any())
    failures.extend(check_mirrored_tests())
    if failures:
        joined = "\n".join(f"- {failure}" for failure in failures)
        raise SystemExit(f"Python SDK quality contract failed:\n{joined}")
    print("python sdk quality contract ok")


def project_python_files() -> list[Path]:
    files: list[Path] = []
    for root in CHECKED_ROOTS:
        if not root.exists():
            continue
        files.extend(
            path
            for path in root.rglob("*.py")
            if SKIPPED_PARTS.isdisjoint(path.relative_to(ROOT).parts)
        )
    return sorted(files)


def check_line_counts() -> list[str]:
    failures: list[str] = []
    for path in project_python_files():
        lines = path.read_text(encoding="utf-8").splitlines()
        if len(lines) > MAX_PYTHON_LINES:
            failures.append(f"{relative(path)} has {len(lines)} lines; max is {MAX_PYTHON_LINES}")
    return failures


def check_future_annotations() -> list[str]:
    failures: list[str] = []
    banned = "from __future__ import " + "annotations"
    for path in project_python_files():
        text = path.read_text(encoding="utf-8")
        if banned in text:
            failures.append(f"{relative(path)} imports future annotations")
    return failures


def check_wire_any() -> list[str]:
    failures: list[str] = []
    for root in TYPED_BOUNDARY_ROOTS:
        for path in sorted(root.rglob("*.py")):
            if SKIPPED_PARTS.isdisjoint(path.relative_to(ROOT).parts):
                text = path.read_text(encoding="utf-8")
                if "Any" in text:
                    failures.append(f"{relative(path)} contains `Any` in a typed seam boundary")
    return failures


def check_mirrored_tests() -> list[str]:
    failures: list[str] = []
    for source, test in sorted(MIRRORED_TESTS.items()):
        if not source.exists():
            failures.append(f"mirrored source missing: {relative(source)}")
        if not test.exists():
            failures.append(f"{relative(source)} must be covered by {relative(test)}")
    return failures


def relative(path: Path) -> str:
    return str(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
