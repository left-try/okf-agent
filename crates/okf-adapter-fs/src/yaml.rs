use serde::de::DeserializeOwned;

use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use okf_domain::paths::RepoPath;

pub fn parse_yaml<T>(source: &str, path: &RepoPath) -> Result<T, Diagnostic>
where
    T: DeserializeOwned,
{
    serde_saphyr::from_str(source).map_err(|error| {
        Diagnostic::new(
            ErrorKind::InvalidConfiguration,
            format!("cannot parse YAML: {error}"),
        )
        .at_path(path.as_str())
    })
}
