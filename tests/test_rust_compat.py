from __future__ import annotations

import tomllib
import unittest
import importlib
from pathlib import Path


class RustCompatibilityTests(unittest.TestCase):
    def test_python_package_does_not_register_a_python_cli_entrypoint(self) -> None:
        project = tomllib.loads(Path("pyproject.toml").read_text(encoding="utf-8"))["project"]
        self.assertNotIn("scripts", project)

    def test_existing_python_package_modules_remain_importable(self) -> None:
        for name in ("bundle", "cli", "discovery", "docs", "lifecycle", "mcp", "workflows"):
            with self.subTest(module=name):
                self.assertIsNotNone(importlib.import_module(f"okf_agent.{name}"))


if __name__ == "__main__":
    unittest.main()
