use std::{
    io,
    path::PathBuf,
    process::{Command, ExitStatus, Output},
};

pub fn contains(name: &str) -> bool {
    find(name).is_some()
}

pub fn output(name: &str, args: &[&str]) -> io::Result<Output> {
    let executable = find(name).unwrap_or_else(|| PathBuf::from(name));
    command(&executable, args).output()
}

pub fn status(name: &str) -> io::Result<ExitStatus> {
    let executable = find(name).unwrap_or_else(|| PathBuf::from(name));
    command(&executable, &[]).status()
}

fn find(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(name);
    if path.components().count() > 1 || path.is_absolute() {
        return candidates(&path)
            .into_iter()
            .find(|candidate| candidate.is_file());
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .flat_map(|directory| candidates(&directory.join(name)))
        .find(|candidate| candidate.is_file())
}

fn candidates(path: &std::path::Path) -> Vec<PathBuf> {
    let paths = vec![path.to_path_buf()];
    #[cfg(windows)]
    let paths = {
        let mut paths = paths;
        if path.extension().is_none() {
            let extensions =
                std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
            paths.extend(
                extensions
                    .split(';')
                    .filter(|extension| !extension.is_empty())
                    .map(|extension| path.with_extension(extension.trim_start_matches('.'))),
            );
        }
        paths
    };
    paths
}

#[cfg(windows)]
fn command(executable: &std::path::Path, args: &[&str]) -> Command {
    use std::ffi::OsStr;
    let is_script = executable
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "cmd" | "bat"));
    if is_script {
        use std::os::windows::process::CommandExt;

        let mut command = Command::new("cmd.exe");
        let mut invocation = format!("\"{}\"", executable.display());
        for arg in args {
            invocation.push(' ');
            invocation.push_str(arg);
        }
        command.args(["/D", "/S", "/C"]);
        command.raw_arg(format!(" \"{invocation}\""));
        command
    } else {
        let mut command = Command::new(executable);
        command.args(args);
        command
    }
}

#[cfg(not(windows))]
fn command(executable: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(executable);
    command.args(args);
    command
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn launches_batch_wrappers_from_paths_containing_spaces() {
        use std::{fs, time::SystemTime};

        let root = std::env::temp_dir().join(format!(
            "okf agent process {} {}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let script = root.join("fake-client.cmd");
        fs::write(&script, "@echo off\r\necho launched\r\n").unwrap();

        let output = super::command(&script, &[]).output().unwrap();

        assert!(
            output.status.success(),
            "status={}, stdout={}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("launched"));
        let _ = fs::remove_dir_all(root);
    }
}
