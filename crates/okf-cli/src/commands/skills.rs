use clap::{Args, Subcommand};
use serde_json::{Value, json};
#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    command: SkillsCommand,
}

#[derive(Debug, Subcommand)]
enum SkillsCommand {
    /// Install an external skills profile through the Codex CLI.
    Apply {
        #[arg(value_parser = ["default"])]
        profile: String,
        #[arg(long, default_value = "codex", value_parser = ["codex"])]
        agent: String,
    },
    /// Inspect Codex plugin marketplaces and installed plugins.
    Doctor {
        #[arg(long, default_value = "codex", value_parser = ["codex"])]
        agent: String,
    },
}

pub fn run(args: SkillsArgs) -> i32 {
    match args.command {
        SkillsCommand::Apply { profile, agent } => apply(&profile, &agent),
        SkillsCommand::Doctor { agent } => doctor(&agent),
    }
}

fn apply(profile: &str, agent: &str) -> i32 {
    if profile != "default" {
        return emit(
            &json!({"error": format!("Unknown skills profile: {profile}")}),
            2,
        );
    }
    if agent != "codex" {
        return emit(
            &json!({"error": "The built-in profiles currently support only the Codex plugin CLI."}),
            2,
        );
    }
    if !super::process::contains("codex") {
        return emit(
            &json!({"error": "Codex CLI is not on PATH; install Codex or use `okf-agent skills doctor`."}),
            2,
        );
    }

    let plugins = [
        ("mattpocock", "mattpocock-skills", "mattpocock/skills"),
        ("ponytail", "ponytail", "DietrichGebert/ponytail"),
    ];
    let mut actions = Vec::new();
    for (marketplace, package, repository) in plugins {
        let package_spec = format!("{package}@{marketplace}");
        for args in [
            vec!["plugin", "marketplace", "add", repository],
            vec!["plugin", "add", package_spec.as_str()],
        ] {
            let command_text = args.join(" ");
            let output = match super::process::output("codex", &args) {
                Ok(output) => output,
                Err(error) => {
                    return emit(
                        &json!({"error": format!("Could not run Codex CLI: {error}")}),
                        2,
                    );
                }
            };
            let status = output.status.code().unwrap_or(1);
            actions.push(json!({
                "command": command_text,
                "status": if output.status.success() { "ok" } else { "failed" },
                "output": String::from_utf8_lossy(&output.stdout).trim(),
                "error": optional_text(&output.stderr),
            }));
            if !output.status.success() {
                return emit(
                    &json!({"error": format!("Codex command failed ({status}): {command_text}")}),
                    2,
                );
            }
        }
    }
    emit(
        &json!({
            "profile": profile,
            "agent": agent,
            "installed": ["mattpocock-skills", "ponytail"],
            "actions": actions,
            "next": "Restart Codex or start a new task to load the installed plugins. Review and trust Ponytail's lifecycle hooks in Codex before enabling them."
        }),
        0,
    )
}

fn doctor(agent: &str) -> i32 {
    if agent != "codex" {
        return emit(
            &json!({"error": "The built-in profiles currently support only the Codex plugin CLI."}),
            2,
        );
    }
    if !super::process::contains("codex") {
        return emit(
            &json!({"agent": agent, "available": false, "error": "Codex CLI is not on PATH."}),
            1,
        );
    }

    let marketplaces = run_doctor_command(&["plugin", "marketplace", "list"]);
    let plugins = run_doctor_command(&["plugin", "list"]);
    let ok =
        marketplaces["ok"].as_bool().unwrap_or(false) && plugins["ok"].as_bool().unwrap_or(false);
    emit(
        &json!({
            "agent": agent,
            "available": true,
            "marketplaces": marketplaces,
            "plugins": plugins,
            "ok": ok,
        }),
        if ok { 0 } else { 1 },
    )
}

fn run_doctor_command(args: &[&str]) -> Value {
    match super::process::output("codex", args) {
        Ok(output) => json!({
            "ok": output.status.success(),
            "output": String::from_utf8_lossy(&output.stdout).trim(),
            "error": optional_text(&output.stderr),
        }),
        Err(error) => json!({"ok": false, "output": "", "error": error.to_string()}),
    }
}

fn optional_text(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn emit(value: &Value, code: i32) -> i32 {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
    code
}
