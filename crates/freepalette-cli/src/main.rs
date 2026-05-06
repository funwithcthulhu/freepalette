use std::path::PathBuf;

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use freepalette_core::{
    builtin_registry, Action, AppIndexEntry, AppIndexEntrySource, AppIndexReport,
    AppLauncherProvider, Config, ProviderRegistry, RankedResult,
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
            let config = load_config(cli.config.as_ref())?;
            let registry = builtin_registry(&config)?;
            let limit = limit.unwrap_or(config.general.max_results);
            let results = registry.search(&query, limit)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                print_results(&results);
            }

            if run {
                run_first_result(&registry, &results, allow_shell)?;
            }
        }
        Commands::Run {
            query,
            allow_shell,
            limit,
        } => {
            let config = load_config(cli.config.as_ref())?;
            let registry = builtin_registry(&config)?;
            let limit = limit.unwrap_or(config.general.max_results);
            let results = registry.search(&query, limit)?;
            run_first_result(&registry, &results, allow_shell)?;
        }
        Commands::Apps { command } => match command {
            AppsCommand::List { json } => {
                let config = load_config(cli.config.as_ref())?;
                let report = app_index_report(&config);
                print_app_report(&report, json)?;
            }
        },
        Commands::Debug { command } => match command {
            DebugCommand::Apps { json } => {
                let config = load_config(cli.config.as_ref())?;
                let report = app_index_report(&config);
                print_app_report(&report, json)?;
            }
        },
        Commands::Providers => {
            let config = load_config(cli.config.as_ref())?;
            let registry = builtin_registry(&config)?;
            for provider_id in registry.provider_ids() {
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

fn load_config(path: Option<&PathBuf>) -> anyhow::Result<Config> {
    match path {
        Some(path) => Config::load_from_path(path)
            .with_context(|| format!("failed to load config from {}", path.display())),
        None => Config::load_default_or_default().context("failed to load default config"),
    }
}

fn app_index_report(config: &Config) -> AppIndexReport {
    AppLauncherProvider::from_config(config).index_report()
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
    registry: &ProviderRegistry,
    results: &[RankedResult],
    allow_shell: bool,
) -> anyhow::Result<()> {
    let Some(first) = results.first() else {
        println!("Nothing to run");
        return Ok(());
    };

    ensure_action_allowed(&first.result.action, allow_shell)?;
    println!(
        "Running: [{}] {}",
        first.result.provider, first.result.title
    );
    let outcome = registry.execute(&first.result)?;
    println!("{}", outcome.message);
    Ok(())
}

fn ensure_action_allowed(action: &Action, allow_shell: bool) -> anyhow::Result<()> {
    if matches!(action, Action::RunShell { .. }) && !allow_shell {
        bail!("refusing to run shell command without --allow-shell");
    }

    Ok(())
}

fn print_app_report(report: &AppIndexReport, json: bool) -> anyhow::Result<()> {
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
    fn shell_actions_require_allow_shell() {
        let action = Action::RunShell {
            command: "echo hello".to_string(),
        };

        assert!(ensure_action_allowed(&action, false).is_err());
        assert!(ensure_action_allowed(&action, true).is_ok());
    }

    #[test]
    fn non_shell_actions_do_not_require_allow_shell() {
        let action = Action::LaunchApp {
            command: "notepad.exe".to_string(),
            args: Vec::new(),
        };

        assert!(ensure_action_allowed(&action, false).is_ok());
    }
}
