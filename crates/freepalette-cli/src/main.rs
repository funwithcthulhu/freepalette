use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use freepalette_core::{
    Action, AppIndexEntry, AppIndexEntrySource, AppIndexReport, Config, RankedResult,
};
use freepalette_daemon::{ActionExecutionPolicy, DaemonState};

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
            let daemon = load_daemon(cli.config.as_ref())?;
            let results = daemon.search(&query, limit)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                print_results(&results);
            }

            if run {
                run_first_result(&daemon, &results, execution_policy(allow_shell))?;
            }
        }
        Commands::Run {
            query,
            allow_shell,
            limit,
        } => {
            let daemon = load_daemon(cli.config.as_ref())?;
            let results = daemon.search(&query, limit)?;
            run_first_result(&daemon, &results, execution_policy(allow_shell))?;
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
        Action::LaunchApp { command, args } if args.is_empty() => format!("launch app: {command}"),
        Action::LaunchApp { command, args } => format!("launch app: {command} {}", args.join(" ")),
        Action::RunShell { command } => format!("run shell command: {command}"),
        Action::CopyText { text } => format!("copy text: {text}"),
        Action::Noop { message } => format!("no-op: {message}"),
    }
}

fn run_first_result(
    daemon: &DaemonState,
    results: &[RankedResult],
    policy: ActionExecutionPolicy,
) -> anyhow::Result<()> {
    let Some(first) = results.first() else {
        println!("Nothing to run");
        return Ok(());
    };

    println!(
        "Running: [{}] {}",
        first.result.provider, first.result.title
    );
    let outcome = daemon.execute_result(&first.result, policy)?;
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
    if entry.args.is_empty() {
        entry.command.clone()
    } else {
        format!("{} {}", entry.command, entry.args.join(" "))
    }
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
}
