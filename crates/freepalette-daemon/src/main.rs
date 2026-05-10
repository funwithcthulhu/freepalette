use freepalette_daemon::DaemonState;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let daemon = DaemonState::from_default_config()?;

    tracing::info!(
        providers = ?daemon.provider_ids(),
        clipboard_items = daemon.clipboard_history_len(),
        hotkey = %daemon.hotkey_state().summary(),
        "daemon initialized"
    );
    println!("freepalette-daemon initialized");
    println!("{}", daemon.hotkey_state().summary());
    println!("IPC and system clipboard capture are intentionally not implemented yet");

    Ok(())
}
