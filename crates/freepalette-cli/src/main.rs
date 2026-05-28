use std::path::PathBuf;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use freepalette_core::{
    Action, AppIndexEntry, AppIndexEntrySource, AppIndexReport, Config, RankedResult,
};
use freepalette_daemon::{
    read_default_ipc_endpoint, send_ipc_request, ActionExecutionPolicy, DaemonError, DaemonState,
    IpcReply, IpcRequest, IpcResponse,
};

#[derive(Debug, Parser)]
#[command(name = "freepalette")]
#[command(about = "Local-first command palette CLI for provider and search testing.")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Search registered providers without opening a GUI.
    Search {
        query: String,
        #[arg(short, long)]
        run: bool,
        #[arg(long)]
        allow_shell: bool,
        #[arg(short, long)]
        json: bool,
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Run the top ranked result for a query.
    Run {
        query: String,
        #[arg(long)]
        allow_shell: bool,
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Inspect indexed applications.
    Apps {
        #[command(subcommand)]
        command: AppsCommand,
    },
    /// Debug provider state.
    Debug {
        #[command(subcommand)]
        command: DebugCommand,
    },
    /// Talk to a running local daemon IPC server.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    /// List registered providers.
    Providers,
    /// Print the default config path for this platform.
    ConfigPath,
}

#[derive(Debug, Subcommand)]
enum AppsCommand {
    /// List app provider entries and indexing status.
    List {
        #[arg(short, long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DebugCommand {
    /// Print app provider indexing status and entries.
    Apps {
        #[arg(short, long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    /// Print status from a running daemon.
    Status {
        #[arg(short, long)]
        json: bool,
    },
    /// Search through a running daemon.
    Search {
        query: String,
        #[arg(short, long)]
        json: bool,
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Run the top ranked result through a running daemon.
    Run {
        query: String,
        #[arg(long)]
        allow_shell: bool,
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Refresh app indexing in a running daemon.
    RefreshApps {
        #[arg(short, long)]
        json: bool,
    },
    /// Stop a running daemon IPC server.
    Stop,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Search {
            query,
            run,
            allow_shell,
            json,
            limit,
        } => {
            let mut daemon = load_daemon(cli.config.as_ref())?;
            let results = daemon.search(&query, limit)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                print_results(&results);
            }

            if run {
                run_first_result(&mut daemon, &results, execution_policy(allow_shell))?;
            }
        }
        Commands::Run {
            query,
            allow_shell,
            limit,
        } => {
            let mut daemon = load_daemon(cli.config.as_ref())?;
            let results = daemon.search(&query, limit)?;
            run_first_result(&mut daemon, &results, execution_policy(allow_shell))?;
        }
        Commands::Apps { command } => match command {
            AppsCommand::List { json } => {
                let daemon = load_daemon(cli.config.as_ref())?;
                print_app_report(daemon.app_index_report(), json)?;
            }
        },
        Commands::Debug { command } => match command {
            DebugCommand::Apps { json } => {
                let daemon = load_daemon(cli.config.as_ref())?;
                print_app_report(daemon.app_index_report(), json)?;
            }
        },
        Commands::Daemon { command } => run_daemon_ipc_command(command)?,
        Commands::Providers => {
            let daemon = load_daemon(cli.config.as_ref())?;
            for provider_id in daemon.provider_ids() {
                println!("{provider_id}");
            }
        }
        Commands::ConfigPath => match Config::default_path() {
            Some(path) => println!("{}", path.display()),
            None => println!("default config path is unavailable on this platform"),
        },
    }

    Ok(())
}

fn run_daemon_ipc_command(command: DaemonCommand) -> anyhow::Result<()> {
    let endpoint = read_default_ipc_endpoint().context("failed to read daemon IPC endpoint")?;
    match command {
        DaemonCommand::Status { json } => {
            let reply = send_ipc_request(&endpoint, IpcRequest::Status)
                .context("daemon status request failed")?;
            let response = require_ipc_response(reply)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                print_daemon_status(response)?;
            }
        }
        DaemonCommand::Search { query, json, limit } => {
            let reply = send_ipc_request(&endpoint, IpcRequest::Search { query, limit })
                .context("daemon search request failed")?;
            let response = require_ipc_response(reply)?;
            match response {
                IpcResponse::Search { results } => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&results)?);
                    } else {
                        print_results(&results);
                    }
                }
                _ => bail!("daemon returned an unexpected search response"),
            }
        }
        DaemonCommand::Run {
            query,
            allow_shell,
            limit,
        } => {
            let reply = send_ipc_request(
                &endpoint,
                IpcRequest::Execute {
                    query,
                    limit,
                    allow_shell,
                },
            )
            .context("daemon run request failed")?;
            let response = require_ipc_response(reply)?;
            match response {
                IpcResponse::Executed {
                    provider,
                    title,
                    message,
                } => {
                    if provider.is_empty() {
                        println!("{message}");
                    } else {
                        println!("Running: [{provider}] {title}");
                        println!("{message}");
                    }
                }
                _ => bail!("daemon returned an unexpected execution response"),
            }
        }
        DaemonCommand::RefreshApps { json } => {
            let reply = send_ipc_request(&endpoint, IpcRequest::RefreshAppIndex)
                .context("daemon app index refresh request failed")?;
            let response = require_ipc_response(reply)?;
            match response {
                IpcResponse::AppIndexRefreshed { report } => {
                    print_app_report(report.as_ref(), json)?;
                }
                _ => bail!("daemon returned an unexpected app index refresh response"),
            }
        }
        DaemonCommand::Stop => {
            let reply = send_ipc_request(&endpoint, IpcRequest::Shutdown)
                .context("daemon shutdown request failed")?;
            let response = require_ipc_response(reply)?;
            match response {
                IpcResponse::ShuttingDown => println!("daemon stopping"),
                _ => bail!("daemon returned an unexpected shutdown response"),
            }
        }
    }

    Ok(())
}

