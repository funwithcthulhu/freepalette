use std::{
    fs,
    path::PathBuf,
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

fn run_freepalette(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_freepalette"))
        .args(args)
        .output()
        .expect("freepalette CLI should run")
}

fn output_text(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
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
    assert!(output_text(&output.stderr).contains("--allow-shell"));
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
