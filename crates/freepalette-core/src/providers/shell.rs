use std::process::Command;

use freepalette_plugin_api::{
    Action, ActionOutcome, PluginError, Provider, ProviderId, ResultKind, SearchContext,
    SearchResult,
};

const SHELL_SCORE_HINT: i64 = 900;

pub struct ShellCommandProvider;

impl Provider for ShellCommandProvider {
    fn id(&self) -> ProviderId {
        ProviderId::from("shell")
    }

    fn search(&self, context: &SearchContext) -> Result<Vec<SearchResult>, PluginError> {
        let Some(command) = shell_command_from_query(context.query.raw()) else {
            return Ok(Vec::new());
        };

        Ok(vec![SearchResult::new(
            self.id(),
            format!("shell:{command}"),
            format!("Run: {command}"),
            ResultKind::Shell,
            Action::RunShell {
                command: command.to_string(),
            },
        )
        .with_subtitle("Shell command")
        .with_keywords(vec![
            ">".to_string(),
            "shell".to_string(),
            "command".to_string(),
        ])
        .with_score_hint(SHELL_SCORE_HINT)])
    }

    fn execute(&self, action: &Action) -> Result<ActionOutcome, PluginError> {
        match action {
            Action::RunShell { command } => run_shell_command(command),
            _ => Err(PluginError::UnsupportedAction),
        }
    }
}

fn shell_command_from_query(query: &str) -> Option<&str> {
    let trimmed = query.trim_start();
    trimmed
        .strip_prefix('>')
        .map(str::trim)
        .filter(|command| !command.is_empty())
}

fn run_shell_command(command: &str) -> Result<ActionOutcome, PluginError> {
    let output = shell_program()
        .args(shell_args(command))
        .output()
        .map_err(|source| PluginError::Action(format!("failed to start shell: {source}")))?;

    let status = output.status.code().map_or_else(
        || "terminated by signal".to_string(),
        |code| code.to_string(),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let summary = if output.status.success() {
        first_non_empty_line(&stdout).unwrap_or("command completed")
    } else {
        first_non_empty_line(&stderr).unwrap_or("command failed")
    };

    Ok(ActionOutcome::new(format!(
        "shell exited with {status}: {summary}"
    )))
}

#[cfg(target_os = "windows")]
fn shell_program() -> Command {
    Command::new("cmd")
}

#[cfg(not(target_os = "windows"))]
fn shell_program() -> Command {
    Command::new("sh")
}

#[cfg(target_os = "windows")]
fn shell_args(command: &str) -> [&str; 2] {
    ["/C", command]
}

#[cfg(not(target_os = "windows"))]
fn shell_args(command: &str) -> [&str; 2] {
    ["-c", command]
}

fn first_non_empty_line(output: &str) -> Option<&str> {
    output.lines().map(str::trim).find(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shell_prefix() {
        assert_eq!(shell_command_from_query("> echo hello"), Some("echo hello"));
        assert_eq!(shell_command_from_query("   > pwd"), Some("pwd"));
        assert_eq!(shell_command_from_query(">"), None);
        assert_eq!(shell_command_from_query(">   "), None);
        assert_eq!(shell_command_from_query("echo hello"), None);
    }

    #[test]
    fn provider_returns_result_for_shell_query() {
        let provider = ShellCommandProvider;
        let results = provider
            .search(&SearchContext::new("> echo hello", 10))
            .expect("shell search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Run: echo hello");
    }
}
