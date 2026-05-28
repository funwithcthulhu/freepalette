use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, Context};
use freepalette_daemon::{
    default_ipc_endpoint_path, read_default_ipc_endpoint, send_ipc_request, serve_ipc, DaemonState,
    HotkeyLoopStatus, IpcRequest, IpcResponse,
};

const DAEMON_START_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
enum DaemonCommand {
    Status,
    Run,
    Start,
    Stop,
    Serve { bind: Option<String> },
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
        DaemonCommand::Start => start_background_daemon()?,
        DaemonCommand::Stop => stop_background_daemon()?,
        DaemonCommand::Serve { bind } => run_ipc_daemon(daemon, bind.as_deref())?,
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

        match command.as_str() {
            "status" => {
                reject_extra_args(args)?;
                Ok(Self::Status)
            }
            "run" => {
                reject_extra_args(args)?;
                Ok(Self::Run)
            }
            "start" => {
                reject_extra_args(args)?;
                Ok(Self::Start)
            }
            "stop" => {
                reject_extra_args(args)?;
                Ok(Self::Stop)
            }
            "serve" => parse_serve_args(args),
            _ => bail!(
                "unknown daemon command '{command}'. Use 'status', 'run', 'start', 'stop', or 'serve'"
            ),
        }
    }
}

fn reject_extra_args(mut args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    if args.next().is_some() {
        bail!("too many arguments. Use 'freepalette-daemon status', 'run', 'start', 'stop', or 'serve'");
    }

    Ok(())
}

fn parse_serve_args(mut args: impl Iterator<Item = String>) -> anyhow::Result<DaemonCommand> {
    let mut bind = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                let Some(value) = args.next() else {
                    bail!("--bind requires an address such as 127.0.0.1:0");
                };
                bind = Some(value);
            }
            _ => bail!("unknown serve argument '{arg}'. Use '--bind <address>'"),
        }
    }

    Ok(DaemonCommand::Serve { bind })
}

fn print_status(daemon: &DaemonState) {
    println!("freepalette-daemon initialized");
    println!("{}", daemon.hotkey_state().summary());
    println!("IPC server is available with 'freepalette-daemon serve'");
    match daemon.local_state_path() {
        Some(path) => println!("local state: {}", path.display()),
        None => println!("local state: unavailable"),
    }
}

fn run_foreground_daemon(daemon: &DaemonState) -> anyhow::Result<()> {
    println!("freepalette-daemon running in the foreground");
    println!("Use 'freepalette-daemon serve' for local IPC");

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

fn run_ipc_daemon(daemon: DaemonState, bind: Option<&str>) -> anyhow::Result<()> {
    println!("freepalette-daemon serving local IPC");
    serve_ipc(daemon, bind).context("failed to run daemon IPC server")
}

fn start_background_daemon() -> anyhow::Result<()> {
    if let Ok(endpoint) = read_default_ipc_endpoint() {
        if send_ipc_request(&endpoint, IpcRequest::Status).is_ok() {
            println!(
                "freepalette-daemon is already running at {}",
                endpoint.address
            );
            return Ok(());
        }
    }

    if let Some(path) = default_ipc_endpoint_path() {
        let _ = std::fs::remove_file(path);
    }

    let executable = std::env::current_exe().context("failed to locate daemon executable")?;
    let mut command = Command::new(executable);
    command
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_background_process_flags(&mut command);
    let _child = command
        .spawn()
        .context("failed to start background daemon")?;

    let endpoint = wait_for_ipc_endpoint()?;
    println!("freepalette-daemon started at {}", endpoint.address);
    Ok(())
}

fn stop_background_daemon() -> anyhow::Result<()> {
    let endpoint = read_default_ipc_endpoint().context("failed to read daemon IPC endpoint")?;
    let reply = send_ipc_request(&endpoint, IpcRequest::Shutdown)
        .context("failed to send daemon shutdown request")?;
    if !reply.ok {
        bail!(
            "{}",
            reply
                .error
                .unwrap_or_else(|| "daemon refused shutdown without a message".to_string())
        );
    }
    match reply.response {
        Some(IpcResponse::ShuttingDown) => {
            println!("freepalette-daemon stopping");
            Ok(())
        }
        _ => bail!("daemon returned an unexpected shutdown response"),
    }
}

fn wait_for_ipc_endpoint() -> anyhow::Result<freepalette_daemon::IpcEndpoint> {
    let deadline = Instant::now() + DAEMON_START_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(endpoint) = read_default_ipc_endpoint() {
            if send_ipc_request(&endpoint, IpcRequest::Status).is_ok() {
                return Ok(endpoint);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    bail!("timed out waiting for daemon IPC endpoint")
}

#[cfg(windows)]
fn apply_background_process_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_background_process_flags(_command: &mut Command) {}

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
    fn daemon_command_parses_start() {
        let command =
            DaemonCommand::from_args(["freepalette-daemon".to_string(), "start".to_string()])
                .expect("start daemon args should parse");

        assert!(matches!(command, DaemonCommand::Start));
    }

    #[test]
    fn daemon_command_parses_stop() {
        let command =
            DaemonCommand::from_args(["freepalette-daemon".to_string(), "stop".to_string()])
                .expect("stop daemon args should parse");

        assert!(matches!(command, DaemonCommand::Stop));
    }

    #[test]
    fn daemon_command_parses_serve_with_bind() {
        let command = DaemonCommand::from_args([
            "freepalette-daemon".to_string(),
            "serve".to_string(),
            "--bind".to_string(),
            "127.0.0.1:4567".to_string(),
        ])
        .expect("serve daemon args should parse");

        assert!(matches!(
            command,
            DaemonCommand::Serve {
                bind: Some(address)
            } if address == "127.0.0.1:4567"
        ));
    }

    #[test]
    fn daemon_command_rejects_unknown_commands() {
        let error =
            DaemonCommand::from_args(["freepalette-daemon".to_string(), "watch".to_string()])
                .expect_err("unknown daemon command should fail");

        assert!(error.to_string().contains("unknown daemon command 'watch'"));
    }
}
