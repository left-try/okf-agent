import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from okf_agent.cli import main
from okf_agent.docs import build_docs, check_docs
from okf_agent.knowledge import initialize
from okf_agent.validation import validate


class DocsTests(unittest.TestCase):
    def test_build_creates_searchable_wiki_and_source_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)

            result = build_docs(root)
            output = Path(result["output"])

            self.assertTrue((output / "index.md").exists())
            self.assertTrue((output / "repository.md").exists())
            self.assertTrue((output / "source" / ".okf" / "application" / "overview.md").exists())
            self.assertIn("source/.okf/application/overview.md", (output / "index.md").read_text())
            self.assertTrue(check_docs(root)["ok"])

    def test_build_is_byte_repeatable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)

            build_docs(root)
            output = root / ".okf-index" / "site"
            before = {p.relative_to(output).as_posix(): p.read_bytes() for p in output.rglob("*") if p.is_file()}
            build_docs(root)
            after = {p.relative_to(output).as_posix(): p.read_bytes() for p in output.rglob("*") if p.is_file()}

            self.assertEqual(after, before)

    def test_check_detects_changed_source_since_last_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            build_docs(root)
            overview = root / ".okf" / "application" / "overview.md"
            overview.write_text(overview.read_text() + "\nnew documentation\n")

            result = check_docs(root)

            self.assertFalse(result["ok"])
            self.assertTrue(any("stale" in error.lower() for error in result["errors"]))

    def test_check_detects_changed_code_since_last_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            code = root / "app.py"
            code.write_text("VALUE = 1\n")
            build_docs(root)
            code.write_text("VALUE = 2\n")

            result = check_docs(root)

            self.assertFalse(result["ok"])
            self.assertTrue(any("stale" in error.lower() for error in result["errors"]))

    def test_cli_check_returns_nonzero_for_stale_docs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            output = StringIO()
            with redirect_stdout(output):
                self.assertEqual(main(["docs", "build", directory]), 0)
                overview = root / ".okf" / "application" / "overview.md"
                overview.write_text(overview.read_text() + "\nnew content\n")
                result = main(["docs", "check", directory])

            self.assertEqual(result, 1)

    def test_init_builds_local_wiki_automatically(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                result = main(["init", directory])

            self.assertEqual(result, 0)
            self.assertTrue((Path(directory) / ".okf-index" / "site" / "index.md").exists())

    def test_validate_rejects_stale_built_wiki(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            build_docs(root)
            source = root / "app.py"
            source.write_text("VALUE = 1\n")

            result = validate(root)

            self.assertFalse(result["ok"])
            self.assertTrue(any("stale" in error.lower() for error in result["errors"]))

    def test_validate_rejects_modified_generated_page(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            build_docs(root)
            (root / ".okf-index" / "site" / "index.md").write_text("edited\n")

            result = validate(root)

            self.assertFalse(result["ok"])
            self.assertTrue(any("changed" in error.lower() for error in result["errors"]))



if __name__ == "__main__":
    unittest.main()
