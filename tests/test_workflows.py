import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from contextlib import redirect_stdout
from io import StringIO
from unittest.mock import patch

from okf_agent.workflows import (
    install_profiles,
    WorkflowAssessment,
    WorkflowCapabilities,
    WorkflowConfigError,
    load_policy,
    resolve_workflow,
)


class WorkflowResolverTests(unittest.TestCase):
    def resolve(self, assessment, capabilities=None):
        with tempfile.TemporaryDirectory() as directory:
            return resolve_workflow(load_policy(Path(directory)), assessment, capabilities)

    def test_defaults_cover_every_task_type(self):
        expected_methods = {
            "feature": ("sdd", "tdd"), "bugfix": ("tdd",), "hotfix": ("tdd",),
            "docs": (), "tests": (), "refactor": (), "maintenance": (),
        }
        for task_type, methods in expected_methods.items():
            with self.subTest(task_type=task_type):
                decision = self.resolve(WorkflowAssessment(primary_type=task_type))
                self.assertEqual(decision.primary_type, task_type)
                self.assertEqual(decision.effective_methods, methods)

    def test_secondary_types_keep_one_primary_type(self):
        decision = self.resolve(WorkflowAssessment(primary_type="bugfix", secondary_types=("tests", "docs")))
        self.assertEqual(decision.primary_type, "bugfix")
        self.assertEqual(decision.secondary_types, ("tests", "docs"))

    def test_ambiguous_feature_adds_specification_method(self):
        decision = self.resolve(WorkflowAssessment(primary_type="feature", ambiguous=True))
        self.assertEqual(decision.effective_methods, ("sdd", "tdd"))
        self.assertIn("Specify behavior", decision.phases)

    def test_strict_risk_adds_contract_and_broader_verification(self):
        decision = self.resolve(WorkflowAssessment(primary_type="docs", risk="strict"))
        self.assertEqual(decision.rigor, "strict")
        self.assertIn("Specify behavior", decision.phases)
        self.assertIn("Broader verification", decision.phases)

    def test_strict_contract_requirement_survives_profile_subset(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / ".okf" / "workflows" / "config.json"
            config.parent.mkdir(parents=True)
            config.write_text(json.dumps({"schema_version": 1, "enabled_profiles": ["tdd"]}))
            decision = resolve_workflow(
                load_policy(root), WorkflowAssessment(primary_type="feature", risk="strict")
            )
        self.assertIn("sdd", decision.effective_methods)
        self.assertIn("Specify behavior", decision.phases)

    def test_explicit_method_override_survives_profile_subset(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / ".okf" / "workflows" / "config.json"
            config.parent.mkdir(parents=True)
            config.write_text(json.dumps({"schema_version": 1, "enabled_profiles": ["sdd"]}))
            decision = resolve_workflow(
                load_policy(root),
                WorkflowAssessment(primary_type="feature", requested_methods=("tdd",)),
            )
        self.assertIn("tdd", decision.effective_methods)

    def test_high_impact_assessment_cannot_resolve_to_light(self):
        decision = self.resolve(
            WorkflowAssessment(primary_type="feature", risk="light", high_impact=True)
        )
        self.assertTrue(decision.high_impact)
        self.assertEqual(decision.rigor, "strict")
        self.assertIn("Broader verification", decision.phases)

    def test_same_assessment_and_policy_produce_same_decision(self):
        assessment = WorkflowAssessment(primary_type="bugfix", risk="standard")
        with tempfile.TemporaryDirectory() as directory:
            policy = load_policy(Path(directory))
            self.assertEqual(
                resolve_workflow(policy, assessment),
                resolve_workflow(policy, assessment),
            )

    def test_hotfix_does_not_waive_verification(self):
        decision = self.resolve(WorkflowAssessment(primary_type="hotfix", risk="light"))
        self.assertNotEqual(decision.rigor, "light")
        self.assertIn("Establish verification evidence", decision.phases)

    def test_behavioral_docs_can_request_tests(self):
        decision = self.resolve(WorkflowAssessment(primary_type="docs", behavioral=True, requested_methods=("tdd",)))
        self.assertIn("tdd", decision.effective_methods)

    def test_behavioral_docs_select_tdd_without_hiding_behind_category(self):
        decision = self.resolve(WorkflowAssessment(primary_type="docs", behavioral=True))
        self.assertIn("tdd", decision.effective_methods)

    def test_method_order_is_deduplicated(self):
        decision = self.resolve(WorkflowAssessment(primary_type="feature", requested_methods=("tdd", "sdd", "tdd")))
        self.assertEqual(decision.effective_methods, ("sdd", "tdd"))
        self.assertEqual(len(decision.phases), len(set(decision.phases)))

    def test_gtdd_without_isolated_roles_uses_named_single_agent_fallback(self):
        decision = self.resolve(WorkflowAssessment(primary_type="feature", independent_challenge=True))
        self.assertEqual(decision.execution, "single-agent")
        self.assertIn("independent-roles", decision.missing_capabilities)
        self.assertEqual(decision.fallback, "sequential-adversarial-review")
        self.assertIn("Single-agent adversarial review", decision.phases)

    def test_gtdd_requires_both_material_need_and_capabilities(self):
        capabilities = WorkflowCapabilities(independent_roles=True, enforceable_handoffs=True)
        decision = self.resolve(WorkflowAssessment(primary_type="feature", independent_challenge=True), capabilities)
        self.assertEqual(decision.execution, "independent-roles")
        self.assertIn("gtdd", decision.effective_methods)

    def test_gtdd_profile_must_be_enabled_even_when_roles_are_available(self):
        capabilities = WorkflowCapabilities(independent_roles=True, enforceable_handoffs=True)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / ".okf" / "workflows" / "config.json"
            config.parent.mkdir(parents=True)
            config.write_text(json.dumps({"schema_version": 1, "enabled_profiles": ["sdd"]}))
            decision = resolve_workflow(
                load_policy(root),
                WorkflowAssessment(primary_type="feature", independent_challenge=True),
                capabilities,
            )
        self.assertEqual(decision.execution, "single-agent")
        self.assertIn("gtdd-profile", decision.missing_capabilities)
        self.assertEqual(decision.fallback, "sequential-adversarial-review")
        self.assertTrue(any("GTDD profile is not enabled" in reason for reason in decision.reasons))

    def test_unrequested_gtdd_is_not_selected_even_when_supported(self):
        capabilities = WorkflowCapabilities(independent_roles=True, enforceable_handoffs=True)
        decision = self.resolve(WorkflowAssessment(primary_type="feature"), capabilities)
        self.assertNotIn("gtdd", decision.effective_methods)

    def test_invalid_assessment_value_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "task type"):
            self.resolve(WorkflowAssessment(primary_type="unknown"))


class WorkflowConfigTests(unittest.TestCase):
    def write_config(self, root, value):
        config = root / ".okf" / "workflows" / "config.json"
        config.parent.mkdir(parents=True)
        config.write_text(json.dumps(value), encoding="utf-8")
        return config

    def test_missing_optional_config_loads_builtin_policy(self):
        with tempfile.TemporaryDirectory() as directory:
            policy = load_policy(Path(directory))
        self.assertEqual(policy.schema_version, 1)
        self.assertEqual(policy.default_rigor, "standard")

    def test_unknown_config_key_has_actionable_path(self):
        with tempfile.TemporaryDirectory() as directory:
            self.write_config(Path(directory), {"schema_version": 1, "unknown": True})
            with self.assertRaises(WorkflowConfigError) as caught:
                load_policy(Path(directory))
        self.assertIn(".okf/workflows/config.json", str(caught.exception))
        self.assertIn("unknown", str(caught.exception))

    def test_unsupported_schema_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.write_config(Path(directory), {"schema_version": 2})
            with self.assertRaisesRegex(WorkflowConfigError, "schema_version"):
                load_policy(Path(directory))

    def test_non_integer_schema_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.write_config(Path(directory), {"schema_version": 1.0})
            with self.assertRaisesRegex(WorkflowConfigError, "schema_version"):
                load_policy(Path(directory))

    def test_unknown_nested_config_key_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.write_config(
                Path(directory), {"schema_version": 1, "gtdd": {"fallback": "review", "typo": True}}
            )
            with self.assertRaisesRegex(WorkflowConfigError, "typo"):
                load_policy(Path(directory))

    def test_invalid_enum_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.write_config(Path(directory), {"schema_version": 1, "default_rigor": "extreme"})
            with self.assertRaisesRegex(WorkflowConfigError, "default_rigor"):
                load_policy(Path(directory))

    def test_configured_default_rigor_applies_to_behavioral_task_types(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_config(root, {"schema_version": 1, "default_rigor": "light"})
            decision = resolve_workflow(load_policy(root), WorkflowAssessment(primary_type="feature"))

        self.assertEqual(decision.rigor, "light")

    def test_invalid_selection_value_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            self.write_config(Path(directory), {"schema_version": 1, "selection": "explicit"})
            with self.assertRaisesRegex(WorkflowConfigError, "selection"):
                load_policy(Path(directory))

    def test_global_strict_default_raises_nonbehavioral_category(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_config(root, {"schema_version": 1, "default_rigor": "strict"})
            decision = resolve_workflow(load_policy(root), WorkflowAssessment(primary_type="docs"))
        self.assertEqual(decision.rigor, "strict")

    def test_profile_install_creates_selected_files_and_is_idempotent(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = install_profiles(root, ("sdd", "tdd"))
            config = root / ".okf" / "workflows" / "config.json"
            active = root / ".okf" / "workflows" / "active.md"
            sdd = root / ".okf" / "workflows" / "profiles" / "sdd.md"

            self.assertTrue(config.is_file())
            self.assertTrue(active.is_file())
            self.assertTrue(sdd.is_file())
            self.assertEqual(json.loads(config.read_text())["enabled_profiles"], ["sdd", "tdd"])
            self.assertIn(".okf/workflows/profiles/sdd.md", first.created)
            custom = "# Team SDD profile\n"
            sdd.write_text(custom, encoding="utf-8")

            second = install_profiles(root, ("sdd", "tdd"))

            self.assertEqual(sdd.read_text(encoding="utf-8"), custom)
            self.assertIn(".okf/workflows/profiles/sdd.md", second.retained)
            self.assertFalse(second.conflicts)

    def test_profile_install_rejects_unknown_profile(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "profile"):
                install_profiles(Path(directory), ("unknown",))

    def test_profile_install_refuses_workflow_directory_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            root = Path(directory)
            (root / ".okf").mkdir()
            (root / ".okf" / "workflows").symlink_to(Path(outside), target_is_directory=True)

            result = install_profiles(root, ("sdd",))

            self.assertTrue(any("unsafe" in conflict for conflict in result.conflicts))
            self.assertEqual(list(Path(outside).iterdir()), [])

    def test_policy_loader_rejects_config_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            root = Path(directory)
            workflow = root / ".okf" / "workflows"
            workflow.mkdir(parents=True)
            external = Path(outside) / "config.json"
            external.write_text(json.dumps({"schema_version": 1}), encoding="utf-8")
            (workflow / "config.json").symlink_to(external)

            with self.assertRaisesRegex(WorkflowConfigError, "unsafe"):
                load_policy(root)


class WorkflowMCPTests(unittest.TestCase):
    class FakeServer:
        def __init__(self, _name):
            self.tools = {}

        def tool(self, name):
            def register(function):
                self.tools[name] = function
                return function
            return register

        def run(self):
            pass

    def create_fake_server(self, root):
        module = SimpleNamespace(FastMCP=self.FakeServer)
        with patch.dict(sys.modules, {"fastmcp": module}):
            from okf_agent.mcp import create_server
            return create_server(root)

    def test_context_adds_short_routing_only_when_profiles_exist(self):
        from okf_agent.mcp import context

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertNotIn("workflow_routing", context(root, "ordinary request"))

            install_profiles(root, ("sdd",))
            result = context(root, "ordinary request")

        self.assertIn(".okf/workflows/active.md", result["workflow_routing"])
        self.assertNotIn("Use specification-driven development", result["workflow_routing"])

    def test_mcp_policy_and_resolver_tools_match_cli(self):
        from okf_agent.cli import main

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            server = self.create_fake_server(root)
            policy = server.tools["okf.get_workflow_policy"]()
            self.assertEqual(policy["schema_version"], 1)

            output = StringIO()
            with redirect_stdout(output):
                self.assertEqual(main([
                    "workflow", "resolve", str(root), "--type", "feature",
                    "--risk", "strict", "--stateful",
                ]), 0)
            cli_result = json.loads(output.getvalue())
            mcp_result = server.tools["okf.resolve_workflow"](
                primary_type="feature", risk="strict", stateful=True
            )

        self.assertEqual(mcp_result, cli_result)


if __name__ == "__main__":
    unittest.main()
