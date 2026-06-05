"""Check local Python SDK quality invariants that linters do not express."""

import ast
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
DOMAIN_VALUE_NAMES = {"output", "raw_output", "target", "payload", "result", "value"}
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
    ROOT / "src" / "leaven" / "artifacts" / "skill_bank.py": ROOT
    / "tests"
    / "artifacts"
    / "test_skill_bank.py",
    ROOT / "src" / "leaven" / "assessment.py": ROOT / "tests" / "test_assessment.py",
    ROOT / "src" / "leaven" / "builders" / "case.py": ROOT / "tests" / "builders" / "test_case.py",
    ROOT / "src" / "leaven" / "builders" / "agent.py": ROOT
    / "tests"
    / "builders"
    / "test_agent.py",
    ROOT / "src" / "leaven" / "builders" / "lm.py": ROOT
    / "tests"
    / "builders"
    / "test_lm.py",
    ROOT / "src" / "leaven" / "builders" / "_output_contract.py": ROOT
    / "tests"
    / "builders"
    / "test_output_contract.py",
    ROOT / "src" / "leaven" / "cases" / "__init__.py": ROOT
    / "tests"
    / "cases"
    / "test_public_surface.py",
    ROOT / "src" / "leaven" / "cases" / "csv.py": ROOT / "tests" / "cases" / "test_csv.py",
    ROOT / "src" / "leaven" / "cases" / "jsonl.py": ROOT / "tests" / "cases" / "test_jsonl.py",
    ROOT / "src" / "leaven" / "evidence.py": ROOT / "tests" / "test_evidence.py",
    ROOT / "src" / "leaven" / "evaluation_job.py": ROOT / "tests" / "test_evaluation_job.py",
    ROOT / "src" / "leaven" / "output.py": ROOT / "tests" / "test_output.py",
    ROOT / "src" / "leaven" / "output_record.py": ROOT / "tests" / "test_output_record.py",
    ROOT / "src" / "leaven" / "result.py": ROOT / "tests" / "test_result.py",
    ROOT / "src" / "leaven" / "run_inspection.py": ROOT / "tests" / "test_run_inspection.py",
    ROOT / "src" / "leaven" / "score.py": ROOT / "tests" / "test_score.py",
    ROOT / "src" / "leaven" / "scoring.py": ROOT / "tests" / "test_scoring.py",
    ROOT / "scripts" / "check_quality_contract.py": ROOT
    / "tests"
    / "scripts"
    / "test_check_quality_contract.py",
}


def main() -> None:
    failures = list(check_line_counts())
    failures.extend(check_future_annotations())
    failures.extend(check_wire_any())
    failures.extend(check_defensive_type_erasure())
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


def check_defensive_type_erasure(files: list[Path] | None = None) -> list[str]:
    failures: list[str] = []
    for path in files or project_python_files():
        if path.relative_to(ROOT).parts[:1] == ("scripts",):
            continue
        text = path.read_text(encoding="utf-8")
        failures.extend(defensive_type_erasure_failures_for_source(path, text))
    return failures


def defensive_type_erasure_failures_for_source(path: Path, source: str) -> list[str]:
    tree = ast.parse(source, filename=str(path))
    visitor = DefensiveTypeErasureVisitor(path, source)
    visitor.visit(tree)
    return visitor.failures


