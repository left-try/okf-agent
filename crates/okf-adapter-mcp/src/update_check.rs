use serde_json::{Value, json};
use std::sync::{Arc, RwLock};

fn fetch() -> Option<String> {
    let output = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--max-time",
            "3",
            "--user-agent",
            "okf-agent-update-check",
            "https://api.github.com/repos/left-try/okf-agent/releases/latest",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Check at most daily for CLI users. Failure never changes command status.
pub fn cli_notice() -> Option<Value> {
    if std::env::var_os("OKF_DISABLE_UPDATE_CHECK").is_some() {
        return None;
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cache"))
        })?;
    let dir = base.join("okf-agent");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("release-check.json");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let cached: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    if now.saturating_sub(cached["checked_at"].as_u64().unwrap_or(0)) < 86_400 {
        return cached
            .get("response")
            .and_then(Value::as_str)
            .and_then(|body| notice(env!("CARGO_PKG_VERSION"), body));
    }
    let response = fetch();
    let found = response
        .as_deref()
        .and_then(|body| notice(env!("CARGO_PKG_VERSION"), body));
    let _ = std::fs::write(
        path,
        json!({"checked_at": now, "response": response}).to_string(),
    );
    found
}

pub type UpdateNotice = Arc<RwLock<Option<Value>>>;

/// Poll in the background so an unavailable network never delays MCP calls.
pub fn start(state: UpdateNotice) {
    if std::env::var_os("OKF_DISABLE_UPDATE_CHECK").is_some() {
        return;
    }
    std::thread::spawn(move || {
        loop {
            if let Some(text) = fetch()
                && let Ok(mut guard) = state.write()
            {
                *guard = notice(env!("CARGO_PKG_VERSION"), &text);
            }
            std::thread::sleep(std::time::Duration::from_secs(24 * 60 * 60));
        }
    });
}

/// Return an agent-readable notice only for a newer stable GitHub release.
pub fn notice(current: &str, response: &str) -> Option<Value> {
    let response: Value = serde_json::from_str(response).ok()?;
    let latest = response
        .get("tag_name")?
        .as_str()?
        .strip_prefix('v')
        .unwrap_or(response.get("tag_name")?.as_str()?);
    fn version(value: &str) -> Option<[u64; 3]> {
        let mut parts = value.split('.');
        let result = [
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ];
        if parts.next().is_some() {
            return None;
        }
        Some(result)
    }
    if version(latest)? <= version(current)? {
        return None;
    }
    Some(
        json!({"installed": current, "latest": latest, "release_url": "https://github.com/left-try/okf-agent/releases/latest", "message": "A newer okf-agent release is available. Tell the user and follow their installation method to update."}),
    )
}

#[cfg(test)]
mod tests {
    use super::notice;
    #[test]
    fn only_reports_newer_stable_versions() {
        assert_eq!(
            notice("0.2.0", r#"{"tag_name":"v0.3.0"}"#).unwrap()["latest"],
            "0.3.0"
        );
        assert!(notice("0.2.0", r#"{"tag_name":"v0.2.0"}"#).is_none());
        assert!(notice("0.2.0", r#"{"tag_name":"v0.3.0-beta.1"}"#).is_none());
        assert!(notice("0.2.0", "not json").is_none());
    }
}
