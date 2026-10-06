import tempfile
import json
import unittest
import subprocess
import threading
import time
import re
from unittest.mock import patch
from pathlib import Path

from okf_agent.discovery import build_index, detect, search
from okf_agent.docs import build_docs, check_docs
from okf_agent.enforcement import install, install_report
from okf_agent.knowledge import initialize, replace_generated
from okf_agent.lifecycle import git_changed, refresh_knowledge
from okf_agent.validation import validate
from okf_agent.watcher import _snapshot, start
from okf_agent.paths import find_root


class LifecycleTests(unittest.TestCase):
    def test_find_root_ignores_empty_git_marker_above_project(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory)
            (parent / ".git").mkdir()
            project = parent / "project"
            project.mkdir()

            self.assertEqual(find_root(project), project.resolve())

    def test_initialize_without_profiles_does_not_create_workflow_config(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            initialize(root)

            self.assertFalse((root / ".okf" / "workflows" / "config.json").exists())

    def test_initialize_installs_explicit_workflow_profiles(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            initialize(root, profiles=("sdd",))

            config = root / ".okf" / "workflows" / "config.json"
            self.assertTrue(config.is_file())
            self.assertEqual(json.loads(config.read_text())["enabled_profiles"], ["sdd"])

    def test_detected_language_manifests_are_also_indexed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifests = {
                "pyproject.toml": "[project]\nname='sample'\n",
                "package.json": '{"name":"sample"}',
                "go.mod": "module example.org/sample\n",
                "Cargo.toml": "[package]\nname='sample'\n",
                "pom.xml": "<project><artifactId>sample</artifactId></project>",
            }
            for name, content in manifests.items():
                (root / name).write_text(content)

            indexed = build_index(root)
            facts = detect(root)

            self.assertEqual(set(facts["manifests"]), set(manifests))
            self.assertEqual(indexed["files"], len(manifests))
            self.assertTrue(search(root, "example.org/sample"))

    def test_supported_source_extensions_are_indexed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sources = {
                "module.py": "PYTHON_TOKEN = 1\n",
                "module.ts": "const TYPESCRIPT_TOKEN = 1;\n",
                "module.go": "package main\nconst GOTOKEN = 1\n",
                "module.rs": "const RUSTTOKEN: i32 = 1;\n",
                "Module.java": "class Module { int JAVATOKEN = 1; }\n",
            }
            for name, content in sources.items():
                (root / name).write_text(content)

            result = build_index(root)

            self.assertEqual(result["files"], len(sources))
            for marker in ("PYTHON_TOKEN", "TYPESCRIPT_TOKEN", "GOTOKEN", "RUSTTOKEN", "JAVATOKEN"):
                with self.subTest(marker=marker):
                    self.assertTrue(search(root, marker))

    def test_watcher_snapshot_ignores_files_removed_during_stat(self):
        path = Path("vanishing.py")
        with patch("okf_agent.watcher.files", return_value=[path]), \
             patch.object(Path, "exists", return_value=True), \
             patch.object(Path, "stat", side_effect=FileNotFoundError):
            self.assertEqual(_snapshot(Path(".")), {})

    def test_watcher_reports_index_error_and_keeps_running(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "app.py"
            source.write_text("VALUE = 1\n")
            failed = threading.Event()

            def fail_index(_root):
                failed.set()
                raise OSError("index unavailable")

            with patch("okf_agent.watcher.build_index", side_effect=fail_index), self.assertLogs("okf_agent.watcher", level="ERROR"):
                watcher = start(root, interval=0.01)
                try:
                    time.sleep(0.05)
                    source.write_text("VALUE = 2\n")
                    self.assertTrue(failed.wait(1))
                    self.assertTrue(watcher.is_alive)
                finally:
                    watcher.stop()

    def test_watcher_handle_stops_and_joins_thread(self):
        with tempfile.TemporaryDirectory() as directory:
            watcher = start(Path(directory), interval=0.01)

            watcher.stop()

            self.assertFalse(watcher.is_alive)

    def test_validate_rejects_malformed_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            (root / ".okf" / "manifest.yaml").write_text("schema_version: nope\n")

            result = validate(root)

            self.assertFalse(result["ok"])
            self.assertTrue(any("manifest.yaml" in error for error in result["errors"]))

    def test_validate_warns_when_index_is_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)

            result = validate(root)

            self.assertTrue(any("absent" in warning.lower() for warning in result["warnings"]))

    def test_validate_accepts_absent_optional_workflow_profiles(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)

            result = validate(root)

            self.assertTrue(result["ok"])
            self.assertFalse(any("workflow" in error.lower() for error in result["errors"]))

    def test_validate_reports_missing_enabled_workflow_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root, profiles=("sdd",))
            profile = root / ".okf" / "workflows" / "profiles" / "sdd.md"
            profile.unlink()

            result = validate(root)

            self.assertTrue(any("profiles/sdd.md" in error for error in result["errors"]))

    def test_validate_reports_invalid_workflow_config_with_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root, profiles=("sdd",))
            config = root / ".okf" / "workflows" / "config.json"
            config.write_text('{"schema_version": 1, "typo": true}', encoding="utf-8")

            result = validate(root)

            self.assertTrue(any(".okf/workflows/config.json" in error for error in result["errors"]))
            self.assertTrue(any("typo" in error for error in result["errors"]))

    def test_validate_rejects_unmatched_generated_markers(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            overview = root / ".okf" / "application" / "overview.md"
            overview.write_text(overview.read_text().replace("<!-- okf:generated:end -->", ""))

            result = validate(root)

            self.assertFalse(result["ok"])
            self.assertTrue(any("generated block" in error.lower() for error in result["errors"]))

    def test_validate_reports_broken_relative_markdown_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            overview = root / ".okf" / "application" / "overview.md"
            overview.write_text(overview.read_text() + "\n[Missing](missing.md)\n")

            result = validate(root)

            self.assertTrue(any("missing.md" in error for error in result["errors"]))

    def test_validate_warns_when_index_is_stale(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            source = root / "app.py"
            source.write_text("VALUE = 1\n")
            build_index(root)
            source.write_text("VALUE = 2\n")

            result = validate(root)

            self.assertTrue(any("stale" in warning.lower() for warning in result["warnings"]))

    def test_generated_pre_commit_hook_does_not_hide_update_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hooks = root / ".git" / "hooks"
            hooks.mkdir(parents=True)

            install(root)

            hook = (hooks / "pre-commit").read_text(encoding="utf-8")
            self.assertIn("update .", hook)
            self.assertNotIn("|| exit 0", hook)

    def test_install_preserves_custom_pre_commit_hook(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            hooks = root / ".git" / "hooks"
            hooks.mkdir(parents=True)
            custom = "#!/bin/sh\necho team-hook\n"
            (hooks / "pre-commit").write_text(custom, encoding="utf-8")

            install(root)

            self.assertEqual((hooks / "pre-commit").read_text(encoding="utf-8"), custom)

    def test_workflow_adapter_routing_is_idempotent_and_preserves_curated_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            instructions = root / "AGENTS.md"
            before = "# Team instructions\n\nKeep this text.\n"
            instructions.write_text(before, encoding="utf-8")
            initialize(root, profiles=("sdd",))

            first = install_report(root)
            installed = instructions.read_text(encoding="utf-8")

            self.assertIn(before, installed)
            self.assertIn("<!-- okf:workflow:start -->", installed)
            self.assertIn(".okf/workflows/active.md", installed)
            self.assertIn(instructions, first.installed)

            second = install_report(root)

            self.assertEqual(instructions.read_text(encoding="utf-8"), installed)
            self.assertNotIn(instructions, second.installed)

    def test_malformed_workflow_adapter_markers_are_reported_without_rewrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            instructions = root / "CLAUDE.md"
            original = (
                "# Curated Claude rules\n\n## OKF Repository Protocol\n"
                "Preserve this protocol.\n<!-- okf:workflow:start -->\n"
            )
            instructions.write_text(original, encoding="utf-8")
            initialize(root, profiles=("sdd",))

            result = install_report(root)

            self.assertEqual(instructions.read_text(encoding="utf-8"), original)
            self.assertTrue(any("CLAUDE.md" in conflict for conflict in result.conflicts))

    def test_new_cursor_rule_receives_workflow_routing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root, profiles=("sdd",))

            install_report(root)

            cursor_rule = root / ".cursor" / "rules" / "okf.mdc"
            self.assertIn("<!-- okf:workflow:start -->", cursor_rule.read_text(encoding="utf-8"))

    def test_workflow_adapter_symlink_is_preserved_and_reported(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            root = Path(directory)
            external = Path(outside) / "instructions.md"
            original = "# Shared instructions\n"
            external.write_text(original, encoding="utf-8")
            (root / "AGENTS.md").symlink_to(external)
            initialize(root, profiles=("sdd",))

            result = install_report(root)

            self.assertEqual(external.read_text(encoding="utf-8"), original)
            self.assertTrue(any("AGENTS.md" in conflict for conflict in result.conflicts))

    def test_adapter_parent_symlink_cannot_redirect_writes_outside_root(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            root = Path(directory)
            external = Path(outside)
            (root / ".claude").symlink_to(external, target_is_directory=True)
            (root / ".cursor").symlink_to(external, target_is_directory=True)
            initialize(root, profiles=("sdd",))

            result = install_report(root)

            self.assertEqual(list(external.iterdir()), [])
            self.assertTrue(any(".claude" in conflict for conflict in result.conflicts))
            self.assertTrue(any(".cursor" in conflict for conflict in result.conflicts))

    def test_repeated_refresh_does_not_rewrite_unchanged_generated_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            (root / "app.py").write_text("VALUE = 1\n")
            refresh_knowledge(root, ["app.py"])
            generated = [
                root / ".okf" / "application" / "overview.md",
                root / ".okf" / "architecture" / "index.md",
                root / ".okf" / "changes" / "current.md",
            ]
            before = {path: (path.read_text(encoding="utf-8"), path.stat().st_mtime_ns) for path in generated}

            refresh_knowledge(root, ["app.py"])

            after = {path: (path.read_text(encoding="utf-8"), path.stat().st_mtime_ns) for path in generated}
            self.assertEqual(after, before)

    def test_replace_generated_keeps_curated_text_outside_malformed_block(self):
        with tempfile.TemporaryDirectory() as directory:
            document = Path(directory) / "notes.md"
            document.write_text(
                "Before.\n<!-- okf:generated:start -->\nold\n"
                "<!-- okf:generated:start -->\nolder\n<!-- okf:generated:end -->\n"
                "Curated after inner marker.\n<!-- okf:generated:end -->\nAfter.\n"
            )

            replace_generated(document, "fresh facts")

            result = document.read_text()
            self.assertIn("Before.", result)
            self.assertIn("fresh facts", result)
            self.assertIn("Curated after inner marker.", result)
            self.assertIn("After.", result)
            self.assertNotIn("older", result)

    def test_update_rebuilds_existing_documentation_after_source_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            source = root / "app.py"
            source.write_text("VALUE = 1\n")
            build_docs(root)
            source.write_text("VALUE = 2\n")

            refresh_knowledge(root, ["app.py"])

            self.assertTrue(check_docs(root)["ok"])

    def test_update_indexes_new_source_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            (root / "new_module.py").write_text("def fresh_symbol():\n    return 1\n")

            refresh_knowledge(root)

            self.assertTrue(search(root, "fresh_symbol"))

    def test_git_changed_includes_staged_unstaged_and_untracked_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=root, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=root, check=True)
            (root / "staged.py").write_text("old\n")
            (root / "unstaged.py").write_text("old\n")
            subprocess.run(["git", "add", "staged.py", "unstaged.py"], cwd=root, check=True)
            subprocess.run(["git", "commit", "-qm", "initial"], cwd=root, check=True)
            (root / "staged.py").write_text("staged change\n")
            subprocess.run(["git", "add", "staged.py"], cwd=root, check=True)
            (root / "unstaged.py").write_text("unstaged change\n")
            (root / "new.py").write_text("new file\n")

            self.assertEqual(set(git_changed(root)), {"staged.py", "unstaged.py", "new.py"})

    def test_git_changed_handles_non_git_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(git_changed(Path(directory)), [])

    def test_git_changed_includes_staged_files_before_first_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            (root / "staged.py").write_text("staged\n")
            subprocess.run(["git", "add", "staged.py"], cwd=root, check=True)
            (root / "untracked.py").write_text("untracked\n")

            self.assertEqual(set(git_changed(root)), {"staged.py", "untracked.py"})

    def test_validate_reports_malformed_markdown_links_with_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize(root)
            overview = root / ".okf" / "application" / "overview.md"
            overview.write_text(overview.read_text() + "\n[Broken](target\n")

            result = validate(root)

            self.assertTrue(any("malformed markdown link" in error.lower() and re.search(r":\d+ ->", error) for error in result["errors"]))

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
