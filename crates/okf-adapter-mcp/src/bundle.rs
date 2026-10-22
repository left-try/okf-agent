use rmcp::{
    Json, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

fn failure(error: impl std::fmt::Display) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(error.to_string(), None)
}
fn output(value: Value) -> Json<Value> {
    Json(value)
}
#[derive(Clone)]
pub struct BundleServer {
    root: PathBuf,
    index_root: PathBuf,
    tool_router: ToolRouter<Self>,
}
impl BundleServer {
    pub fn new(root: PathBuf, index_root: PathBuf) -> Self {
        Self {
            root,
            index_root,
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
#[derive(Deserialize, JsonSchema)]
pub struct IndexArgs {
    #[serde(default)]
    pub wikilinks: bool,
}
#[derive(Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_limit() -> usize {
    10
}
#[derive(Deserialize, JsonSchema)]
pub struct GraphArgs {
    pub path: String,
    #[serde(default = "default_depth")]
    pub depth: usize,
}
fn default_depth() -> usize {
    1
}

#[tool_router]
impl BundleServer {
    #[tool(
        name = "okf.bundle_index",
        description = "Index a standalone Markdown bundle"
    )]
    async fn index(
        &self,
        Parameters(args): Parameters<IndexArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = self.reindex(args.wikilinks).map_err(failure)?;
        Ok(output(serde_json::to_value(report).map_err(failure)?))
    }
    #[tool(
        name = "okf.bundle_validate",
        description = "Validate the standalone Markdown bundle"
    )]
    async fn validate(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        let report = self.reindex(false).map_err(failure)?;
        Ok(output(okf_application::bundle::validation_result(&report)))
    }
    #[tool(
        name = "okf.bundle_search",
        description = "Search indexed Markdown bundle pages"
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        okf_application::bundle::require_index(&self.index_root).map_err(failure)?;
        let hits = okf_application::bundle::search(
            &self.index_root,
            &args.query,
            args.limit.max(1),
            &okf_adapter_sqlite::bundle::SqliteBundleStore,
        )
        .map_err(failure)?;
        Ok(output(json!(hits)))
    }
    #[tool(
        name = "okf.bundle_backlinks",
        description = "Find incoming Markdown links to a bundle page"
    )]
    async fn backlinks(
        &self,
        Parameters(args): Parameters<GraphArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        okf_application::bundle::require_index(&self.index_root).map_err(failure)?;
        let result = okf_application::bundle::graph(
            &self.index_root,
            &args.path,
            args.depth,
            true,
            &okf_adapter_sqlite::bundle::SqliteBundleStore,
        )
        .map_err(failure)?;
        Ok(output(result))
    }
    #[tool(
        name = "okf.bundle_neighborhood",
        description = "Traverse outgoing Markdown links from a bundle page"
    )]
    async fn neighborhood(
        &self,
        Parameters(args): Parameters<GraphArgs>,
    ) -> Result<Json<Value>, rmcp::ErrorData> {
        okf_application::bundle::require_index(&self.index_root).map_err(failure)?;
        let result = okf_application::bundle::graph(
            &self.index_root,
            &args.path,
            args.depth,
            false,
            &okf_adapter_sqlite::bundle::SqliteBundleStore,
        )
        .map_err(failure)?;
        Ok(output(result))
    }
    #[tool(
        name = "okf.bundle_export",
        description = "Export indexed Markdown bundle pages, links, and provenance"
    )]
    async fn export(&self) -> Result<Json<Value>, rmcp::ErrorData> {
        okf_application::bundle::require_index(&self.index_root).map_err(failure)?;
        let result = okf_application::bundle::export(
            &self.index_root,
            &okf_adapter_sqlite::bundle::SqliteBundleStore,
        )
        .map_err(failure)?;
        Ok(output(result))
    }
}
impl BundleServer {
    fn reindex(
        &self,
        wikilinks: bool,
    ) -> Result<okf_application::bundle::BundleIndexReport, okf_domain::diagnostics::Diagnostic>
    {
        let root = std::fs::canonicalize(&self.root).map_err(|e| {
            okf_domain::diagnostics::Diagnostic::new(
                okf_domain::diagnostics::ErrorKind::InvalidPath,
                format!("cannot resolve bundle root: {e}"),
            )
        })?;
        std::fs::create_dir_all(&self.index_root).map_err(|e| {
            okf_domain::diagnostics::Diagnostic::new(
                okf_domain::diagnostics::ErrorKind::Io,
                e.to_string(),
            )
        })?;
        okf_application::bundle::index(
            &root,
            &self.index_root,
            wikilinks,
            &okf_adapter_fs::bundle::FsBundleSource,
            &okf_adapter_sqlite::bundle::SqliteBundleStore,
        )
    }
}
#[tool_handler(router=self.tool_router)]
impl ServerHandler for BundleServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("OKF read-only Markdown bundle indexing and graph tools")
    }
}

pub async fn serve(root: PathBuf, index_root: PathBuf) -> Result<(), String> {
    let server = BundleServer::new(root, index_root)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| e.to_string())?;
    server.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}
