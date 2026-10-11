from __future__ import annotations

import importlib
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from okf_agent.cli import main
from okf_agent.discovery import build_index
from okf_agent.knowledge import GENERATED_END, GENERATED_START, initialize
from okf_agent.validation import validate


def docs_api(test: unittest.TestCase, name: str):
    """Load a planned docs API while reporting an implementation gap as RED."""
    try:
        module = importlib.import_module("okf_agent.docs")
    except ModuleNotFoundError:
        test.fail("okf_agent.docs is missing; add the docs API after these tests fail")
    test.assertTrue(hasattr(module, name), f"okf_agent.docs.{name}() is required")
    return getattr(module, name)


def make_repo(root: Path) -> None:
    (root / ".git").mkdir(parents=True, exist_ok=True)
    initialize(root)
    (root / "pyproject.toml").write_text("[project]\nname = 'sample'\n", encoding="utf-8")
    (root / "src space").mkdir()
    (root / "src space" / "service.py").write_text(
        "# DO NOT DOCUMENT AS PRODUCT BEHAVIOR\n"
        "import json\n\n"
        "def delete_user(user_id):\n    return user_id\n",
        encoding="utf-8",
    )
    overview = root / ".okf" / "application" / "overview.md"
    overview.write_text(
        overview.read_text(encoding="utf-8") + "\nOperators review the processing queue here.\n",
        encoding="utf-8",
    )


def run_cli(argv: list[str]) -> tuple[int, str, str]:
    stdout = StringIO()
    stderr = StringIO()
    try:
        with redirect_stdout(stdout), redirect_stderr(stderr):
            code = main(argv)
    except SystemExit as exc:
        code = int(exc.code or 0)
    return code, stdout.getvalue(), stderr.getvalue()


