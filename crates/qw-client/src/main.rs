mod cli;
mod client;
mod config;
mod handshake;
mod net;
mod prediction;
mod runner;
mod session;
mod state;

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::cli::{CliAction, DEFAULT_QPORT};
use crate::config::ClientConfig;
use crate::net::NetClient;
use crate::runner::{ClientRunner, RunnerError};
use crate::session::{Session, SessionState};
use qw_common::UserCmd;

const MOVE_INTERVAL_MS: u64 = 50;

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let action = cli::parse_args(std::env::args().skip(1))?;
    let args = match action {
        CliAction::Help => {
            println!("{}", cli::usage());
            return Ok(());
        }
        CliAction::Run(args) => args,
    };

    if let Some(path) = args.data_dir.as_deref() {
        // Safety: env var mutation is process-global; caller owns process.
        unsafe {
            std::env::set_var("RUSTQUAKE_DATA_DIR", path);
        }
    }
    if let Some(path) = args.download_dir.as_deref() {
        // Safety: env var mutation is process-global; caller owns process.
        unsafe {
            std::env::set_var("RUSTQUAKE_DOWNLOAD_DIR", path);
        }
    }

    let mut config = ClientConfig::default();
    if let Some(name) = args.name.as_deref() {
        config.userinfo.set("name", name)?;
    }
    if let Some(topcolor) = args.topcolor.as_deref() {
        config.userinfo.set("topcolor", topcolor)?;
    }
    if let Some(bottomcolor) = args.bottomcolor.as_deref() {
        config.userinfo.set("bottomcolor", bottomcolor)?;
    }
    if let Some(rate) = args.rate.as_deref() {
        config.userinfo.set("rate", rate)?;
    }

    let server_addr = std::net::SocketAddr::from(args.server.to_socket_addr());
    let net = NetClient::connect(server_addr)?;
    let qport = if args.qport == 0 {
        DEFAULT_QPORT
    } else {
        args.qport
    };
    let session = Session::new(qport, config.userinfo.as_str().to_string());
    let mut runner = ClientRunner::new(net, session);
    runner.start_connect()?;

    let mut last_move = Instant::now();
    let mut buf = [0u8; 8192];
    let mut was_connected = false;
    let mut pending_cmds: VecDeque<String> = VecDeque::new();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            if stdin.read_line(&mut line).is_err() {
                break;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if tx.send(trimmed.to_string()).is_err() {
                break;
            }
        }
    });
    loop {
        if let Some(crate::client::ClientPacket::Messages(_)) = runner.poll_once(&mut buf)? {
            for (level, message) in runner.state.prints.drain(..) {
                println!("[{level}] {message}");
            }
            for message in runner.state.center_prints.drain(..) {
                println!("[center] {message}");
            }
        }

        if runner.session.state == SessionState::Connected {
            was_connected = true;
            while let Some(cmd) = pending_cmds.pop_front() {
                runner.send_string_cmd(&cmd)?;
            }
            let elapsed = last_move.elapsed();
            if elapsed >= Duration::from_millis(MOVE_INTERVAL_MS) {
                let mut cmd = UserCmd::default();
                let msec = elapsed.as_millis().min(u128::from(u8::MAX)) as u8;
                cmd.msec = msec;
                cmd.angles = runner.state.view_angles;
                runner.send_move(cmd)?;
                last_move = Instant::now();
            }
        } else if runner.session.state == SessionState::Disconnected && was_connected {
            break;
        }

        while let Ok(cmd) = rx.try_recv() {
            if cmd.eq_ignore_ascii_case("quit") || cmd.eq_ignore_ascii_case("exit") {
                return Ok(());
            }
            if runner.session.state == SessionState::Connected {
                runner.send_string_cmd(&cmd)?;
            } else {
                pending_cmds.push_back(cmd);
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    Ok(())
}

#[derive(Debug)]
enum AppError {
    Cli(cli::CliError),
    Runner(RunnerError),
    Info(qw_common::InfoError),
    Io(std::io::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Cli(err) => write!(f, "{}", err),
            AppError::Runner(err) => write!(f, "{:?}", err),
            AppError::Info(err) => write!(f, "{}", err),
            AppError::Io(err) => write!(f, "{}", err),
        }
    }
}

impl From<cli::CliError> for AppError {
    fn from(err: cli::CliError) -> Self {
        AppError::Cli(err)
    }
}

impl From<RunnerError> for AppError {
    fn from(err: RunnerError) -> Self {
        AppError::Runner(err)
    }
}

impl From<qw_common::InfoError> for AppError {
    fn from(err: qw_common::InfoError) -> Self {
        AppError::Info(err)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}
