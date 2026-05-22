use anyhow::{bail, Context};
use freepalette_daemon::{DaemonState, HotkeyLoopStatus};

#[derive(Debug)]
enum DaemonCommand {
    Status,
    Run,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();

    let command = DaemonCommand::from_args(std::env::args())?;
    let daemon = DaemonState::from_default_config()?;

    tracing::info!(
        providers = ?daemon.provider_ids(),
        clipboard_items = daemon.clipboard_history_len(),
        hotkey = %daemon.hotkey_state().summary(),
        "daemon initialized"
    );
    match command {
        DaemonCommand::Status => print_status(&daemon),
        DaemonCommand::Run => run_foreground_daemon(&daemon)?,
    }

    Ok(())
}

impl DaemonCommand {
    fn from_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut args = args.into_iter();
        let _program = args.next();
        let Some(command) = args.next() else {
            return Ok(Self::Status);
        };

        if args.next().is_some() {
            bail!("too many arguments. Use 'freepalette-daemon' or 'freepalette-daemon run'");
        }

        match command.as_str() {
            "status" => Ok(Self::Status),
            "run" => Ok(Self::Run),
            _ => bail!("unknown daemon command '{command}'. Use 'status' or 'run'"),
        }
    }
}

fn print_status(daemon: &DaemonState) {
    println!("freepalette-daemon initialized");
    println!("{}", daemon.hotkey_state().summary());
    println!("IPC and system clipboard capture are intentionally not implemented yet");
}

fn run_foreground_daemon(daemon: &DaemonState) -> anyhow::Result<()> {
    println!("freepalette-daemon running in the foreground");
    println!("IPC and system clipboard capture are intentionally not implemented yet");

    match daemon
        .run_hotkey_loop()
        .context("failed to run global hotkey loop")?
    {
        HotkeyLoopStatus::Disabled => {
            println!("global hotkey disabled; foreground daemon exited");
        }
        HotkeyLoopStatus::UnsupportedPlatform { platform } => {
            println!("global hotkey unsupported on {platform}; foreground daemon exited");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_command_defaults_to_status() {
        let command = DaemonCommand::from_args(["freepalette-daemon".to_string()])
            .expect("empty daemon args should parse");

        assert!(matches!(command, DaemonCommand::Status));
    }

    #[test]
    fn daemon_command_parses_run() {
        let command =
            DaemonCommand::from_args(["freepalette-daemon".to_string(), "run".to_string()])
                .expect("run daemon args should parse");

        assert!(matches!(command, DaemonCommand::Run));
    }

    #[test]
    fn daemon_command_rejects_unknown_commands() {
        let error =
            DaemonCommand::from_args(["freepalette-daemon".to_string(), "watch".to_string()])
                .expect_err("unknown daemon command should fail");

        assert!(error.to_string().contains("unknown daemon command 'watch'"));
    }
}