fn require_ipc_response(reply: IpcReply) -> anyhow::Result<IpcResponse> {
    if reply.ok {
        return reply
            .response
            .ok_or_else(|| anyhow::anyhow!("daemon returned an empty success response"));
    }

    let error = reply
        .error
        .unwrap_or_else(|| "daemon request failed without an error message".to_string());
    bail!("{error}")
}

fn print_daemon_status(response: IpcResponse) -> anyhow::Result<()> {
    let IpcResponse::Status {
        providers,
        clipboard_history_len,
        recent_result_count,
        hotkey,
        local_state_path,
        ..
    } = response
    else {
        bail!("daemon returned an unexpected status response");
    };

    println!("providers: {}", providers.join(", "));
    println!("clipboard items: {clipboard_history_len}");
    println!("recent actions: {recent_result_count}");
    println!("hotkey: {hotkey}");
    match local_state_path {
        Some(path) => println!("local state: {path}"),
        None => println!("local state: unavailable"),
    }
    Ok(())
}

fn load_daemon(path: Option<&PathBuf>) -> anyhow::Result<DaemonState> {
    match path {
        Some(path) => DaemonState::load_from_path(path)
            .with_context(|| format!("failed to load config from {}", path.display())),
        None => DaemonState::from_default_config().context("failed to load default config"),
    }
}

fn print_results(results: &[RankedResult]) {
    if results.is_empty() {
        println!("No results");
        return;
    }

    for (index, ranked) in results.iter().enumerate() {
        let result = &ranked.result;
        let subtitle = result.subtitle.as_deref().unwrap_or("");
        println!(
            "{:>2}. [{}] {}{} (score {})",
            index + 1,
            result.provider,
            result.title,
            format_subtitle(subtitle),
            ranked.score
        );
        println!("    action: {}", describe_action(&result.action));
    }
}

fn format_subtitle(subtitle: &str) -> String {
    if subtitle.is_empty() {
        String::new()
    } else {
        format!(" - {subtitle}")
    }
}

fn describe_action(action: &Action) -> String {
    match action {
        Action::LaunchApp { command, args } => {
            format!("launch app: {}", format_command_with_args(command, args))
        }
        Action::OpenPath { path } => format!("open path: {path}"),
        Action::RunShell { command } => format!("run shell command: {command}"),
        Action::CopyText { text } => format!("copy text: {text}"),
        Action::Noop { message } => format!("no-op: {message}"),
    }
}

