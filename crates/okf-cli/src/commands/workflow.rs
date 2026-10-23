use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use okf_application::workflow::resolve;
use okf_domain::workflow::{
    RigorLevel, TaskType, WorkflowAssessment, WorkflowCapabilities, WorkflowMethod,
};
use serde::Serialize;

#[derive(Debug, Args)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    command: WorkflowCommand,
}

#[derive(Debug, Subcommand)]
enum WorkflowCommand {
    /// Show validated repository workflow policy.
    Show {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Explain the selected workflow for an assessment.
    Explain(AssessmentArgs),
    /// Resolve a concrete workflow for an assessment.
    Resolve(AssessmentArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum TaskArg {
    Feature,
    Bugfix,
    Hotfix,
    Docs,
    Tests,
    Refactor,
    Maintenance,
}

impl From<TaskArg> for TaskType {
    fn from(value: TaskArg) -> Self {
        match value {
            TaskArg::Feature => Self::Feature,
            TaskArg::Bugfix => Self::Bugfix,
            TaskArg::Hotfix => Self::Hotfix,
            TaskArg::Docs => Self::Docs,
            TaskArg::Tests => Self::Tests,
            TaskArg::Refactor => Self::Refactor,
            TaskArg::Maintenance => Self::Maintenance,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RiskArg {
    Light,
    Standard,
    Strict,
}

impl From<RiskArg> for RigorLevel {
    fn from(value: RiskArg) -> Self {
        match value {
            RiskArg::Light => Self::Light,
            RiskArg::Standard => Self::Standard,
            RiskArg::Strict => Self::Strict,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum MethodArg {
    Sdd,
    Tdd,
    Gtdd,
}

impl From<MethodArg> for WorkflowMethod {
    fn from(value: MethodArg) -> Self {
        match value {
            MethodArg::Sdd => Self::Sdd,
            MethodArg::Tdd => Self::Tdd,
            MethodArg::Gtdd => Self::Gtdd,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExecutionArg {
    SingleAgent,
    IndependentRoles,
}

#[derive(Debug, Args)]
struct AssessmentArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long = "type", value_enum, required = true)]
    task_type: TaskArg,
    #[arg(long = "secondary-type", value_enum)]
    secondary_types: Vec<TaskArg>,
    #[arg(long, value_enum)]
    risk: Option<RiskArg>,
    #[arg(long = "method", value_enum)]
    methods: Vec<MethodArg>,
    #[arg(long)]
    ambiguous: bool,
    #[arg(long)]
    behavioral: bool,
    #[arg(long = "high-impact")]
    high_impact: bool,
    #[arg(long = "stateful")]
    stateful: bool,
    #[arg(long, value_enum, default_value = "single-agent")]
    execution: ExecutionArg,
}

pub fn run(args: WorkflowArgs) -> i32 {
    match args.command {
        WorkflowCommand::Show { path } => {
            match okf_adapter_fs::load_workflow_policy(&super::root(&path)) {
                Ok(policy) => print_json(&policy, 0),
                Err(error) => print_json(&ErrorOutput { error }, 2),
            }
        }
        WorkflowCommand::Explain(args) | WorkflowCommand::Resolve(args) => {
            let policy = match okf_adapter_fs::load_workflow_policy(&super::root(&args.path)) {
                Ok(policy) => policy,
                Err(error) => return print_json(&ErrorOutput { error }, 2),
            };
            let independent = matches!(args.execution, ExecutionArg::IndependentRoles);
            let assessment = WorkflowAssessment {
                primary_type: args.task_type.into(),
                secondary_types: args.secondary_types.into_iter().map(Into::into).collect(),
                risk: args.risk.map(Into::into),
                requested_methods: args.methods.into_iter().map(Into::into).collect(),
                ambiguous: args.ambiguous,
                behavioral: args.behavioral,
                independent_challenge: args.stateful,
                high_impact: args.high_impact,
            };
            let capabilities = WorkflowCapabilities {
                independent_roles: independent,
                enforceable_handoffs: independent,
            };
            match resolve(policy, assessment, capabilities) {
                Ok(decision) => print_json(&decision, 0),
                Err(error) => print_json(&ErrorOutput { error }, 2),
            }
        }
    }
}

#[derive(Serialize)]
struct ErrorOutput {
    error: okf_domain::diagnostics::Diagnostic,
}

fn print_json(value: &impl Serialize, code: i32) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(output) => {
            println!("{output}");
            code
        }
        Err(error) => {
            eprintln!("cannot serialize workflow result: {error}");
            2
        }
    }
}
