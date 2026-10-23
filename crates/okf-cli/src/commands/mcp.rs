use clap::Args;
#[cfg(not(feature = "mcp"))]
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct McpArgs {
    #[arg(long, default_value = ".")]
    pub repo: PathBuf,
    #[arg(long,value_parser=["repository","bundle"],default_value="repository")]
    pub profile: String,
    #[arg(long)]
    pub root: Option<PathBuf>,
    #[arg(long = "index-root")]
    pub index_root: Option<PathBuf>,
}
pub fn run(args: McpArgs) -> i32 {
    if args.profile == "bundle" && (args.root.is_none() || args.index_root.is_none()) {
        eprintln!("mcp --profile bundle requires --root and --index-root");
        return 2;
    }
    #[cfg(feature = "mcp")]
    {
        let result = if args.profile == "bundle" {
            let root = super::root(args.root.as_deref().unwrap());
            let index = super::root(args.index_root.as_deref().unwrap());
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .and_then(|runtime| {
                    runtime
                        .block_on(okf_adapter_mcp::bundle::serve(root, index))
                        .map_err(std::io::Error::other)
                })
        } else {
            let root = super::root(&args.repo);
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .and_then(|runtime| {
                    runtime
                        .block_on(okf_adapter_mcp::repository::serve(root))
                        .map_err(std::io::Error::other)
                })
        };
        match result {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("MCP server failed: {error}");
                2
            }
        }
    }
    #[cfg(not(feature = "mcp"))]
    {
        let _ = args;
        println!(
            "{}",
            json!({"error":{"code":"feature_disabled","message":"MCP support is optional. Rebuild with `cargo install okf-agent --features mcp` or run `cargo run --features mcp -- mcp`."}})
        );
        2
    }
}
