use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_config_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("freepalette-cli-{name}-{unique}.toml"))
}

struct TempConfig {
    path: PathBuf,
}

impl TempConfig {
    fn path_arg(&self) -> String {
        self.path.display().to_string()
    }
}

impl Drop for TempConfig {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn write_config(name: &str, contents: &str) -> TempConfig {
    let path = temp_config_path(name);
    fs::write(&path, contents).expect("test config should be writable");
    TempConfig { path }
}

struct TempMarker {
    path: PathBuf,
}

impl TempMarker {
    fn new(name: &str) -> Self {
        Self {
            path: temp_config_path(name),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn run_freepalette(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_freepalette"))
        .args(args)
        .output()
        .expect("freepalette CLI should run")
}

fn output_text(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
}

#[cfg(target_os = "windows")]
fn shell_write_marker_command(path: &Path) -> String {
    format!("echo shell-ran > \"{}\"", path.display())
}

#[cfg(not(target_os = "windows"))]
fn shell_write_marker_command(path: &Path) -> String {
    let quoted = path.display().to_string().replace('\'', "'\\''");
    format!("printf shell-ran > '{quoted}'")
}

#[test]
fn search_calculator_query_prints_result() {
    let config = write_config(
        "calculator",
        r#"
            [providers]
            apps = false
            calculator = true
            shell = false
            clipboard = false
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "search", "calc 2+2"]);

    assert!(output.status.success());
    assert!(output_text(&output.stdout).contains("2+2 = 4"));
}

#[test]
fn search_shell_query_prints_action_without_running() {
    let config = write_config(
        "shell-search",
        r#"
            [providers]
            apps = false
            calculator = false
            shell = true
            clipboard = false
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "search", "> echo hello"]);

    assert!(output.status.success());
    let stdout = output_text(&output.stdout);
    assert!(stdout.contains("Run: echo hello"));
    assert!(stdout.contains("action: run shell command: echo hello"));
}

#[test]
fn search_shell_query_does_not_execute_command() {
    let config = write_config(
        "shell-search-no-execute",
        r#"
            [providers]
            apps = false
            calculator = false
            shell = true
            clipboard = false
        "#,
    );
    let marker = TempMarker::new("shell-search-marker");
    let config_arg = config.path_arg();
    let query = format!("> {}", shell_write_marker_command(marker.path()));

    let output = run_freepalette(&["--config", &config_arg, "search", &query]);

    assert!(output.status.success());
    assert!(
        !marker.path().exists(),
        "plain search must not execute shell actions"
    );
    assert!(output_text(&output.stdout).contains("action: run shell command:"));
}

#[test]
fn run_shell_query_requires_allow_shell() {
    let config = write_config(
        "shell-run",
        r#"
            [providers]
            apps = false
            calculator = false
            shell = true
            clipboard = false
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "run", "> echo hello"]);

    assert!(!output.status.success());
    assert!(!output_text(&output.stdout).contains("Running:"));
    assert!(
        output_text(&output.stderr).contains("refusing to run shell command without --allow-shell")
    );
}

#[test]
fn empty_config_keeps_shell_execution_blocked_by_default() {
    let config = write_config("empty-shell-default", "");
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "run", "> echo hello"]);

    assert!(!output.status.success());
    assert!(!output_text(&output.stdout).contains("Running:"));
    assert!(
        output_text(&output.stderr).contains("refusing to run shell command without --allow-shell")
    );
}

#[test]
fn search_run_shell_query_requires_allow_shell() {
    let config = write_config(
        "shell-search-run",
        r#"
            [providers]
            apps = false
            calculator = false
            shell = true
            clipboard = false
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "search", "--run", "> echo hello"]);

    assert!(!output.status.success());
    assert!(output_text(&output.stdout).contains("Run: echo hello"));
    assert!(
        output_text(&output.stderr).contains("refusing to run shell command without --allow-shell")
    );
}

#[test]
fn run_calculator_query_is_not_blocked_by_shell_guard() {
    let config = write_config(
        "calculator-run",
        r#"
            [providers]
            apps = false
            calculator = true
            shell = true
            clipboard = false
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "run", "calc 2+2"]);

    assert!(output.status.success());
    let stdout = output_text(&output.stdout);
    assert!(stdout.contains("Running: [calculator] 2+2 = 4"));
    assert!(stdout.contains("calculator result ready to copy: 4"));
    assert!(!output_text(&output.stderr).contains("--allow-shell"));
}

#[test]
fn run_stale_app_entry_fails_before_launching() {
    let missing_app = TempMarker::new("missing-app-target");
    let missing_app_path = missing_app.path().display().to_string();
    let escaped_missing_app_path = missing_app_path.replace('\\', "\\\\");
    let config_text = format!(
        r#"
            [providers]
            apps = true
            calculator = false
            shell = true
            clipboard = false

            [[apps]]
            name = "Stale App"
            command = "{escaped_missing_app_path}"
        "#
    );
    let config = write_config("stale-app-run", &config_text);
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "run", "stale"]);

    assert!(!output.status.success());
    assert!(!output_text(&output.stdout).contains("Running:"));
    let stderr = output_text(&output.stderr);
    assert!(stderr.contains("app target does not exist"));
    assert!(stderr.contains(&missing_app_path));
    assert!(
        !missing_app.path().exists(),
        "stale app test path must remain absent"
    );
}

#[test]
fn providers_command_accepts_unknown_config_keys() {
    let config = write_config(
        "unknown-keys",
        r#"
            unknown_root_key = "ignored"

            [providers]
            apps = false
            calculator = false
            shell = true
            clipboard = false
            unknown_provider_key = true
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "providers"]);

    assert!(output.status.success());
    assert_eq!(output_text(&output.stdout), "shell\n");
}

#[test]
fn invalid_config_file_fails_with_parse_context() {
    let config = write_config("invalid", "[providers\nshell = true");
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "providers"]);

    assert!(!output.status.success());
    let stderr = output_text(&output.stderr);
    assert!(stderr.contains("failed to load config from"));
    assert!(stderr.contains("failed to parse config at"));
}

#[test]
fn apps_list_reports_disabled_app_provider() {
    let config = write_config(
        "apps-disabled",
        r#"
            [providers]
            apps = false
            calculator = false
            shell = false
            clipboard = false
        "#,
    );
    let config_arg = config.path_arg();

    let output = run_freepalette(&["--config", &config_arg, "apps", "list"]);

    assert!(output.status.success());
    assert!(output_text(&output.stdout).contains("app provider is disabled"));
}
