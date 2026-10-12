import json
import tempfile
import unittest
from types import SimpleNamespace
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from okf_agent.cli import main
from okf_agent.onboarding import SKILL_NAME, install_codex_skill


class OnboardingTests(unittest.TestCase):
    def test_init_can_opt_in_to_workflow_profiles(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                result = main(["init", directory, "--profiles", "sdd,tdd"])

            payload = json.loads(output.getvalue())
            self.assertEqual(result, 0)
            self.assertEqual(
                json.loads((Path(directory) / ".okf/workflows/config.json").read_text())["enabled_profiles"],
                ["sdd", "tdd"],
            )
            self.assertEqual(payload["workflow_profiles"]["enabled_profiles"], ["sdd", "tdd"])

    def test_init_without_profile_option_keeps_policy_optional(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                result = main(["init", directory])

            self.assertEqual(result, 0)
            self.assertFalse((Path(directory) / ".okf/workflows/config.json").exists())

    def test_workflow_show_and_resolve_return_json_decisions(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                result = main(["workflow", "show", directory])
            policy = json.loads(output.getvalue())
            self.assertEqual(result, 0)
            self.assertEqual(policy["schema_version"], 1)

            output = StringIO()
            with redirect_stdout(output):
                result = main([
                    "workflow", "resolve", directory, "--type", "feature", "--risk", "strict"
                ])
            decision = json.loads(output.getvalue())
            self.assertEqual(result, 0)
            self.assertEqual(decision["primary_type"], "feature")
            self.assertEqual(decision["rigor"], "strict")
            self.assertIn("Broader verification", decision["phases"])

    def test_workflow_resolve_reports_single_agent_gtdd_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                result = main([
                    "workflow", "resolve", directory, "--type", "feature", "--stateful"
                ])
            decision = json.loads(output.getvalue())

            self.assertEqual(result, 0)
            self.assertEqual(decision["execution"], "single-agent")
            self.assertEqual(decision["fallback"], "sequential-adversarial-review")

    def test_installs_portable_auto_trigger_skill(self):
        with tempfile.TemporaryDirectory() as directory:
            target = install_codex_skill(Path(directory))
            skill = (target / "SKILL.md").read_text(encoding="utf-8")
            self.assertEqual(target.name, SKILL_NAME)
            self.assertIn("even when the user does not mention OKF", skill)
            self.assertIn("okf-agent init <root>", skill)

    def test_cli_installs_to_requested_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                self.assertEqual(main(["install", "--skills-dir", directory]), 0)
            payload = json.loads(output.getvalue())
            self.assertEqual(Path(payload["skill"]).parent, Path(directory))

    def test_run_no_launch_reports_context_delivery_limits(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with redirect_stdout(output):
                result = main(["run", "codex", directory, "--no-launch"])

            payload = json.loads(output.getvalue())
            self.assertEqual(result, 0)
            self.assertIn("not injected", payload["context_delivery"])

    def test_run_missing_client_returns_actionable_error(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with patch("okf_agent.cli.shutil.which", return_value=None), redirect_stdout(output):
                result = main(["run", "missing-client", directory])

            payload = json.loads(output.getvalue())
            self.assertEqual(result, 2)
            self.assertIn("not on PATH", payload["error"])

    def test_run_returns_child_process_exit_code(self):
        with tempfile.TemporaryDirectory() as directory:
            output = StringIO()
            with patch("okf_agent.cli.shutil.which", return_value="fake-client"), \
                 patch("okf_agent.cli.subprocess.run", return_value=SimpleNamespace(returncode=17)), \
                 redirect_stdout(output):
                result = main(["run", "fake-client", directory])

            self.assertEqual(result, 17)
            payload = json.loads(output.getvalue())
            self.assertIn("not injected", payload["context_delivery"])

    def test_docs_help_exposes_build_check_and_serve_commands(self):
        output = StringIO()
        with redirect_stdout(output), self.assertRaises(SystemExit) as exit_info:
            main(["docs", "--help"])

        self.assertEqual(exit_info.exception.code, 0)
        self.assertIn("build", output.getvalue())
        self.assertIn("check", output.getvalue())
        self.assertIn("serve", output.getvalue())


if __name__ == "__main__":
    unittest.main()
