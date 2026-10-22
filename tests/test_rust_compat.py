from __future__ import annotations

import contextlib
import io
import json
import os
import unittest
import importlib
from unittest.mock import patch

from okf_agent import cli


class Completed:
    def __init__(self, stdout: str = "", returncode: int = 0):
        self.stdout = stdout
        self.returncode = returncode


class RustCompatibilityTests(unittest.TestCase):
    def test_python_remains_default_without_rust_environment(self) -> None:
        with patch.dict(os.environ, {}, clear=True), patch("okf_agent.cli.subprocess.run") as run:
            self.assertIsNone(cli._try_rust_delegation(["index", "."]))
            run.assert_not_called()

    def test_only_advertised_completed_command_families_are_delegated(self) -> None:
        cases = {
            ("index", "."): "index",
            ("workflow", "resolve", "."): "workflow resolve",
            ("bundle", "search", "term"): "bundle search",
            ("docs", "check", "."): "docs check",
            ("init", "."): None,
            ("run", "codex", "."): None,
            ("skills", "doctor"): None,
            ("mcp",): None,
        }
        for arguments, expected in cases.items():
            with self.subTest(arguments=arguments):
                self.assertEqual(cli._delegated_capability(list(arguments)), expected)

    def test_opted_in_delegation_checks_version_and_capability_then_forwards(self) -> None:
        calls: list[list[str]] = []

        def fake_run(command: list[str], **_kwargs: object) -> Completed:
            calls.append(command)
            if command[-1] == "--version":
                return Completed("okf-agent-rs 0.1.1\n")
            if command[-1] == "capabilities":
                return Completed(json.dumps({"implemented": ["index"]}))
            return Completed(returncode=17)

        with patch.dict(os.environ, {"OKF_AGENT_USE_RUST": "1", "OKF_AGENT_RUST_BIN": "native"}), patch(
            "okf_agent.cli.subprocess.run", side_effect=fake_run
        ):
            self.assertEqual(cli._try_rust_delegation(["index", "."]), 17)
        self.assertEqual(calls, [["native", "--version"], ["native", "capabilities"], ["native", "index", "."]])

    def test_old_or_missing_native_binary_falls_back_to_python_with_diagnostic(self) -> None:
        with patch.dict(os.environ, {"OKF_AGENT_USE_RUST": "true", "OKF_AGENT_RUST_BIN": "native"}), patch(
            "okf_agent.cli.subprocess.run", return_value=Completed("okf-agent-rs 0.1.0\n")
        ), contextlib.redirect_stderr(io.StringIO()) as stderr:
            self.assertIsNone(cli._try_rust_delegation(["index", "."]))
        self.assertIn("using Python", stderr.getvalue())

    def test_missing_native_executable_falls_back_to_python(self) -> None:
        with patch.dict(os.environ, {"OKF_AGENT_USE_RUST": "1", "OKF_AGENT_RUST_BIN": "missing"}), patch(
            "okf_agent.cli.subprocess.run", side_effect=FileNotFoundError("missing executable")
        ), contextlib.redirect_stderr(io.StringIO()) as stderr:
            self.assertIsNone(cli._try_rust_delegation(["index", "."]))
        self.assertIn("using Python", stderr.getvalue())

    def test_existing_python_package_modules_remain_importable(self) -> None:
        for name in ("bundle", "cli", "discovery", "docs", "lifecycle", "mcp", "workflows"):
            with self.subTest(module=name):
                self.assertIsNotNone(importlib.import_module(f"okf_agent.{name}"))


if __name__ == "__main__":
    unittest.main()
