from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from okf_agent.bundle import export_bundle, graph_query, index_bundle, search_bundle, validate_bundle


class BundleProfileTests(unittest.TestCase):
    def setUp(self):
        # The Windows workspace sandbox permits test scratch only inside the workspace.
        self.temp = tempfile.TemporaryDirectory(dir=Path.cwd())
        self.root = Path(self.temp.name) / "course-bundle"
        self.index = Path(self.temp.name) / "external-index"
        self.root.mkdir()

    def tearDown(self):
        self.temp.cleanup()

    def write(self, path: str, text: str):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def test_gitless_bundle_indexes_searches_and_traverses_typed_links(self):
        self.write("index.md", "# Course\n\n- [Course](course.md)\n")
        self.write("course.md", "---\ntype: Course\naliases: [syllabus]\ncustom_field: keep-me\nrelations:\n  - target: /lessons/one.md\n    relation: includes\n---\n# Course\n[Lesson](/lessons/one.md)\n")
        self.write("lessons/one.md", "---\ntype: Lesson\ntitle: First Lesson\nresource: https://example.test/lesson\nsources:\n - resource: /sources/handbook.md\n---\n# Learning\nSearchable curriculum phrase.\n[Course](/course.md)\n")
        self.write("sources/handbook.md", "---\ntype: Source Material\n---\nReference text.\n")

        indexed = index_bundle(self.root, self.index)
        exported = export_bundle(self.index)
        edges = graph_query(self.index, "course.md", 1)["edges"]
        chain = graph_query(self.index, "sources/handbook.md", 2)

        self.assertTrue(indexed["conformant"])
        self.assertEqual(len(search_bundle(self.index, "curriculum")), 1)
        self.assertEqual(edges[0]["relation"], "includes")
        self.assertEqual({node["path"] for node in chain["nodes"]}, {"sources/handbook.md", "lessons/one.md", "course.md"})
        self.assertEqual(len(chain["edges"]), 2)
        self.assertEqual(chain["edges"][0]["relation"], "provenance")
        self.assertEqual(exported["profile_contract"]["okf_version"], "0.2")
        self.assertEqual([item["document"] for item in exported["provenance"]], ["lessons/one.md"])
        self.assertFalse((self.root / ".git").exists())

    def test_unknown_type_and_fields_round_trip_across_reindex_after_manual_edit(self):
        self.write("notes/lesson.md", "---\ntype: Local Teaching Artifact\ncustom_field: keep-this\naliases: [my lesson]\n---\nOriginal body.\n")
        index_bundle(self.root, self.index)
        source = self.root / "notes/lesson.md"
        manual = "---\ntype: Local Teaching Artifact\ncustom_field: keep-this\naliases: [my lesson]\n---\nManual body edit.\n"
        manual_bytes = manual.replace("\n", "\r\n").encode("utf-8")
        source.write_bytes(manual_bytes)

        result = index_bundle(self.root, self.index)
        node = export_bundle(self.index)["nodes"][0]

        self.assertTrue(result["conformant"])
        self.assertEqual(source.read_bytes(), manual_bytes)
        self.assertEqual(node["frontmatter"]["custom_field"], "keep-this")
        self.assertEqual(node["body"], "Manual body edit.\r\n")
        self.assertEqual(node["revision"], 2)

    def test_broken_links_are_diagnostics_not_conformance_errors_and_ambiguity_is_reported(self):
        self.write("start.md", "---\ntype: Guide\n---\n[Ambiguous](lesson.md) [Missing](nope.md)\n\n```md\n[Fake](not-a-link.md)\n```\n")
        self.write("a/lesson.md", "---\ntype: Lesson\n---\nA\n")
        self.write("b/lesson.md", "---\ntype: Lesson\n---\nB\n")

        result = validate_bundle(self.root, self.index)
        messages = [item["message"] for item in result["diagnostics"]]

        self.assertTrue(result["ok"])
        self.assertTrue(any("Ambiguous link" in message for message in messages))
        self.assertTrue(any("Broken link" in message for message in messages))
        self.assertFalse(any("not-a-link" in message for message in messages))

    def test_link_parser_handles_escaped_spaces_anchors_embeds_and_wikilinks(self):
        self.write("index.md", "# Index\n")
        self.write("lesson one.md", "---\ntype: Lesson\n---\nBody\n")
        self.write("page.md", "---\ntype: Page\n---\n[lesson](lesson\\ one.md#part) ![chart](lesson%20one.md) [[lesson one.md|alias]]\n")

        index_bundle(self.root, self.index, wikilinks=True)
        result = graph_query(self.index, "page.md", 1)

        self.assertEqual(len(result["edges"]), 3)
        self.assertTrue(all(edge["target"] == "lesson one.md" for edge in result["edges"]))

    def test_cli_export_is_json_and_does_not_initialize_repository_adapters(self):
        from contextlib import redirect_stdout
        from io import StringIO
        from okf_agent.cli import main

        self.write("concept.md", "---\ntype: Note\n---\nText\n")
        output = StringIO()
        with redirect_stdout(output):
            self.assertEqual(main(["bundle", "index", "--root", str(self.root), "--index-root", str(self.index)]), 0)
        self.assertEqual(json.loads(output.getvalue())["documents"], 1)
        self.assertFalse((self.root / ".okf").exists())
        self.assertFalse((self.root / "AGENTS.md").exists())

    def test_export_contains_latest_validation_and_bundle_mcp_exposes_same_contract(self):
        from types import SimpleNamespace
        from unittest.mock import patch

        self.write("note.md", "---\ntype: Unknown-but-valid\n---\nBundle API.\n")
        index_bundle(self.root, self.index)
        export = export_bundle(self.index)
        self.assertTrue(export["validation"]["conformant"])

        class FakeServer:
            def __init__(self, _name):
                self.tools = {}

            def tool(self, name):
                return lambda function: self.tools.setdefault(name, function)

        with patch.dict("sys.modules", {"fastmcp": SimpleNamespace(FastMCP=FakeServer)}):
            from okf_agent.mcp import create_bundle_server
            server = create_bundle_server(self.root, self.index)
        self.assertIn("okf.bundle_export", server.tools)
        self.assertIn("okf.bundle_validate", server.tools)
        self.assertEqual(server.tools["okf.bundle_export"](), export)

    def test_okf_reserved_files_and_optional_frontmatter_families_validate(self):
        self.write("index.md", "---\nokf_version: '0.2'\n---\n# Bundle\n")
        self.write("log.md", "# Update Log\n\n## 2026-10-12\n- **Update**: Added a concept.\n\n## 2026-10-01\n- **Creation**: Started the bundle.\n")
        self.write("concept.md", "---\ntype: Organization-Specific Type\nverified: {by: 'human:reviewer', at: '2026-10-12T00:00:00Z'}\ncustom: {nested: [preserved, values]}\n---\nBody.\n")

        result = validate_bundle(self.root, self.index)
        node = export_bundle(self.index)["nodes"][0]
        self.assertTrue(result["ok"])
        self.assertEqual(node["frontmatter"]["type"], "Organization-Specific Type")
        self.assertEqual(node["frontmatter"]["custom"]["nested"], ["preserved", "values"])

        self.write("log.md", "# Update Log\n\n## 2026-10-01\n- Old.\n\n## 2026-10-12\n- New.\n")
        result = validate_bundle(self.root, self.index)
        self.assertFalse(result["ok"])
        self.assertTrue(any("newest first" in d["message"] for d in result["errors"]))


if __name__ == "__main__":
    unittest.main()
