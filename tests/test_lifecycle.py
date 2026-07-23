import tempfile
import unittest
from pathlib import Path

from okf_agent.discovery import build_index, search
from okf_agent.enforcement import install
from okf_agent.knowledge import initialize
from okf_agent.lifecycle import refresh_knowledge
from okf_agent.validation import validate


class LifecycleTests(unittest.TestCase):
    def test_initialize_index_update_preserves_curated_content(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            source = root / "app.py"; source.write_text("def hello():\n    return 'hi'\n")
            overview = root / ".okf" / "application" / "overview.md"
            overview.write_text(overview.read_text() + "\nA human sentence.")
            result = build_index(root)
            refresh_knowledge(root, ["app.py"])
            self.assertEqual(result["files"], 1)
            self.assertTrue(search(root, "hello"))
            self.assertIn("A human sentence.", overview.read_text())
            self.assertTrue(validate(root)["ok"])

    def test_hooks_are_safe_without_git(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); initialize(root)
            artifacts = install(root)
            self.assertEqual(artifacts, [root / "AGENTS.md"])


if __name__ == "__main__": unittest.main()
