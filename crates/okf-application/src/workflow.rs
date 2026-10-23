use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use okf_domain::workflow::{
    GtddFallback, RigorLevel, TaskType, WorkflowAssessment, WorkflowCapabilities, WorkflowDecision,
    WorkflowMethod, WorkflowPolicy,
};

const METHOD_ORDER: [WorkflowMethod; 3] = [
    WorkflowMethod::Sdd,
    WorkflowMethod::Tdd,
    WorkflowMethod::Gtdd,
];

pub fn resolve(
    policy: WorkflowPolicy,
    assessment: WorkflowAssessment,
    capabilities: WorkflowCapabilities,
) -> Result<WorkflowDecision, Diagnostic> {
    if policy.schema_version != 1 {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "workflow schema_version must be 1",
        ));
    }
    if assessment
        .secondary_types
        .contains(&assessment.primary_type)
    {
        return Err(Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            "primary task type must not also appear as a secondary task type",
        ));
    }

    let task_type = assessment.primary_type;
    let rigor = if assessment.high_impact {
        RigorLevel::Strict
    } else if let Some(risk) = assessment.risk {
        risk
    } else if policy.default_rigor == RigorLevel::Strict
        || matches!(
            task_type,
            TaskType::Feature | TaskType::Bugfix | TaskType::Hotfix | TaskType::Refactor
        )
    {
        policy.default_rigor
    } else {
        match task_type {
            TaskType::Docs | TaskType::Tests | TaskType::Maintenance => RigorLevel::Light,
            _ => RigorLevel::Standard,
        }
    };
    let rigor = if task_type == TaskType::Hotfix && rigor == RigorLevel::Light {
        RigorLevel::Standard
    } else {
        rigor
    };

    let mut methods = policy.defaults.get(&task_type).cloned().unwrap_or_default();
    if !assessment.requested_methods.is_empty() {
        methods = assessment.requested_methods.clone();
    }
    let mut required = assessment.requested_methods.clone();
    if assessment.ambiguous || rigor == RigorLevel::Strict {
        methods.push(WorkflowMethod::Sdd);
        required.push(WorkflowMethod::Sdd);
    }
    if assessment.behavioral && matches!(task_type, TaskType::Docs | TaskType::Tests) {
        methods.push(WorkflowMethod::Tdd);
        required.push(WorkflowMethod::Tdd);
    }
    methods = ordered(&methods);
    methods.retain(|method| {
        policy.enabled_profiles.contains(method)
            || required.contains(method)
            || *method == WorkflowMethod::Gtdd
    });

    let wants_gtdd = assessment.independent_challenge || methods.contains(&WorkflowMethod::Gtdd);
    let needs_challenge = assessment.independent_challenge
        || assessment.requested_methods.contains(&WorkflowMethod::Gtdd);
    let mut fallback = None;
    let mut missing = Vec::new();
    let mut execution = "single-agent";
    if wants_gtdd && needs_challenge {
        if !policy.enabled_profiles.contains(&WorkflowMethod::Gtdd) {
            missing.push("gtdd-profile".to_owned());
        }
        if !capabilities.independent_roles {
            missing.push("independent-roles".to_owned());
        }
        if !capabilities.enforceable_handoffs {
            missing.push("enforceable-handoffs".to_owned());
        }
        if missing.is_empty() {
            execution = "independent-roles";
            if !methods.contains(&WorkflowMethod::Gtdd) {
                methods.push(WorkflowMethod::Gtdd);
            }
        } else {
            fallback = Some(policy.gtdd.fallback);
            methods.retain(|method| *method != WorkflowMethod::Gtdd);
            methods.push(WorkflowMethod::Gtdd);
        }
    }
    methods = ordered(&methods);
    let effective_methods: Vec<_> = methods
        .iter()
        .copied()
        .filter(|method| *method != WorkflowMethod::Gtdd || execution == "independent-roles")
        .collect();

    let mut phases = vec![
        "Retrieve repository context",
        "Classify work and assess risk",
    ];
    if effective_methods.contains(&WorkflowMethod::Sdd) {
        phases.push("Specify behavior");
    }
    if effective_methods.contains(&WorkflowMethod::Tdd) {
        phases.push("Establish verification evidence");
    }
    if rigor == RigorLevel::Strict {
        phases.push("Broader verification");
    }
    phases.push("Implement");
    if execution == "independent-roles" {
        phases.push("Independent adversarial testing");
    } else if fallback.is_some() {
        phases.push("Single-agent adversarial review");
    }
    phases.extend([
        "Review results and unresolved issues",
        "Update repository knowledge and validate",
        "Report evidence and limitations",
    ]);
    phases.dedup();

    let mut reasons = vec![format!(
        "Classified as {} with {} rigor.",
        task_name(task_type),
        rigor_name(rigor)
    )];
    if assessment.ambiguous {
        reasons.push("Ambiguous behavior adds a specification phase.".to_owned());
    }
    if assessment.high_impact {
        reasons.push("High-impact work requires strict rigor and broader verification.".to_owned());
    }
    if task_type == TaskType::Hotfix && assessment.risk == Some(RigorLevel::Light) {
        reasons.push("Hotfix urgency cannot reduce rigor below standard verification.".to_owned());
    }
    if !missing.is_empty() {
        let labels = missing
            .iter()
            .map(|item| match item.as_str() {
                "gtdd-profile" => "the GTDD profile is not enabled",
                "independent-roles" => "independent role contexts are unavailable",
                _ => "enforceable handoffs are unavailable",
            })
            .collect::<Vec<_>>()
            .join(" and ");
        reasons.push(format!("Independent GTDD was requested, but {labels}."));
    }
    let artifacts = if effective_methods.contains(&WorkflowMethod::Sdd) {
        vec!["behavior contract".to_owned()]
    } else {
        vec![]
    };
    let gates = if effective_methods.contains(&WorkflowMethod::Tdd)
        || rigor != RigorLevel::Light
        || task_type == TaskType::Hotfix
    {
        vec!["targeted verification".to_owned()]
    } else {
        vec![]
    };
    Ok(WorkflowDecision {
        primary_type: task_type,
        secondary_types: assessment.secondary_types,
        high_impact: assessment.high_impact,
        rigor,
        requested_methods: assessment.requested_methods,
        effective_methods,
        phases: phases.into_iter().map(str::to_owned).collect(),
        reasons,
        artifacts,
        gates,
        execution: execution.to_owned(),
        missing_capabilities: missing,
        fallback,
    })
}

fn ordered(values: &[WorkflowMethod]) -> Vec<WorkflowMethod> {
    METHOD_ORDER
        .into_iter()
        .filter(|method| values.contains(method))
        .collect()
}

fn task_name(value: TaskType) -> &'static str {
    match value {
        TaskType::Feature => "feature",
        TaskType::Bugfix => "bugfix",
        TaskType::Hotfix => "hotfix",
        TaskType::Docs => "docs",
        TaskType::Tests => "tests",
        TaskType::Refactor => "refactor",
        TaskType::Maintenance => "maintenance",
    }
}

fn rigor_name(value: RigorLevel) -> &'static str {
    match value {
        RigorLevel::Light => "light",
        RigorLevel::Standard => "standard",
        RigorLevel::Strict => "strict",
    }
}

#[allow(dead_code)]
fn _fallback_name(value: GtddFallback) -> &'static str {
    match value {
        GtddFallback::SequentialAdversarialReview => "sequential-adversarial-review",
    }
}