class DocumentationRenderingTests(unittest.TestCase):
    def test_rendered_pages_link_curated_overview_and_indexed_code(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)
            build_index(root)

            build_docs = docs_api(self, "build_docs")
            result = build_docs(root)
            pages = root / "docs" / "reference"
            index = (pages / "index.md").read_text(encoding="utf-8")
            code_map = (pages / "code-map.md").read_text(encoding="utf-8")

            self.assertIn(".okf/application/overview.md", index)
            self.assertIn("Operators review the processing queue here.", index)
            self.assertIn("src space/service.py", code_map)
            self.assertIn("delete_user", code_map)
            self.assertTrue(result["updated"])

    def test_docs_without_okf_omit_missing_overview_link_and_encode_source_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".git").mkdir()
            (root / "src space").mkdir()
            (root / "src space" / "service.py").write_text("def serve():\n    pass\n", encoding="utf-8")

            docs_api(self, "build_docs")(root)
            index = (root / "docs" / "reference" / "index.md").read_text(encoding="utf-8")
            code_map = (root / "docs" / "reference" / "code-map.md").read_text(encoding="utf-8")

            self.assertNotIn(".okf/application/overview.md", index)
            self.assertIn("No curated application overview", index)
            self.assertIn("src%20space/service.py", code_map)

    def test_rendered_pages_do_not_infer_behavior(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)

            docs_api(self, "build_docs")(root)
            rendered = "\n".join(
                path.read_text(encoding="utf-8")
                for path in (root / "docs" / "reference").glob("*.md")
            )

            self.assertIn("delete_user", rendered)
            self.assertNotIn("DO NOT DOCUMENT AS PRODUCT BEHAVIOR", rendered)
            self.assertNotIn("This service deletes user accounts", rendered)

    def test_rendered_pages_handle_empty_index(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            build_index(root)

            docs_api(self, "build_docs")(root)
            code_map = (root / "docs" / "reference" / "code-map.md").read_text(encoding="utf-8")

            self.assertRegex(code_map, r"(?i)(no indexed (files|symbols)|none found|empty index)")


class DocumentationBuildTests(unittest.TestCase):
    def test_build_docs_creates_only_reference_pages(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)

            docs_api(self, "build_docs")(root)

            self.assertEqual(
                sorted(path.relative_to(root).as_posix() for path in (root / "docs" / "reference").rglob("*")),
                ["docs/reference/code-map.md", "docs/reference/index.md"],
            )

    def test_build_docs_preserves_curated_text_outside_markers(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)
            output = root / "docs" / "reference"
            output.mkdir(parents=True)
            page = output / "code-map.md"
            page.write_text(
                "# Team code guide\n\nBefore generated section.\n\n"
                f"{GENERATED_START}\nold generated content\n{GENERATED_END}\n\n"
                "After generated section.\n",
                encoding="utf-8",
            )

            docs_api(self, "build_docs")(root)
            updated = page.read_text(encoding="utf-8")

            self.assertTrue(updated.startswith("# Team code guide\n\nBefore generated section."))
            self.assertIn("After generated section.", updated)
            self.assertIn(f"{GENERATED_START}\n", updated)
            self.assertIn(f"\n{GENERATED_END}", updated)
            self.assertNotIn("old generated content", updated)

    def test_build_docs_leaves_unrelated_docs_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)
            unrelated = root / "docs" / "superpowers" / "plans" / "example.md"
            unrelated.parent.mkdir(parents=True)
            unrelated.write_text("# Do not change\n\nCurated plan.\n", encoding="utf-8")
            before = unrelated.read_bytes()

            docs_api(self, "build_docs")(root)

            self.assertEqual(unrelated.read_bytes(), before)

    def test_build_docs_is_byte_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)
            build = docs_api(self, "build_docs")

            first = build(root)
            before = {
                path.relative_to(root).as_posix(): path.read_bytes()
                for path in (root / "docs" / "reference").glob("*.md")
            }
            second = build(root)
            after = {
                path.relative_to(root).as_posix(): path.read_bytes()
                for path in (root / "docs" / "reference").glob("*.md")
            }

            self.assertEqual(first["updated"], ["docs/reference/code-map.md", "docs/reference/index.md"])
            self.assertEqual(second["updated"], [])
            self.assertEqual(after, before)

    def test_build_docs_rejects_page_symlink_outside_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            root = workspace / "repo"
            root.mkdir()
            make_repo(root)
            output = root / "docs" / "reference"
            output.mkdir(parents=True)
            outside = workspace / "outside.md"
            outside.write_text("Keep this file unchanged.\n", encoding="utf-8")
            page_link = output / "index.md"
            simulated_link = False
            try:
                page_link.symlink_to(outside)
            except (OSError, NotImplementedError) as exc:
                simulated_link = True
                page_link.write_text("Keep this page unchanged.\n", encoding="utf-8")

            if simulated_link:
                with patch.object(Path, "is_symlink", autospec=True, side_effect=lambda path: path == page_link):
                    with self.assertRaisesRegex(ValueError, "symlink"):
                        docs_api(self, "build_docs")(root)
                self.assertEqual(page_link.read_text(encoding="utf-8"), "Keep this page unchanged.\n")
            else:
                with self.assertRaisesRegex(ValueError, "symlink"):
                    docs_api(self, "build_docs")(root)
                self.assertEqual(outside.read_text(encoding="utf-8"), "Keep this file unchanged.\n")



class DocumentationCheckTests(unittest.TestCase):
    def test_check_docs_detects_missing_and_stale_pages(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)
            build = docs_api(self, "build_docs")
            check = docs_api(self, "check_docs")

            missing = check(root)
            self.assertFalse(missing["ok"])
            self.assertEqual(
                missing["stale"],
                ["docs/reference/code-map.md", "docs/reference/index.md"],
            )

            build(root)
            self.assertTrue(check(root)["ok"])
            source = root / "src space" / "service.py"
            source.write_text(source.read_text(encoding="utf-8") + "\ndef new_symbol():\n    pass\n", encoding="utf-8")
            stale = check(root)

            self.assertFalse(stale["ok"])
            self.assertIn("docs/reference/code-map.md", stale["stale"])
            self.assertIn("okf-agent docs build", " ".join(stale["errors"]))

    def test_check_docs_does_not_write_docs_or_persistent_index(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)
            build = docs_api(self, "build_docs")
            check = docs_api(self, "check_docs")
            build(root)
            docs_before = {
                path.relative_to(root).as_posix(): path.read_bytes()
                for path in (root / "docs" / "reference").glob("*.md")
            }
            index = root / ".okf-index" / "knowledge.sqlite3"
            index_before = index.read_bytes()
            source = root / "src space" / "service.py"
            source.write_text(source.read_text(encoding="utf-8") + "\ndef changed():\n    pass\n", encoding="utf-8")

            check(root)

            docs_after = {
                path.relative_to(root).as_posix(): path.read_bytes()
                for path in (root / "docs" / "reference").glob("*.md")
            }
            self.assertEqual(docs_after, docs_before)
            self.assertEqual(index.read_bytes(), index_before)


