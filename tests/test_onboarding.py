import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from okf_agent.cli import main
from okf_agent.onboarding import SKILL_NAME, install_codex_skill


class OnboardingTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