class DefensiveTypeErasureVisitor(ast.NodeVisitor):
    """Find Python patterns that hide a bad domain type instead of failing."""

    def __init__(self, path: Path, source: str) -> None:
        self.path = path
        self.lines = source.splitlines()
        self.failures: list[str] = []
        self.output_contract_names: set[str] = set()

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self._check_function_args(node)
        self._visit_function_body(node)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self._check_function_args(node)
        self._visit_function_body(node)

    def visit_If(self, node: ast.If) -> None:
        if self._is_strict_type_guard(node):
            for statement in [*node.body, *node.orelse]:
                self.visit(statement)
            return
        self.generic_visit(node)

    def visit_IfExp(self, node: ast.IfExp) -> None:
        if self._is_str_else_str_fallback(node):
            self._add(
                node,
                "LEAVEN003",
                "uses isinstance(..., str) else str(...) defensive fallback",
            )
        self.generic_visit(node)

    def visit_Call(self, node: ast.Call) -> None:
        if self._is_banned_str_coercion(node):
            self._add(node, "LEAVEN002", "uses str(...) to coerce a domain value")
        if self._is_banned_isinstance_check(node):
            self._add(
                node,
                "LEAVEN004",
                "branches on isinstance(output, ...) instead of typed output",
            )
        if self._is_banned_get_probe(node):
            self._add(node, "LEAVEN005", "uses .get(...) on an unparsed domain value")
        if self._is_getattr_probe(node):
            self._add(node, "LEAVEN006", "uses getattr(...) to probe a domain value")
        self.generic_visit(node)

    def _check_function_args(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        for arg in node.args.args:
            if arg.arg != "output":
                continue
            if isinstance(arg.annotation, ast.Name) and arg.annotation.id == "object":
                self._add(arg, "LEAVEN001", "widens callback output to object")

    def _visit_function_body(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        for arg in [*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs]:
            if arg.annotation is not None and self._is_output_contract_annotation(arg.annotation):
                self.output_contract_names.add(arg.arg)
        for statement in node.body:
            self.visit(statement)
        self.output_contract_names.clear()

    def _is_banned_str_coercion(self, node: ast.Call) -> bool:
        if not isinstance(node.func, ast.Name) or node.func.id != "str":
            return False
        if len(node.args) != 1:
            return False
        return self._is_domain_value(node.args[0])

    def _is_banned_isinstance_check(self, node: ast.AST) -> bool:
        if not isinstance(node, ast.Call):
            return False
        if not isinstance(node.func, ast.Name) or node.func.id != "isinstance":
            return False
        if not node.args:
            return False
        checked = node.args[0]
        if isinstance(checked, ast.Name) and checked.id in self.output_contract_names:
            return False
        return isinstance(checked, ast.Name) and checked.id in {"output", "raw_output"}

    def _is_strict_type_guard(self, node: ast.If) -> bool:
        test = node.test
        if isinstance(test, ast.UnaryOp) and isinstance(test.op, ast.Not):
            test = test.operand
        return (
            isinstance(test, ast.Call)
            and self._is_banned_isinstance_check(test)
            and self._if_body_raises(node)
        )

    def _is_str_else_str_fallback(self, node: ast.IfExp) -> bool:
        checked_name = self._isinstance_str_checked_name(node.test)
        if checked_name is None:
            return False
        return (
            isinstance(node.body, ast.Name)
            and node.body.id == checked_name
            and self._is_str_call_for_name(node.orelse, checked_name)
        )

    def _isinstance_str_checked_name(self, node: ast.AST) -> str | None:
        if not isinstance(node, ast.Call):
            return None
        if not isinstance(node.func, ast.Name) or node.func.id != "isinstance":
            return None
        if len(node.args) != 2:
            return None
        checked, type_expr = node.args
        if not isinstance(checked, ast.Name):
            return None
        if not isinstance(type_expr, ast.Name) or type_expr.id != "str":
            return None
        return checked.id

    def _is_str_call_for_name(self, node: ast.AST, name: str) -> bool:
        return (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "str"
            and len(node.args) == 1
            and isinstance(node.args[0], ast.Name)
            and node.args[0].id == name
        )

    def _is_banned_get_probe(self, node: ast.Call) -> bool:
        if not isinstance(node.func, ast.Attribute) or node.func.attr != "get":
            return False
        owner = node.func.value
        return not self._is_os_environ(owner)

    def _is_getattr_probe(self, node: ast.Call) -> bool:
        return (
            isinstance(node.func, ast.Name)
            and node.func.id == "getattr"
            and bool(node.args)
            and self._is_domain_value(node.args[0])
        )

    def _is_domain_value(self, node: ast.AST) -> bool:
        return self._root_name(node) in DOMAIN_VALUE_NAMES

    def _root_name(self, node: ast.AST) -> str | None:
        match node:
            case ast.Name(id=name):
                return name
            case ast.Attribute(value=value):
                return self._root_name(value)
            case ast.Subscript(value=value):
                return self._root_name(value)
            case ast.BoolOp(values=[*_, value]):
                return self._root_name(value)
            case ast.IfExp(body=body, orelse=orelse):
                return self._root_name(body) or self._root_name(orelse)
            case _:
                return None

    def _is_case_target_or_empty(self, node: ast.AST) -> bool:
        if not isinstance(node, ast.BoolOp) or not isinstance(node.op, ast.Or):
            return False
        if len(node.values) != 2:
            return False
        lhs, rhs = node.values
        return (
            isinstance(lhs, ast.Attribute)
            and lhs.attr == "target"
            and isinstance(lhs.value, ast.Name)
            and lhs.value.id == "case"
            and isinstance(rhs, ast.Dict)
            and not rhs.keys
        )

    def _is_output_contract_annotation(self, annotation: ast.AST) -> bool:
        match annotation:
            case ast.Name(id="OutputContract" | "JsonSchemaOutput" | "JsonSchemaValueOutput"):
                return True
            case ast.Subscript(value=value):
                return self._is_output_contract_annotation(value)
            case ast.BinOp(left=left, right=right, op=ast.BitOr()):
                return self._is_output_contract_annotation(
                    left
                ) or self._is_output_contract_annotation(right)
            case _:
                return False

    def _if_body_raises(self, node: ast.If) -> bool:
        return bool(node.body) and all(isinstance(statement, ast.Raise) for statement in node.body)

    def _is_os_environ(self, node: ast.AST) -> bool:
        return (
            isinstance(node, ast.Attribute)
            and node.attr == "environ"
            and isinstance(node.value, ast.Name)
            and node.value.id == "os"
        )

    def _add(self, node: ast.AST, code: str, message: str) -> None:
        lineno = getattr(node, "lineno", 0)
        if self._line_has_noqa(lineno, code):
            return
        self.failures.append(f"{relative(self.path)}:{lineno}: {code} {message}")

    def _line_has_noqa(self, lineno: int, code: str) -> bool:
        if lineno <= 0 or lineno > len(self.lines):
            return False
        marker = "# noqa:"
        comment = self.lines[lineno - 1]
        marker_index = comment.find(marker)
        if marker_index == -1:
            return False
        suffix = comment[marker_index + len(marker) :].strip()
        if " -- " not in suffix:
            return False
        code_text, justification = suffix.split(" -- ", 1)
        if not justification.strip():
            return False
        codes = {part.strip() for part in code_text.split(",")}
        return code in codes


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