class DocumentationCliTests(unittest.TestCase):
    def test_docs_cli_build_and_check_exit_codes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            make_repo(root)

            build_code, build_stdout, _ = run_cli(["docs", "build", str(root)])
            self.assertEqual(build_code, 0)
            self.assertEqual(json.loads(build_stdout)["updated"], ["docs/reference/code-map.md", "docs/reference/index.md"])

            fresh_code, fresh_stdout, _ = run_cli(["docs", "check", str(root)])
            self.assertEqual(fresh_code, 0)
            self.assertTrue(json.loads(fresh_stdout)["ok"])

            (root / "src space" / "service.py").write_text("def changed():\n    pass\n", encoding="utf-8")
            stale_code, stale_stdout, _ = run_cli(["docs", "check", str(root)])
            self.assertNotEqual(stale_code, 0)
            self.assertFalse(json.loads(stale_stdout)["ok"])

    def test_docs_cli_resolves_output_within_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "repo"
            root.mkdir()
            make_repo(root)

            code, _, _ = run_cli(["docs", "build", str(root), "--output", "human-docs"])
            self.assertEqual(code, 0)
            self.assertTrue((root / "human-docs" / "index.md").exists())

            outside = Path(directory) / "escaped-docs"
            escape_code, _, escape_stderr = run_cli(["docs", "build", str(root), "--output", "../escaped-docs"])
            self.assertNotEqual(escape_code, 0)
            self.assertFalse(outside.exists())
            self.assertIn("output", escape_stderr.lower())

    def test_docs_help_lists_subcommands(self):
        code, stdout, _ = run_cli(["docs", "--help"])
        self.assertEqual(code, 0)
        self.assertIn("build", stdout)
        self.assertIn("check", stdout)


class DocumentationOptInTests(unittest.TestCase):
    def test_lifecycle_commands_do_not_generate_docs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".git" / "hooks").mkdir(parents=True)

            init_code, _, _ = run_cli(["init", str(root)])
            self.assertEqual(init_code, 0)
            update_code, _, _ = run_cli(["update", str(root)])
            self.assertEqual(update_code, 0)
            with patch("okf_agent.cli.start_watcher", return_value=None):
                run_code, _, _ = run_cli(["run", "codex", str(root), "--no-launch"])
            self.assertEqual(run_code, 0)

            self.assertFalse((root / "docs" / "reference").exists())
            for hook in (root / ".git" / "hooks").iterdir():
                self.assertNotIn("docs build", hook.read_text(encoding="utf-8"))

    def test_validate_rejects_malformed_generated_markers_and_broken_links(self):
        invalid_pages = {
            "unclosed-start": f"{GENERATED_START}\nbody\n",
            "unmatched-end": f"body\n{GENERATED_END}\n",
            "duplicate-start": f"{GENERATED_START}\none\n{GENERATED_START}\ntwo\n{GENERATED_END}\n",
            "reversed-markers": f"{GENERATED_END}\nbody\n{GENERATED_START}\n",
            "broken-link": f"{GENERATED_START}\n[Missing](../../does-not-exist.md)\n{GENERATED_END}\n",
        }
        for label, content in invalid_pages.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                initialize(root)
                page = root / "docs" / "reference" / "index.md"
                page.parent.mkdir(parents=True)
                page.write_text(content, encoding="utf-8")

                result = validate(root)

                self.assertFalse(result["ok"], label)
                self.assertTrue(any("docs/reference/index.md" in error for error in result["errors"]), label)


if __name__ == "__main__":
    unittest.main()
