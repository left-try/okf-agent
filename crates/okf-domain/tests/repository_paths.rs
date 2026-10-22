use std::fs;
use std::path::{Path, PathBuf};

use okf_domain::paths::RepoPath;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "okf-agent-path-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn relative_paths_normalize_separators_and_preserve_spaces_and_unicode() {
    let scratch = Scratch::new();
    let root = scratch.0.join("repo");
    fs::create_dir_all(&root).unwrap();

    let path = RepoPath::new(&root, Path::new("docs/naïve file.md")).unwrap();

    assert_eq!(path.as_str(), "docs/naïve file.md");
}

#[test]
fn absolute_paths_inside_root_become_repository_relative() {
    let scratch = Scratch::new();
    let root = scratch.0.join("repo");
    fs::create_dir_all(&root).unwrap();
    let absolute = root.join("nested").join("page.md");

    let path = RepoPath::new(&root, &absolute).unwrap();

    assert_eq!(path.as_str(), "nested/page.md");
}

#[test]
fn parent_components_are_rejected_even_when_the_final_path_is_inside() {
    let scratch = Scratch::new();
    let root = scratch.0.join("repo");
    fs::create_dir_all(root.join("nested")).unwrap();

    let error = RepoPath::new(&root, Path::new("nested/../page.md")).unwrap_err();

    assert_eq!(error.kind, okf_domain::diagnostics::ErrorKind::InvalidPath);
}

#[test]
fn windows_absolute_path_on_another_drive_is_rejected() {
    #[cfg(windows)]
    {
        let scratch = Scratch::new();
        let root = scratch.0.join("repo");
        fs::create_dir_all(&root).unwrap();
        let root_drive = root
            .to_string_lossy()
            .chars()
            .next()
            .unwrap()
            .to_ascii_uppercase();
        let other_drive = if root_drive == 'Z' { 'C' } else { 'Z' };
        let outside = PathBuf::from(format!("{other_drive}:\\outside\\page.md"));

        assert!(RepoPath::new(&root, &outside).is_err());
    }
}

#[test]
fn symlinked_ancestor_cannot_escape_root() {
    let scratch = Scratch::new();
    let root = scratch.0.join("repo");
    let outside = scratch.0.join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let link = root.join("linked");

    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_dir(&outside, &link);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(&outside, &link);
    #[cfg(not(any(windows, unix)))]
    let result: std::io::Result<()> = Err(std::io::Error::other("directory symlinks unsupported"));

    if result.is_err() {
        eprintln!("skipping symlink escape assertion because symlink creation is unavailable");
        return;
    }

    let error = RepoPath::new(&root, Path::new("linked/new-file.md")).unwrap_err();

    assert_eq!(error.kind, okf_domain::diagnostics::ErrorKind::InvalidPath);
}
