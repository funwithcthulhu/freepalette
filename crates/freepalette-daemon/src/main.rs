use anyhow::Context;
use freepalette_core::Config;
use freepalette_daemon::DaemonState;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let config = Config::load_default_or_default().context("failed to load default config")?;
    let daemon = DaemonState::new(config).context("failed to initialize daemon state")?;

    tracing::info!(
        providers = ?daemon.provider_ids(),
        clipboard_items = daemon.clipboard_history_len(),
        "daemon initialized"
    );
    println!("freepalette-daemon initialized");
    println!("global hotkey, app indexing, and clipboard capture are intentionally stubbed");

    Ok(())
}
