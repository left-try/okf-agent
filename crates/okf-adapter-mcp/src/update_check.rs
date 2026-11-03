use serde_json::{Value, json};
use std::sync::{Arc, RwLock};

pub type UpdateNotice = Arc<RwLock<Option<Value>>>;

/// Poll in the background so an unavailable network never delays MCP calls.
pub fn start(state: UpdateNotice) {
    if std::env::var_os("OKF_DISABLE_UPDATE_CHECK").is_some() {
        return;
    }
    std::thread::spawn(move || {
        loop {
            let result = std::process::Command::new("curl")
                .args([
                    "--fail",
                    "--silent",
                    "--show-error",
                    "--max-time",
                    "3",
                    "--user-agent",
                    "okf-agent-update-check",
                    "https://api.github.com/repos/left-try/okf-agent/releases/latest",
                ])
                .output();
            if let Ok(output) = result
                && output.status.success()
                && let Ok(text) = String::from_utf8(output.stdout)
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