fn format_command_with_args(command: &str, args: &[String]) -> String {
    let command = format_arg_for_display(command);
    if args.is_empty() {
        command
    } else {
        let formatted_args = args
            .iter()
            .map(|arg| format_arg_for_display(arg))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{command} {formatted_args}")
    }
}

fn format_arg_for_display(arg: &str) -> String {
    if arg.chars().any(char::is_whitespace) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

fn run_first_result(
    daemon: &mut DaemonState,
    results: &[RankedResult],
    policy: ActionExecutionPolicy,
) -> anyhow::Result<()> {
    let Some(first) = results.first() else {
        println!("Nothing to run");
        return Ok(());
    };

    let outcome = match daemon.execute_result(&first.result, policy) {
        Ok(outcome) => outcome,
        Err(DaemonError::ShellCommandBlocked) => {
            bail!("refusing to run shell command without --allow-shell")
        }
        Err(error) => return Err(error.into()),
    };
    println!(
        "Running: [{}] {}",
        first.result.provider, first.result.title
    );
    println!("{}", outcome.message);
    Ok(())
}

fn execution_policy(allow_shell: bool) -> ActionExecutionPolicy {
    if allow_shell {
        ActionExecutionPolicy::AllowShellCommands
    } else {
        ActionExecutionPolicy::BlockShellCommands
    }
}

fn print_app_report(report: Option<&AppIndexReport>, json: bool) -> anyhow::Result<()> {
    let Some(report) = report else {
        print_disabled_app_report(json)?;
        return Ok(());
    };

    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("status: {}", report.summary);
    println!("apps: {}", report.entries.len());

    for (index, entry) in report.entries.iter().enumerate() {
        println!(
            "{:>3}. {} - {}",
            index + 1,
            entry.name,
            describe_app_entry(entry)
        );
        println!("     source: {}", describe_app_source(entry));
    }

    Ok(())
}

fn print_disabled_app_report(json: bool) -> anyhow::Result<()> {
    if json {
        let output = serde_json::json!({
            "summary": "app provider is disabled",
            "status": {
                "state": "disabled"
            },
            "entries": []
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("status: app provider is disabled");
        println!("apps: 0");
    }

    Ok(())
}

fn describe_app_entry(entry: &AppIndexEntry) -> String {
    format_command_with_args(&entry.command, &entry.args)
}

fn describe_app_source(entry: &AppIndexEntry) -> String {
    let source = match entry.source {
        AppIndexEntrySource::Config => "config",
        AppIndexEntrySource::Known => "known",
        AppIndexEntrySource::WindowsStartMenu => "windows-start-menu",
        AppIndexEntrySource::Fallback => "fallback",
    };

    match entry.source_detail.as_deref() {
        Some(detail) => format!("{source} ({detail})"),
        None => source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_execution_policy_requires_explicit_allow_flag() {
        assert_eq!(
            execution_policy(false),
            ActionExecutionPolicy::BlockShellCommands
        );
        assert_eq!(
            execution_policy(true),
            ActionExecutionPolicy::AllowShellCommands
        );
    }

    #[test]
    fn disabled_app_report_can_be_printed() {
        print_app_report(None, false).expect("disabled app report should print");
    }

    #[test]
    fn app_action_preview_keeps_command_and_args_readable() {
        let action = Action::LaunchApp {
            command: r"C:\Program Files\Example App\app.exe".to_string(),
            args: vec!["--profile".to_string(), "German News".to_string()],
        };

        assert_eq!(
            describe_action(&action),
            r#"launch app: "C:\Program Files\Example App\app.exe" --profile "German News""#
        );
    }

    #[test]
    fn daemon_refresh_apps_command_parses() {
        let cli = Cli::try_parse_from(["freepalette", "daemon", "refresh-apps", "--json"])
            .expect("daemon refresh-apps command should parse");

        assert!(matches!(
            cli.command,
            Commands::Daemon {
                command: DaemonCommand::RefreshApps { json: true }
            }
        ));
    }
}
