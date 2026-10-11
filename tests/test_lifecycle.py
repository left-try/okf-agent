import tempfile
import unittest
from pathlib import Path

from okf_agent import discovery
from okf_agent.discovery import build_index, search
from okf_agent.enforcement import install
from okf_agent.knowledge import initialize
from okf_agent.lifecycle import refresh_knowledge
from okf_agent.validation import validate


class LifecycleTests(unittest.TestCase):
    def test_indexed_content_returns_sorted_documents_symbols_and_edges(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "zeta.py").write_text("import alpha\ndef zebra():\n    pass\n", encoding="utf-8")
            (root / "alpha.py").write_text("class Apple:\n    pass\n", encoding="utf-8")
            build_index(root)

            self.assertTrue(
                hasattr(discovery, "indexed_content"),
                "discovery.indexed_content() must expose indexed rows for documentation generation",
            )
            rows = discovery.indexed_content(root)

            self.assertEqual([item["path"] for item in rows["documents"]], ["alpha.py", "zeta.py"])
            self.assertEqual(
                [(item["name"], item["path"]) for item in rows["symbols"]],
                [("Apple", "alpha.py"), ("zebra", "zeta.py")],
            )
            self.assertEqual(rows["edges"], [{"source": "zeta.py", "target": "alpha", "kind": "import"}])

    def test_indexed_content_missing_index_is_read_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            self.assertTrue(
                hasattr(discovery, "indexed_content"),
                "discovery.indexed_content() must expose indexed rows for documentation generation",
            )
            with self.assertRaisesRegex(FileNotFoundError, "index"):
                discovery.indexed_content(root)

            self.assertFalse((root / ".okf-index").exists())

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

    def test_adapters_preserve_existing_instructions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            existing = root / "AGENTS.md"
            existing.write_text("# Team guidance\n", encoding="utf-8")
            initialize(root)
            install(root)
            content = existing.read_text(encoding="utf-8")
            self.assertIn("# Team guidance", content)
            self.assertIn("## OKF Repository Protocol", content)
            install(root)
            self.assertEqual(content, existing.read_text(encoding="utf-8"))
    def test_hooks_are_safe_without_git(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); initialize(root)
            artifacts = install(root)
            self.assertIn(root / "AGENTS.md", artifacts)
            self.assertIn(root / "CLAUDE.md", artifacts)
            self.assertIn(root / "GEMINI.md", artifacts)
            self.assertTrue((root / ".cursor" / "rules" / "okf.mdc").exists())


if __name__ == "__main__": unittest.main()
