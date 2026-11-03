//! Optional MCP delivery adapter.

#[cfg(feature = "mcp")]
pub mod bundle;
#[cfg(feature = "mcp")]
pub mod repository;
#[cfg(feature = "mcp")]
pub mod update_check;
