"""Repository-owned workflow policy loading and deterministic resolution."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Mapping, Sequence


TASK_TYPES = ("feature", "bugfix", "hotfix", "docs", "tests", "refactor", "maintenance")
METHODS = ("sdd", "tdd", "gtdd")
RIGOR_LEVELS = ("light", "standard", "strict")
PROFILE_NAMES = METHODS
CONFIG_RELATIVE_PATH = ".okf/workflows/config.json"

BUILTIN_DEFAULTS = {
    "feature": ("sdd", "tdd"),
    "bugfix": ("tdd",),
    "hotfix": ("tdd",),
    "docs": (),
    "tests": (),
    "refactor": (),
    "maintenance": (),
}
DEFAULT_RIGOR = {
    "feature": "standard",
    "bugfix": "standard",
    "hotfix": "standard",
    "docs": "light",
    "tests": "light",
    "refactor": "standard",
    "maintenance": "light",
}


class WorkflowConfigError(ValueError):
    """Invalid optional workflow policy with an actionable repository path."""


@dataclass(frozen=True)
class WorkflowPolicy:
    schema_version: int = 1
    enabled_profiles: tuple[str, ...] = PROFILE_NAMES
    selection: str = "adaptive"
    default_rigor: str = "standard"
    defaults: Mapping[str, tuple[str, ...]] | None = None
    gtdd_execution: str = "recommend-only"
    gtdd_fallback: str = "sequential-adversarial-review"

    def __post_init__(self) -> None:
        if self.defaults is None:
            object.__setattr__(self, "defaults", dict(BUILTIN_DEFAULTS))


@dataclass(frozen=True)
class WorkflowAssessment:
    primary_type: str
    secondary_types: tuple[str, ...] = ()
    risk: str | None = None
    requested_methods: tuple[str, ...] = ()
    ambiguous: bool = False
    behavioral: bool = False
    independent_challenge: bool = False
    high_impact: bool = False


@dataclass(frozen=True)
class WorkflowCapabilities:
    independent_roles: bool = False
    enforceable_handoffs: bool = False


@dataclass(frozen=True)
class WorkflowDecision:
    primary_type: str
    secondary_types: tuple[str, ...]
    high_impact: bool
    rigor: str
    requested_methods: tuple[str, ...]
    effective_methods: tuple[str, ...]
    phases: tuple[str, ...]
    reasons: tuple[str, ...]
    artifacts: tuple[str, ...]
    gates: tuple[str, ...]
    execution: str
    missing_capabilities: tuple[str, ...]
    fallback: str | None


@dataclass(frozen=True)
class ProfileInstallResult:
    created: tuple[str, ...] = ()
    retained: tuple[str, ...] = ()
    updated: tuple[str, ...] = ()
    conflicts: tuple[str, ...] = ()


def workflow_data(value: Any) -> dict[str, Any]:
    """Serialize a policy or decision into JSON-compatible data."""
    def convert(item: Any) -> Any:
        if isinstance(item, dict):
            return {key: convert(child) for key, child in item.items()}
        if isinstance(item, tuple):
            return [convert(child) for child in item]
        if isinstance(item, list):
            return [convert(child) for child in item]
        return item

    return convert(asdict(value))


def _unknown_keys(value: Mapping[str, Any], allowed: set[str], path: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise WorkflowConfigError(f"{path}: unknown key(s): {', '.join(unknown)}")


def _string_list(value: Any, allowed: Sequence[str], path: str) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise WorkflowConfigError(f"{path} must be an array of strings")
    invalid = sorted(set(value) - set(allowed))
    if invalid:
        raise WorkflowConfigError(f"{path} contains unsupported value(s): {', '.join(invalid)}")
    return tuple(dict.fromkeys(value))


def _validate_policy_data(data: Any) -> WorkflowPolicy:
    path = CONFIG_RELATIVE_PATH
    if not isinstance(data, dict):
        raise WorkflowConfigError(f"{path}: expected a JSON object")
    _unknown_keys(data, {"schema_version", "enabled_profiles", "selection", "default_rigor", "defaults", "gtdd"}, path)
    version = data.get("schema_version")
    if not isinstance(version, int) or isinstance(version, bool) or version != 1:
        raise WorkflowConfigError(f"{path}: schema_version must be 1")

    enabled = _string_list(data.get("enabled_profiles", list(PROFILE_NAMES)), PROFILE_NAMES, f"{path}: enabled_profiles")
    selection = data.get("selection", "adaptive")
    if selection != "adaptive":
        raise WorkflowConfigError(f"{path}: selection must be 'adaptive'")
    rigor = data.get("default_rigor", "standard")
    if rigor not in RIGOR_LEVELS:
        raise WorkflowConfigError(f"{path}: default_rigor must be one of {', '.join(RIGOR_LEVELS)}")

    defaults: dict[str, tuple[str, ...]] = dict(BUILTIN_DEFAULTS)
    configured_defaults = data.get("defaults", {})
    if not isinstance(configured_defaults, dict):
        raise WorkflowConfigError(f"{path}: defaults must be an object")
    _unknown_keys(configured_defaults, set(TASK_TYPES), f"{path}: defaults")
    for task_type, methods in configured_defaults.items():
        defaults[task_type] = _string_list(methods, METHODS, f"{path}: defaults.{task_type}")

    gtdd = data.get("gtdd", {})
    if not isinstance(gtdd, dict):
        raise WorkflowConfigError(f"{path}: gtdd must be an object")
    _unknown_keys(gtdd, {"execution", "fallback"}, f"{path}: gtdd")
    execution = gtdd.get("execution", "recommend-only")
    if execution not in ("recommend-only", "available"):
        raise WorkflowConfigError(f"{path}: gtdd.execution must be 'recommend-only' or 'available'")
    fallback = gtdd.get("fallback", "sequential-adversarial-review")
    if fallback != "sequential-adversarial-review":
        raise WorkflowConfigError(f"{path}: gtdd.fallback must be 'sequential-adversarial-review'")
    return WorkflowPolicy(1, enabled, selection, rigor, defaults, execution, fallback)


def load_policy(root: Path) -> WorkflowPolicy:
    """Load validated repository policy, or built-in adaptive defaults."""
    config = root / CONFIG_RELATIVE_PATH
    if config.is_symlink() or not _inside_root(config, root):
        raise WorkflowConfigError(f"{CONFIG_RELATIVE_PATH}: unsafe symlink or path escapes repository")
    if not config.exists():
        return WorkflowPolicy()
    try:
        data = json.loads(config.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise WorkflowConfigError(f"{CONFIG_RELATIVE_PATH}: cannot read valid JSON: {exc}") from exc
    return _validate_policy_data(data)


def _inside_root(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError:
        return False
    return True


def validate_workflows(root: Path) -> list[str]:
    """Validate optional policy files and all files enabled by that policy."""
    config = root / CONFIG_RELATIVE_PATH
    if not config.exists():
        return []
    try:
        policy = load_policy(root)
    except WorkflowConfigError as exc:
        return [str(exc)]

    errors: list[str] = []
    workflow_dir = root / ".okf" / "workflows"
    required = ["active.md", "classification.md"]
    required.extend(f"profiles/{name}.md" for name in policy.enabled_profiles)
    if "gtdd" in policy.enabled_profiles:
        required.extend(f"roles/{name}.md" for name in ("coder", "tester", "auditor"))
    for relative in required:
        path = workflow_dir / relative
        if path.is_symlink() or not path.is_file():
            errors.append(f"Workflow policy references missing or unsafe file: .okf/workflows/{relative}")
    return errors


def install_profiles(root: Path, profiles: Sequence[str]) -> ProfileInstallResult:
    """Install selected starter profiles without replacing curated files."""
    selected = tuple(dict.fromkeys(profiles))
    invalid = sorted(set(selected) - set(PROFILE_NAMES))
    if invalid:
        raise ValueError(f"Unknown workflow profile(s): {', '.join(invalid)}")
    if not selected:
        raise ValueError("Select at least one workflow profile")

    package_templates = Path(__file__).parent / "templates" / "workflows"
    root = root.resolve()
    created: list[str] = []
    retained: list[str] = []
    updated: list[str] = []
    conflicts: list[str] = []
    relative_files = ["active.md", "classification.md"]
    relative_files.extend(f"profiles/{name}.md" for name in selected)
    if "gtdd" in selected:
        relative_files.extend(f"roles/{name}.md" for name in ("coder", "tester", "auditor"))

    workflow_dir = root / ".okf" / "workflows"
    if not _inside_root(workflow_dir, root):
        return ProfileInstallResult(
            conflicts=(".okf/workflows: unsafe symlink or path escapes repository",)
        )
    config = workflow_dir / "config.json"
    if config.is_symlink():
        conflicts.append(f"{CONFIG_RELATIVE_PATH}: unsafe symlink preserved")
        return ProfileInstallResult((), (), (), tuple(conflicts))
    if config.exists():
        try:
            data = json.loads(config.read_text(encoding="utf-8"))
            policy = _validate_policy_data(data)
        except (OSError, UnicodeError, json.JSONDecodeError, WorkflowConfigError) as exc:
            conflicts.append(f"{CONFIG_RELATIVE_PATH}: preserved existing invalid policy ({exc})")
        else:
            if policy.enabled_profiles != selected:
                data["enabled_profiles"] = list(selected)
                config.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
                updated.append(CONFIG_RELATIVE_PATH)
            else:
                retained.append(CONFIG_RELATIVE_PATH)
    else:
        config.parent.mkdir(parents=True, exist_ok=True)
        source = package_templates / "config.json"
        data = json.loads(source.read_text(encoding="utf-8"))
        data["enabled_profiles"] = list(selected)
        config.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
        created.append(CONFIG_RELATIVE_PATH)

    for relative in relative_files:
        target = workflow_dir / relative
        label = f".okf/workflows/{relative}"
        if not _inside_root(target, root):
            conflicts.append(f"{label}: unsafe symlink or path escapes repository")
            continue
        if target.exists():
            retained.append(label)
            continue
        source = package_templates / relative
        if not source.is_file():
            conflicts.append(f"{label}: packaged workflow template is missing")
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
        created.append(label)
    return ProfileInstallResult(tuple(created), tuple(retained), tuple(updated), tuple(conflicts))


def _ordered_methods(values: Sequence[str]) -> tuple[str, ...]:
    unique = set(values)
    return tuple(method for method in METHODS if method in unique)


def resolve_workflow(
    policy: WorkflowPolicy,
    assessment: WorkflowAssessment,
    capabilities: WorkflowCapabilities | None = None,
) -> WorkflowDecision:
    """Resolve an explicit task/risk assessment into ordered workflow phases."""
    capabilities = capabilities or WorkflowCapabilities()
    task_type = assessment.primary_type
    secondary = tuple(dict.fromkeys(assessment.secondary_types))
    if task_type not in TASK_TYPES or any(value not in TASK_TYPES for value in secondary):
        raise ValueError(f"task type must be one of: {', '.join(TASK_TYPES)}")
    if task_type in secondary:
        raise ValueError("primary task type must not also appear as a secondary task type")
    if assessment.risk is not None and assessment.risk not in RIGOR_LEVELS:
        raise ValueError(f"risk must be one of: {', '.join(RIGOR_LEVELS)}")
    if any(method not in METHODS for method in assessment.requested_methods):
        raise ValueError(f"method must be one of: {', '.join(METHODS)}")

    if assessment.high_impact:
        rigor = "strict"
    elif assessment.risk is not None:
        rigor = assessment.risk
    elif policy.default_rigor == "strict":
        rigor = "strict"
    elif task_type in {"feature", "bugfix", "hotfix", "refactor"}:
        rigor = policy.default_rigor
    else:
        rigor = DEFAULT_RIGOR[task_type]
    if task_type == "hotfix" and rigor == "light":
        rigor = "standard"
    methods = list((policy.defaults or BUILTIN_DEFAULTS)[task_type])
    if assessment.requested_methods:
        methods = list(assessment.requested_methods)
    required_methods = set(assessment.requested_methods)
    if assessment.ambiguous or rigor == "strict":
        methods.append("sdd")
        required_methods.add("sdd")
    if assessment.behavioral and task_type in {"docs", "tests"}:
        methods.append("tdd")
        required_methods.add("tdd")
    methods = list(_ordered_methods(methods))
    enabled = set(policy.enabled_profiles)
    methods = [
        method for method in methods
        if method in enabled or method in required_methods or method == "gtdd"
    ]

    wants_gtdd = assessment.independent_challenge or "gtdd" in methods
    needs_challenge = bool(assessment.independent_challenge or "gtdd" in assessment.requested_methods)
    fallback: str | None = None
    missing: list[str] = []
    execution = "single-agent"
    if wants_gtdd and needs_challenge:
        if "gtdd" not in enabled:
            missing.append("gtdd-profile")
        if not capabilities.independent_roles:
            missing.append("independent-roles")
        if not capabilities.enforceable_handoffs:
            missing.append("enforceable-handoffs")
        if missing:
            fallback = policy.gtdd_fallback
            methods = [method for method in methods if method != "gtdd"]
            methods.append("gtdd")
        else:
            execution = "independent-roles"
            if "gtdd" not in methods:
                methods.append("gtdd")
    methods = list(_ordered_methods(methods))
    effective_methods = tuple(method for method in methods if method != "gtdd" or execution == "independent-roles")

    phases = ["Retrieve repository context", "Classify work and assess risk"]
    if "sdd" in effective_methods:
        phases.append("Specify behavior")
    if "tdd" in effective_methods:
        phases.append("Establish verification evidence")
    if rigor == "strict":
        phases.append("Broader verification")
    phases.append("Implement")
    if execution == "independent-roles":
        phases.append("Independent adversarial testing")
    elif fallback:
        phases.append("Single-agent adversarial review")
    phases.extend(("Review results and unresolved issues", "Update repository knowledge and validate", "Report evidence and limitations"))
    phases = list(dict.fromkeys(phases))

    reasons = [f"Classified as {task_type} with {rigor} rigor."]
    if assessment.ambiguous:
        reasons.append("Ambiguous behavior adds a specification phase.")
    if assessment.high_impact:
        reasons.append("High-impact work requires strict rigor and broader verification.")
    if task_type == "hotfix" and assessment.risk == "light":
        reasons.append("Hotfix urgency cannot reduce rigor below standard verification.")
    if fallback:
        missing_reasons = {
            "gtdd-profile": "the GTDD profile is not enabled",
            "independent-roles": "independent role contexts are unavailable",
            "enforceable-handoffs": "enforceable handoffs are unavailable",
        }
        limitations = [missing_reasons[item] for item in missing if item in missing_reasons]
        reasons.append("Independent GTDD was requested, but " + " and ".join(limitations) + ".")
    artifacts = ("behavior contract",) if "sdd" in effective_methods else ()
    gates = ("targeted verification",) if "tdd" in effective_methods or rigor != "light" or task_type == "hotfix" else ()
    return WorkflowDecision(
        task_type, secondary, assessment.high_impact, rigor, tuple(assessment.requested_methods), effective_methods,
        tuple(phases), tuple(reasons), artifacts, gates, execution, tuple(missing), fallback,
    )
