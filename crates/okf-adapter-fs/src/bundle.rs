use std::path::Path;

use okf_application::bundle::{BundleSource, RawPage};
use okf_domain::diagnostics::{Diagnostic, ErrorKind};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::markdown_links::markdown_targets;

#[derive(Default)]
pub struct FsBundleSource;

impl BundleSource for FsBundleSource {
    fn read_all(&self, root: &Path, wikilinks: bool) -> Result<Vec<RawPage>, Diagnostic> {
        if !root.is_dir() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidPath,
                format!("Bundle root is not a directory: {}", root.display()),
            ));
        }
        let mut paths = Vec::new();
        for entry in WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(|error| Diagnostic::new(ErrorKind::Io, error.to_string()))?;
            if entry.file_type().is_file()
                && !entry.file_type().is_symlink()
                && entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("md"))
            {
                paths.push(entry.into_path());
            }
        }
        paths.sort_by_key(|path| path.to_string_lossy().to_lowercase());
        let mut pages = Vec::new();
        for path in paths {
            let bytes = std::fs::read(&path).map_err(|error| {
                Diagnostic::new(ErrorKind::Io, error.to_string())
                    .at_path(path.to_string_lossy().to_string())
            })?;
            let source = String::from_utf8(bytes.clone()).map_err(|error| {
                Diagnostic::new(
                    ErrorKind::InvalidConfiguration,
                    format!("Markdown file is not valid UTF-8: {error}"),
                )
                .at_path(path.to_string_lossy().to_string())
            })?;
            let (metadata, body, parse_error) = crate::frontmatter::parse_frontmatter(&source);
            let relative = path
                .strip_prefix(root)
                .map_err(|error| Diagnostic::new(ErrorKind::InvalidPath, error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            let digest = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            pages.push(RawPage {
                path: relative,
                metadata,
                targets: markdown_targets(&body, wikilinks),
                body,
                digest,
                parse_error,
            });
        }
        Ok(pages)
    }
}
