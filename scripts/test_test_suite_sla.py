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
    def test_workspace_build_timeout_is_separate_from_runtime_sla(self) -> None:
        with mock.patch.object(
            MODULE,
            "WORKSPACE_TEST_BUILD_COMMAND",
            [sys.executable, "-c", "import time; time.sleep(10)"],
        ):
            started = time.perf_counter()
            with self.assertRaises(SystemExit):
                MODULE.discover_workspace_test_binaries(pathlib.Path.cwd(), build_timeout=0.1)

        self.assertLess(time.perf_counter() - started, 3.0)

    def test_runtime_sla_starts_after_workspace_build_discovery(self) -> None:
        with (
            mock.patch.object(MODULE, "discover_workspace_test_binaries", return_value=[]),
            mock.patch.object(MODULE, "test_commands", return_value=[]),
            mock.patch.object(
                MODULE.argparse.ArgumentParser,
                "parse_args",
                return_value=MODULE.argparse.Namespace(sla_seconds=30.0, build_timeout=None),
            ),
        ):
            self.assertEqual(MODULE.main(), 0)

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
