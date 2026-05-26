#!/usr/bin/env python3
"""Tests for the canonical suite SLA runner helpers."""

from __future__ import annotations

import importlib.util
import json
import pathlib
import sys
import tempfile
import time
import unittest
from unittest import mock


SCRIPT = pathlib.Path(__file__).with_name("test-suite-sla.py")
SPEC = importlib.util.spec_from_file_location("test_suite_sla", SCRIPT)
assert SPEC is not None
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def json_line(payload: object) -> str:
    return json.dumps(payload)


class DoctestDetectionTests(unittest.TestCase):
    def test_text_fences_and_string_literals_do_not_require_doctest_harness(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = pathlib.Path(raw)
            source = root / "src" / "lib.rs"
            source.parent.mkdir()
            source.write_text(
                'const PROMPT: &str = "```\\nnot a doc comment\\n```";\n'
                "/// Operator transcript:\n"
                "/// ```text\n"
                "/// not rust\n"
                "/// ```\n"
                "pub fn documented() {}\n",
                encoding="utf-8",
            )

            self.assertFalse(MODULE.package_has_rust_doctest(root))

    def test_rust_doc_fences_require_doctest_harness(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = pathlib.Path(raw)
            source = root / "src" / "lib.rs"
            source.parent.mkdir()
            source.write_text(
                "/// Example:\n"
                "/// ```rust\n"
                "/// let value = 1 + 1;\n"
                "/// assert_eq!(value, 2);\n"
                "/// ```\n"
                "pub fn documented() {}\n",
                encoding="utf-8",
            )

            self.assertTrue(MODULE.package_has_rust_doctest(root))


class SuiteDeadlineTests(unittest.TestCase):
    def test_cargo_test_command_preserves_workspace_excludes(self) -> None:
        command = MODULE.WORKSPACE_TEST_DISCOVERY_COMMAND

        self.assertEqual(command[:4], ["cargo", "test", "--workspace", "--all-targets"])
        self.assertIn("--no-run", command)
        self.assertIn("--message-format=json", command)
        self.assertIn("--workspace", command)
        self.assertIn("--all-targets", command)
        self.assertIn("--exclude", command)
        self.assertIn("p8_aime_gepa", command)

    def test_workspace_test_binary_discovery_reads_executable_artifacts_once(self) -> None:
        first = pathlib.Path("/tmp/test-one")
        second = pathlib.Path("/tmp/test-two")
        payload = "\n".join(
            [
                json_line({"reason": "compiler-message"}),
                json_line({
                    "reason": "compiler-artifact",
                    "package_id": "pkg 1",
                    "executable": str(first),
                }),
                json_line({
                    "reason": "compiler-artifact",
                    "package_id": "pkg 1",
                    "executable": str(first),
                }),
                json_line({"reason": "compiler-artifact", "executable": None}),
                "not json",
                json_line({
                    "reason": "compiler-artifact",
                    "package_id": "pkg 2",
                    "executable": str(second),
                }),
            ]
        )
        roots = {"pkg 1": pathlib.Path("/repo/one"), "pkg 2": pathlib.Path("/repo/two")}
        with (
            mock.patch.object(MODULE, "workspace_package_roots", return_value=roots),
            mock.patch.object(MODULE, "run_capture_with_timeout") as run,
        ):
            run.return_value = (0, payload)

            binaries = MODULE.discover_workspace_test_binaries(pathlib.Path.cwd())

        self.assertEqual(binaries, [(first, roots["pkg 1"]), (second, roots["pkg 2"])])

    def test_workspace_discovery_timeout_is_separate_from_runtime_sla(self) -> None:
        with mock.patch.object(
            MODULE,
            "WORKSPACE_TEST_DISCOVERY_COMMAND",
            [sys.executable, "-c", "import time; time.sleep(10)"],
        ):
            started = time.perf_counter()
            with self.assertRaises(SystemExit) as exit_context:
                MODULE.discover_workspace_test_binaries(pathlib.Path.cwd(), build_timeout=0.1)

        self.assertEqual(exit_context.exception.code, 1)
        self.assertLess(time.perf_counter() - started, 3.0)

    def test_default_build_discovery_timeout_is_a_generous_hang_guard(self) -> None:
        self.assertEqual(MODULE.DEFAULT_BUILD_DISCOVERY_TIMEOUT_SECONDS, 300.0)

    def test_runtime_target_starts_after_workspace_build_discovery(self) -> None:
        with (
            mock.patch.object(
                MODULE,
                "discover_workspace_test_binaries",
                return_value=[(pathlib.Path("/tmp/test-one"), pathlib.Path.cwd())],
            ),
            mock.patch.object(MODULE, "test_commands", return_value=[]),
            mock.patch.object(MODULE, "run_workspace_test_binaries", return_value=0),
            mock.patch.object(
                MODULE.argparse.ArgumentParser,
                "parse_args",
                return_value=MODULE.argparse.Namespace(
                    warn_seconds=30.0,
                    timeout_seconds=600.0,
                    build_timeout=MODULE.DEFAULT_BUILD_DISCOVERY_TIMEOUT_SECONDS,
                ),
            ),
        ):
            self.assertEqual(MODULE.main(), 0)
            MODULE.run_workspace_test_binaries.assert_called_once_with(
                [(pathlib.Path("/tmp/test-one"), pathlib.Path.cwd())],
                mock.ANY,
            )

    def test_doctest_prewarm_runs_before_runtime_deadline_starts(self) -> None:
        with mock.patch.object(MODULE.subprocess, "run") as run:
            run.return_value.returncode = 0
            started = time.perf_counter()
            result = MODULE.prewarm_commands(
                [
                    (
                        "slow doctest compile",
                        [sys.executable, "-c", "import time; time.sleep(0.2)"],
                    )
                ],
                pathlib.Path.cwd(),
            )

        self.assertEqual(result, 0)
        self.assertLess(time.perf_counter() - started, 0.1)
        run.assert_called_once()

    def test_command_deadline_kills_slow_subprocess(self) -> None:
        started = time.perf_counter()
        result = MODULE.run_with_deadline(
            "slow test command",
            [
                sys.executable,
                "-c",
                "import time; time.sleep(10)",
            ],
            pathlib.Path.cwd(),
            started + 0.1,
        )

        self.assertEqual(result, 1)
        self.assertLess(time.perf_counter() - started, 3.0)


if __name__ == "__main__":
    unittest.main()
