use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    #[default]
    Feature,
    Bugfix,
    Hotfix,
    Docs,
    Tests,
    Refactor,
    Maintenance,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowMethod {
    Sdd,
    Tdd,
    Gtdd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RigorLevel {
    Light,
    Standard,
    Strict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GtddExecution {
    RecommendOnly,
    Available,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GtddFallback {
    SequentialAdversarialReview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowSelection {
    Adaptive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtddPolicy {
    #[serde(default = "default_gtdd_execution")]
    pub execution: GtddExecution,
    #[serde(default = "default_gtdd_fallback")]
    pub fallback: GtddFallback,
}

impl Default for GtddPolicy {
    fn default() -> Self {
        Self {
            execution: default_gtdd_execution(),
            fallback: default_gtdd_fallback(),
        }
    }
}

fn default_gtdd_execution() -> GtddExecution {
    GtddExecution::RecommendOnly
}

fn default_gtdd_fallback() -> GtddFallback {
    GtddFallback::SequentialAdversarialReview
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkflowPolicy {
    pub schema_version: u32,
    pub enabled_profiles: Vec<WorkflowMethod>,
    pub selection: WorkflowSelection,
    pub default_rigor: RigorLevel,
    pub defaults: BTreeMap<TaskType, Vec<WorkflowMethod>>,
    pub gtdd: GtddPolicy,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowPolicyInput {
    schema_version: u32,
    #[serde(default = "default_enabled_profiles")]
    enabled_profiles: Vec<WorkflowMethod>,
    #[serde(default = "default_selection")]
    selection: WorkflowSelection,
    #[serde(default = "default_rigor")]
    default_rigor: RigorLevel,
    #[serde(default)]
    defaults: BTreeMap<TaskType, Vec<WorkflowMethod>>,
    #[serde(default)]
    gtdd: GtddPolicy,
}

impl<'de> Deserialize<'de> for WorkflowPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = WorkflowPolicyInput::deserialize(deserializer)?;
        let mut defaults = builtin_defaults();
        defaults.extend(input.defaults);
        Ok(Self {
            schema_version: input.schema_version,
            enabled_profiles: input.enabled_profiles,
            selection: input.selection,
            default_rigor: input.default_rigor,
            defaults,
            gtdd: input.gtdd,
        })
    }
}

impl Default for WorkflowPolicy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            enabled_profiles: default_enabled_profiles(),
            selection: default_selection(),
            default_rigor: default_rigor(),
            defaults: builtin_defaults(),
            gtdd: GtddPolicy::default(),
        }
    }
}

fn default_enabled_profiles() -> Vec<WorkflowMethod> {
    vec![
        WorkflowMethod::Sdd,
        WorkflowMethod::Tdd,
        WorkflowMethod::Gtdd,
    ]
}

fn default_selection() -> WorkflowSelection {
    WorkflowSelection::Adaptive
}

fn default_rigor() -> RigorLevel {
    RigorLevel::Standard
}

fn builtin_defaults() -> BTreeMap<TaskType, Vec<WorkflowMethod>> {
    BTreeMap::from([
        (
            TaskType::Feature,
            vec![WorkflowMethod::Sdd, WorkflowMethod::Tdd],
        ),
        (TaskType::Bugfix, vec![WorkflowMethod::Tdd]),
        (TaskType::Hotfix, vec![WorkflowMethod::Tdd]),
        (TaskType::Docs, vec![]),
        (TaskType::Tests, vec![]),
        (TaskType::Refactor, vec![]),
        (TaskType::Maintenance, vec![]),
    ])
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowAssessment {
    pub primary_type: TaskType,
    #[serde(default)]
    pub secondary_types: Vec<TaskType>,
    pub risk: Option<RigorLevel>,
    #[serde(default)]
    pub requested_methods: Vec<WorkflowMethod>,
    #[serde(default)]
    pub ambiguous: bool,
    #[serde(default)]
    pub behavioral: bool,
    #[serde(default)]
    pub independent_challenge: bool,
    #[serde(default)]
    pub high_impact: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowCapabilities {
    #[serde(default)]
    pub independent_roles: bool,
    #[serde(default)]
    pub enforceable_handoffs: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDecision {
    pub primary_type: TaskType,
    pub secondary_types: Vec<TaskType>,
    pub high_impact: bool,
    pub rigor: RigorLevel,
    pub requested_methods: Vec<WorkflowMethod>,
    pub effective_methods: Vec<WorkflowMethod>,
    pub phases: Vec<String>,
    pub reasons: Vec<String>,
    pub artifacts: Vec<String>,
    pub gates: Vec<String>,
    pub execution: String,
    pub missing_capabilities: Vec<String>,
    pub fallback: Option<GtddFallback>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileInstallResult {
    pub created: Vec<String>,
    pub retained: Vec<String>,
    pub updated: Vec<String>,
    pub conflicts: Vec<String>,
}
