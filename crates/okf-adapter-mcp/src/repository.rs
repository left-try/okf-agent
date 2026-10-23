use okf_domain::{
    diagnostics::{Diagnostic, ErrorKind},
    paths::RepoPath,
    workflow::{WorkflowAssessment, WorkflowCapabilities},
};
use rmcp::{
    Json, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn failure(error: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(error.to_string(), None)
}
fn output(value: Value) -> Json<Value> {
    Json(value)
}

pub fn read_concept(root: &Path, path: &str) -> Result<String, Diagnostic> {
    let relative = Path::new(".okf").join(path);
    let safe = RepoPath::new(root, &relative)?;
    if !safe.as_str().starts_with(".okf/") || !safe.as_str().ends_with(".md") {
        return Err(Diagnostic::new(
            ErrorKind::InvalidPath,
            "Concept path must be a Markdown file inside .okf/",
        ));
    }
    fs::read_to_string(root.join(safe.as_path()))
        .map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()).at_path(safe.as_str()))
}

#[derive(Clone)]
pub struct RepositoryServer {
    root: PathBuf,
    tool_router: ToolRouter<Self>,
}
impl RepositoryServer {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            tool_router: Self::tool_router(),
        }
    }
    pub fn tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect()
    }
}
impl Default for RepositoryServer {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}
fn default_search_limit() -> usize {
    8
}
#[derive(Deserialize, JsonSchema)]
pub struct ConceptArgs {
    pub path: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct ContextArgs {
    pub request: String,
}
#[derive(Deserialize, JsonSchema)]
pub struct DecisionArgs {
    pub title: String,
    pub rationale: String,
    #[serde(default)]
    pub sources: Vec<String>,
}
#[derive(Deserialize, JsonSchema, Default)]
pub struct UpdateArgs {
    #[serde(default)]
    pub changed_files: Option<Vec<String>>,
}
#[derive(Deserialize, JsonSchema)]
pub struct ResolveArgs {
    pub primary_type: String,
    #[serde(default)]
    pub secondary_types: Vec<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub requested_methods: Vec<String>,
    #[serde(default)]
    pub ambiguous: bool,
    #[serde(default)]
    pub behavioral: bool,
    #[serde(default)]
    pub stateful: bool,
    #[serde(default)]
    pub high_impact: bool,
    #[serde(default)]
    pub independent_roles: bool,
    #[serde(default)]
    pub enforceable_handoffs: bool,
}

#[tool_router]
impl RepositoryServer {
    #[tool(
        name = "okf.initialize",
        description = "Initialize the repository knowledge bundle and local index"
    )]
    async fn initialize(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = okf_application::lifecycle::initialize(
            &self.root,
            &[],
            &okf_adapter_fs::lifecycle::FsLifecycle,
            &okf_adapter_git::Git,
            &okf_adapter_fs::discovery::FsSourceCatalog,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        Ok(output(json!({"created":report.created})))
    }
    #[tool(
        name = "okf.status",
        description = "Return repository root and validation status"
    )]
    async fn status(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = okf_application::lifecycle::validate(
            &self.root,
            &okf_adapter_fs::lifecycle::FsLifecycle,
            &okf_adapter_fs::discovery::FsSourceCatalog,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        Ok(output(json!({"root":self.root,"validation":report})))
    }
    #[tool(
        name = "okf.search",
        description = "Search indexed repository source and symbols"
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let results = okf_application::repository_index::search(
            &self.root,
            &args.query,
            args.limit,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        Ok(output(json!(results)))
    }
    #[tool(
        name = "okf.get_concept",
        description = "Read a Markdown concept from .okf"
    )]
    async fn get_concept(
        &self,
        Parameters(args): Parameters<ConceptArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let text = read_concept(&self.root, &args.path).map_err(failure)?;
        Ok(output(json!(text)))
    }
    #[tool(
        name = "okf.get_change_context",
        description = "Search repository context relevant to a request"
    )]
    async fn get_change_context(
        &self,
        Parameters(args): Parameters<ContextArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let matches = okf_application::repository_index::search(
            &self.root,
            &args.request,
            8,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        let concepts = concept_paths(&self.root, &args.request);
        let mut value = json!({"request":args.request,"matches":matches,"concepts":concepts,"protocol":"Retrieve context before edits; update knowledge after edits; validate before completion."});
        if self.root.join(".okf/workflows/config.json").is_file()
            && self.root.join(".okf/workflows/active.md").is_file()
        {
            value["workflow_routing"] = json!(
                "Read .okf/workflows/active.md, classify the task and assess risk, then load only selected profiles and assigned roles."
            );
        }
        Ok(output(value))
    }
    #[tool(
        name = "okf.get_workflow_policy",
        description = "Return the validated workflow policy"
    )]
    async fn get_workflow_policy(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        let policy = okf_adapter_fs::load_workflow_policy(&self.root).map_err(failure)?;
        Ok(output(serde_json::to_value(policy).map_err(failure)?))
    }
    #[tool(
        name = "okf.resolve_workflow",
        description = "Resolve workflow methods and rigor for a task assessment"
    )]
    async fn resolve_workflow(
        &self,
        Parameters(args): Parameters<ResolveArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let assessment:WorkflowAssessment=serde_json::from_value(json!({"primary_type":args.primary_type,"secondary_types":args.secondary_types,"risk":args.risk,"requested_methods":args.requested_methods,"ambiguous":args.ambiguous,"behavioral":args.behavioral,"independent_challenge":args.stateful,"high_impact":args.high_impact})).map_err(failure)?;
        let policy = okf_adapter_fs::load_workflow_policy(&self.root).map_err(failure)?;
        let decision = okf_application::workflow::resolve(
            policy,
            assessment,
            WorkflowCapabilities {
                independent_roles: args.independent_roles,
                enforceable_handoffs: args.enforceable_handoffs,
            },
        )
        .map_err(failure)?;
        Ok(output(serde_json::to_value(decision).map_err(failure)?))
    }
    #[tool(
        name = "okf.record_decision",
        description = "Record a decision in repository knowledge"
    )]
    async fn record_decision(
        &self,
        Parameters(args): Parameters<DecisionArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let path = okf_application::lifecycle::record_decision(
            &self.root,
            &args.title,
            &args.rationale,
            &args.sources,
            &okf_adapter_fs::lifecycle::FsLifecycle,
        )
        .map_err(failure)?;
        Ok(output(json!({"path":path})))
    }
    #[tool(
        name = "okf.update_after_change",
        description = "Refresh repository knowledge after source changes"
    )]
    async fn update_after_change(
        &self,
        Parameters(args): Parameters<UpdateArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = okf_application::lifecycle::update(
            &self.root,
            args.changed_files.as_deref(),
            &okf_adapter_fs::lifecycle::FsLifecycle,
            &okf_adapter_git::Git,
            &okf_adapter_fs::discovery::FsSourceCatalog,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        Ok(output(serde_json::to_value(report).map_err(failure)?))
    }
    #[tool(
        name = "okf.validate",
        description = "Validate repository-owned knowledge and index freshness"
    )]
    async fn validate(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = okf_application::lifecycle::validate(
            &self.root,
            &okf_adapter_fs::lifecycle::FsLifecycle,
            &okf_adapter_fs::discovery::FsSourceCatalog,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        Ok(output(serde_json::to_value(report).map_err(failure)?))
    }
    #[tool(
        name = "okf.reindex",
        description = "Rebuild the disposable repository source index"
    )]
    async fn reindex(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = okf_application::repository_index::build(
            &self.root,
            &okf_adapter_fs::discovery::FsSourceCatalog,
            &okf_adapter_sqlite::repository::SqliteIndexStore,
        )
        .map_err(failure)?;
        Ok(output(serde_json::to_value(report).map_err(failure)?))
    }
}
#[tool_handler(router=self.tool_router)]
impl ServerHandler for RepositoryServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("OKF repository knowledge and source index tools")
    }
}

fn concept_paths(root: &Path, request: &str) -> Vec<String> {
    let mut found = Vec::new();
    let request = request.to_lowercase();
    for entry in walkdir::WalkDir::new(root.join(".okf")).follow_links(false) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file()
            && entry.path().extension().is_some_and(|ext| ext == "md")
            && fs::read_to_string(entry.path())
                .is_ok_and(|text| text.to_lowercase().contains(&request))
            && let Ok(path) = entry.path().strip_prefix(root)
        {
            found.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
    found.sort();
    found.truncate(8);
    found
}

pub async fn serve(root: PathBuf) -> Result<(), String> {
    let server = RepositoryServer::new(root)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| e.to_string())?;
    server.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}
