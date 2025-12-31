mod cli;
mod client;
mod config;
mod handshake;
mod input;
mod net;
mod prediction;
mod runner;
mod session;
mod state;

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::cli::{CliAction, ClientMode, DEFAULT_QPORT};
use crate::config::ClientConfig;
use crate::net::NetClient;
use crate::runner::{ClientRunner, RunnerError};
use crate::session::{Session, SessionState};
use qw_audio::{AudioConfig, AudioSystem};
use qw_common::{InfoError, InfoString, UserCmd};
use qw_renderer_gl::{GlRenderer, Renderer, RendererConfig};
use qw_window_glfw::{Action, GlfwWindow, Key, WindowConfig, WindowEvent};

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

    let mut userinfo = ClientConfig::default().userinfo;
    if let Some(name) = args.name.as_deref() {
        userinfo.set("name", name)?;
    }
    if let Some(topcolor) = args.topcolor.as_deref() {
        userinfo.set("topcolor", topcolor)?;
    }
    if let Some(bottomcolor) = args.bottomcolor.as_deref() {
        userinfo.set("bottomcolor", bottomcolor)?;
    }
    if let Some(rate) = args.rate.as_deref() {
        userinfo.set("rate", rate)?;
    }

    if args.mode == ClientMode::SinglePlayer {
        return Err(AppError::ModeNotImplemented("singleplayer"));
    }

    let server = args.server.ok_or(cli::CliError::MissingServer)?;
    let mut window = GlfwWindow::new(WindowConfig::default());
    let (width, height) = window.size();
    let mut renderer = GlRenderer::new(RendererConfig {
        width,
        height,
        ..RendererConfig::default()
    });
    let audio = AudioSystem::new(AudioConfig::default());

    let server_addr = std::net::SocketAddr::from(server.to_socket_addr());
    let net = NetClient::connect(server_addr)?;
    let qport = if args.qport == 0 {
        DEFAULT_QPORT
    } else {
        args.qport
    };
    let session = Session::new(qport, userinfo.as_str().to_string());
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
        for event in window.poll_events() {
            if handle_window_event(&mut window, &mut renderer, event, &mut pending_cmds) {
                return Ok(());
            }
        }

        if let Some(packet) = runner.poll_once(&mut buf)? {
            match packet {
                crate::client::ClientPacket::Messages(_) => {
                    for (level, message) in runner.state.prints.drain(..) {
                        println!("[{level}] {message}");
                    }
                    for message in runner.state.center_prints.drain(..) {
                        println!("[center] {message}");
                    }
                }
                crate::client::ClientPacket::OutOfBand(msg) => match msg {
                    qw_common::OobMessage::Print(text) => {
                        if !text.is_empty() {
                            println!("[oob] {text}");
                        }
                    }
                    qw_common::OobMessage::ClientCommand(text) => {
                        if !text.is_empty() {
                            println!("[oob-cmd] {text}");
                        }
                    }
                    _ => {}
                },
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
            match maybe_userinfo_command(&cmd, &mut userinfo) {
                Ok(Some(mapped)) => {
                    if runner.session.state == SessionState::Connected {
                        runner.send_string_cmd(&mapped)?;
                    } else {
                        pending_cmds.push_back(mapped);
                    }
                }
                Ok(None) => {
                    if runner.session.state == SessionState::Connected {
                        runner.send_string_cmd(&cmd)?;
                    } else {
                        pending_cmds.push_back(cmd);
                    }
                }
                Err(err) => {
                    println!("[client] invalid userinfo: {err}");
                }
            }
        }

        if audio.is_running() {
            renderer.begin_frame();
            renderer.end_frame();
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
    ModeNotImplemented(&'static str),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Cli(err) => write!(f, "{}", err),
            AppError::Runner(err) => write!(f, "{:?}", err),
            AppError::Info(err) => write!(f, "{}", err),
            AppError::Io(err) => write!(f, "{}", err),
            AppError::ModeNotImplemented(mode) => write!(f, "{} mode not implemented", mode),
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

fn maybe_userinfo_command(
    input: &str,
    userinfo: &mut InfoString,
) -> Result<Option<String>, InfoError> {
    let mut parts = input.splitn(2, ' ');
    let command = parts.next().unwrap_or("").trim();
    let rest = parts.next().unwrap_or("").trim();
    if rest.is_empty() {
        return Ok(None);
    }

    let (key, value) = match command {
        "name" | "skin" | "team" | "topcolor" | "bottomcolor" | "rate" | "msg" => (command, rest),
        "setinfo" => {
            let mut inner = rest.splitn(2, ' ');
            let key = inner.next().unwrap_or("").trim();
            let value = inner.next().unwrap_or("").trim();
            if key.is_empty() || value.is_empty() {
                return Ok(None);
            }
            if key.starts_with('*') {
                userinfo.set_star(key, value)?;
            } else {
                userinfo.set(key, value)?;
            }
            return Ok(Some(format!("setinfo {} {}", key, quote_if_needed(value))));
        }
        _ => return Ok(None),
    };

    userinfo.set(key, value)?;
    Ok(Some(format!("setinfo {} {}", key, quote_if_needed(value))))
}

fn quote_if_needed(value: &str) -> String {
    if value.contains(' ') {
        format!("\"{}\"", value)
    } else {
        value.to_string()
    }
}

fn handle_window_event(
    window: &mut GlfwWindow,
    renderer: &mut GlRenderer,
    event: WindowEvent,
    pending_cmds: &mut VecDeque<String>,
) -> bool {
    match event {
        WindowEvent::CloseRequested => {
            window.close();
            true
        }
        WindowEvent::Resized(width, height) => {
            renderer.resize(width, height);
            false
        }
        WindowEvent::Key { key, action } => {
            if let Some(cmd) = crate::input::map_key_action(key, action) {
                pending_cmds.push_back(cmd);
            }
            if key == Key::Escape && action == Action::Press {
                window.close();
                true
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_name_command_to_setinfo() {
        let mut info = InfoString::new(128);
        let cmd = maybe_userinfo_command("name player", &mut info)
            .unwrap()
            .expect("expected setinfo");
        assert_eq!(cmd, "setinfo name player");
        assert!(info.as_str().contains("\\name\\player"));
    }

    #[test]
    fn quotes_values_with_spaces() {
        let mut info = InfoString::new(128);
        let cmd = maybe_userinfo_command("name player one", &mut info)
            .unwrap()
            .expect("expected setinfo");
        assert_eq!(cmd, "setinfo name \"player one\"");
    }

    #[test]
    fn preserves_star_keys_for_setinfo() {
        let mut info = InfoString::new(128);
        let cmd = maybe_userinfo_command("setinfo *ver rq28", &mut info)
            .unwrap()
            .expect("expected setinfo");
        assert_eq!(cmd, "setinfo *ver rq28");
        assert!(info.as_str().contains("\\*ver\\rq28"));
    }

    #[test]
    fn window_close_event_triggers_exit() {
        let mut window = GlfwWindow::new(WindowConfig::default());
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let mut pending = VecDeque::new();
        let exit = handle_window_event(
            &mut window,
            &mut renderer,
            WindowEvent::CloseRequested,
            &mut pending,
        );
        assert!(exit);
        assert!(window.should_close());
    }
}
