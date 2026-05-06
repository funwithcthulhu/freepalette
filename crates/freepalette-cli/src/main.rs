use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use freepalette_core::{builtin_registry, Action, Config, ProviderRegistry, RankedResult};

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
        #[arg(short, long)]
        json: bool,
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// List registered providers.
    Providers,
    /// Print the default config path for this platform.
    ConfigPath,
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
                run_first_result(&registry, &results)?;
            }
        }
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

fn run_first_result(registry: &ProviderRegistry, results: &[RankedResult]) -> anyhow::Result<()> {
    let Some(first) = results.first() else {
        println!("Nothing to run");
        return Ok(());
    };

    let outcome = registry.execute(&first.result)?;
    println!("{}", outcome.message);
    Ok(())
}
