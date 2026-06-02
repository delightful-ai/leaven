#!/usr/bin/env python3
"""Tests for the canonical suite SLA runner helpers."""

from __future__ import annotations

import importlib.util
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
    def test_nextest_build_command_preserves_workspace_excludes(self) -> None:
        command = MODULE.NEXTEST_BUILD_COMMAND

        self.assertEqual(command[:5], ["cargo", "nextest", "run", "--workspace", "--all-targets"])
        self.assertIn("--no-run", command)
        self.assertIn("--workspace", command)
        self.assertIn("--all-targets", command)
        self.assertIn("--exclude", command)
        self.assertIn("p8_aime_gepa", command)

    def test_nextest_run_command_preserves_workspace_excludes(self) -> None:
        command = MODULE.NEXTEST_RUN_COMMAND

        self.assertEqual(command[:5], ["cargo", "nextest", "run", "--workspace", "--all-targets"])
        self.assertNotIn("--no-run", command)
        self.assertIn("--exclude", command)
        self.assertIn("p8_aime_gepa", command)

    def test_workspace_discovery_timeout_is_separate_from_runtime_sla(self) -> None:
        with mock.patch.object(
            MODULE,
            "NEXTEST_BUILD_COMMAND",
            [sys.executable, "-c", "import time; time.sleep(10)"],
        ):
            started = time.perf_counter()
            result = MODULE.build_workspace_tests(pathlib.Path.cwd(), build_timeout=0.1)

        self.assertEqual(result, 1)
        self.assertLess(time.perf_counter() - started, 3.0)

    def test_default_build_discovery_timeout_is_a_generous_hang_guard(self) -> None:
        self.assertEqual(MODULE.DEFAULT_BUILD_DISCOVERY_TIMEOUT_SECONDS, 300.0)

    def test_missing_nextest_fails_with_actionable_install_hint(self) -> None:
        with mock.patch.object(MODULE.shutil, "which", return_value=None):
            with self.assertRaises(SystemExit) as exit_context:
                MODULE.ensure_nextest_available()

        self.assertIn("cargo install cargo-nextest", str(exit_context.exception))

    def test_runtime_target_starts_after_workspace_build_discovery(self) -> None:
        with (
            mock.patch.object(
                MODULE,
                "ensure_nextest_available",
            ),
            mock.patch.object(MODULE, "build_workspace_tests", return_value=0),
            mock.patch.object(MODULE, "test_commands", return_value=[]),
            mock.patch.object(MODULE, "run_workspace_libtests", return_value=0),
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
            MODULE.build_workspace_tests.assert_called_once_with(pathlib.Path.cwd(), 300.0)
            MODULE.run_workspace_libtests.assert_called_once_with(mock.ANY, pathlib.Path.cwd())

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
