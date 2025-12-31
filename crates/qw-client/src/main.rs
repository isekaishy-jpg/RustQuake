mod cli;
mod client;
mod config;
mod handshake;
mod input;
mod model_cache;
mod net;
mod prediction;
mod runner;
mod session;
mod sound;
mod state;

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::cli::{CliAction, ClientMode, DEFAULT_QPORT};
use crate::config::ClientConfig;
use crate::input::{CommandTarget, InputBindings, InputState};
use crate::net::NetClient;
use crate::runner::{ClientRunner, RunnerError};
use crate::session::{Session, SessionState};
use crate::sound::SoundManager;
use crate::state::ClientState;
use qw_audio::{AudioConfig, AudioSystem};
use qw_common::{InfoError, InfoString, UPDATE_MASK};
use qw_renderer_gl::{
    GlRenderer, RenderEntity, RenderEntityKind, RenderView, RenderWorld, Renderer, RendererConfig,
};
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
    let mut audio = AudioSystem::new(AudioConfig::default());
    let mut sound_manager = SoundManager::new();

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
    let mut last_title: Option<String> = None;
    let mut last_world: Option<String> = None;
    let mut buf = [0u8; 8192];
    let mut was_connected = false;
    let mut pending_server_cmds: VecDeque<String> = VecDeque::new();
    let input_bindings = InputBindings::default();
    let mut input_state = InputState::default();
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
            if handle_window_event(
                &mut window,
                &mut renderer,
                &input_bindings,
                &mut input_state,
                event,
                &mut pending_server_cmds,
            ) {
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
        update_render_world(&mut renderer, &runner.state, &mut last_world);
        let (_, right, _) = angle_vectors(runner.state.sim_angles);
        audio.set_listener(
            [
                runner.state.sim_origin.x,
                runner.state.sim_origin.y,
                runner.state.sim_origin.z,
            ],
            [right.x, right.y, right.z],
        );
        sound_manager.handle_events(&mut audio, &mut runner.state, &runner.client.fs);
        update_window_title(&mut window, &runner.state.serverinfo, &mut last_title);

        if runner.session.state == SessionState::Connected {
            was_connected = true;
            while let Some(cmd) = pending_server_cmds.pop_front() {
                runner.send_string_cmd(&cmd)?;
            }
            let elapsed = last_move.elapsed();
            if elapsed >= Duration::from_millis(MOVE_INTERVAL_MS) {
                let mut cmd = input_state.build_usercmd();
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
                        pending_server_cmds.push_back(mapped);
                    }
                }
                Ok(None) => {
                    if runner.session.state == SessionState::Connected {
                        runner.send_string_cmd(&cmd)?;
                    } else {
                        pending_server_cmds.push_back(cmd);
                    }
                }
                Err(err) => {
                    println!("[client] invalid userinfo: {err}");
                }
            }
        }

        if audio.is_running() {
            renderer.set_view(build_render_view(&runner.state));
            let incoming = runner.client.netchan.incoming_sequence();
            let render_time = runner.time_seconds() - runner.state.latency;
            renderer.set_entities(build_render_entities(&runner.state, incoming, render_time));
            renderer.update_lightmaps(&runner.state.lightstyles, runner.state.server_time);
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
    input_bindings: &InputBindings,
    input_state: &mut InputState,
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
            if let Some(cmd) = input_bindings.command_for(key, action)
                && input_state.apply_command(&cmd) == CommandTarget::Server
            {
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

fn update_window_title(
    window: &mut GlfwWindow,
    serverinfo: &InfoString,
    last_title: &mut Option<String>,
) {
    if let Some(hostname) = qw_common::value_for_key(serverinfo.as_str(), "hostname")
        && last_title.as_deref() != Some(hostname.as_str())
    {
        window.set_title(format!("RustQuake - {hostname}"));
        *last_title = Some(hostname);
    }
}

fn update_render_world(
    renderer: &mut GlRenderer,
    state: &ClientState,
    last_world: &mut Option<String>,
) {
    let Some(map_name) = state.render_world_map.as_ref() else {
        return;
    };
    let Some(world) = state.render_world.as_ref() else {
        return;
    };
    if last_world.as_deref() == Some(map_name.as_str()) {
        return;
    }
    renderer.set_world(RenderWorld::from_bsp_with_palette(
        map_name.clone(),
        world.clone(),
        state.palette.as_ref(),
    ));
    *last_world = Some(map_name.clone());
}

fn build_render_view(state: &ClientState) -> RenderView {
    let (mut origin, angles) = match state.intermission {
        Some((origin, angles)) => (origin, angles),
        None => (state.sim_origin, state.sim_angles),
    };

    if state.intermission.is_none()
        && let Some(data) = &state.client_data
    {
        origin.z += f32::from(data.view_height);
    }

    RenderView {
        origin,
        angles,
        fov_y: 90.0,
    }
}

fn build_render_entities(
    state: &ClientState,
    incoming_sequence: u32,
    render_time: f64,
) -> Vec<RenderEntity> {
    let frame_index = (incoming_sequence as usize) & UPDATE_MASK;
    let prev_index = (incoming_sequence.wrapping_sub(1) as usize) & UPDATE_MASK;
    let frame = &state.frames[frame_index];
    let prev_frame = &state.frames[prev_index];
    let frac = interpolation_fraction(render_time, prev_frame.receivedtime, frame.receivedtime);
    let mut entities = Vec::new();

    let mut prev_lookup = std::collections::HashMap::new();
    let prev_count = prev_frame.packet_entities.num_entities;
    for ent in prev_frame.packet_entities.entities.iter().take(prev_count) {
        prev_lookup.insert(ent.number, *ent);
    }

    let entity_count = frame.packet_entities.num_entities;
    for ent in frame.packet_entities.entities.iter().take(entity_count) {
        let prev = prev_lookup.get(&ent.number);
        push_render_entity(&mut entities, &state.models, ent, prev, frac);
    }

    for ent in &state.static_entities {
        push_render_entity(&mut entities, &state.models, ent, None, 1.0);
    }

    entities
}

fn push_render_entity(
    entities: &mut Vec<RenderEntity>,
    models: &[String],
    ent: &qw_common::EntityState,
    prev: Option<&qw_common::EntityState>,
    frac: f32,
) {
    if ent.modelindex <= 0 {
        return;
    }
    let model_index = ent.modelindex as usize;
    let Some(name) = models.get(model_index) else {
        return;
    };
    let kind = model_kind_from_name(name);
    let origin = match prev {
        Some(previous) => lerp_vec3(previous.origin, ent.origin, frac),
        None => ent.origin,
    };
    let angles = match prev {
        Some(previous) => lerp_vec3(previous.angles, ent.angles, frac),
        None => ent.angles,
    };

    entities.push(RenderEntity {
        kind,
        model_index,
        origin,
        angles,
        frame: ent.frame.max(0) as u32,
        skin: ent.skinnum.max(0) as u32,
        alpha: 1.0,
    });
}

fn model_kind_from_name(name: &str) -> RenderEntityKind {
    if name.starts_with('*') {
        RenderEntityKind::Brush
    } else if name.to_ascii_lowercase().ends_with(".spr") {
        RenderEntityKind::Sprite
    } else {
        RenderEntityKind::Alias
    }
}

fn interpolation_fraction(now: f64, previous: f64, current: f64) -> f32 {
    if previous < 0.0 || current <= previous {
        return 1.0;
    }
    let frac = ((now - previous) / (current - previous)).clamp(0.0, 1.0);
    frac as f32
}

fn lerp_vec3(from: qw_common::Vec3, to: qw_common::Vec3, frac: f32) -> qw_common::Vec3 {
    qw_common::Vec3::new(
        from.x + (to.x - from.x) * frac,
        from.y + (to.y - from.y) * frac,
        from.z + (to.z - from.z) * frac,
    )
}

fn angle_vectors(angles: qw_common::Vec3) -> (qw_common::Vec3, qw_common::Vec3, qw_common::Vec3) {
    let (pitch, yaw, roll) = (
        angles.x.to_radians(),
        angles.y.to_radians(),
        angles.z.to_radians(),
    );
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();

    let forward = qw_common::Vec3::new(cp * cy, cp * sy, -sp);
    let right = qw_common::Vec3::new(-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp);
    let up = qw_common::Vec3::new(cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp);
    (forward, right, up)
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
        let bindings = InputBindings::default();
        let mut input_state = InputState::default();
        let mut pending = VecDeque::new();
        let exit = handle_window_event(
            &mut window,
            &mut renderer,
            &bindings,
            &mut input_state,
            WindowEvent::CloseRequested,
            &mut pending,
        );
        assert!(exit);
        assert!(window.should_close());
    }

    #[test]
    fn key_event_forwards_server_command() {
        let mut window = GlfwWindow::new(WindowConfig::default());
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let mut bindings = InputBindings::new();
        bindings.bind_command(Key::Enter, "say hello");
        let mut input_state = InputState::default();
        let mut pending = VecDeque::new();
        let exit = handle_window_event(
            &mut window,
            &mut renderer,
            &bindings,
            &mut input_state,
            WindowEvent::Key {
                key: Key::Enter,
                action: Action::Press,
            },
            &mut pending,
        );
        assert!(!exit);
        assert_eq!(pending.pop_front(), Some("say hello".to_string()));
    }

    #[test]
    fn updates_window_title_from_hostname() {
        let mut window = GlfwWindow::new(WindowConfig::default());
        let mut info = InfoString::new(128);
        info.set("hostname", "Unit").unwrap();
        let mut last_title = None;
        update_window_title(&mut window, &info, &mut last_title);
        assert_eq!(window.config().title, "RustQuake - Unit");
        assert_eq!(last_title.as_deref(), Some("Unit"));
    }

    #[test]
    fn render_view_applies_view_height() {
        let mut state = ClientState::new();
        state.sim_origin = qw_common::Vec3::new(1.0, 2.0, 3.0);
        state.sim_angles = qw_common::Vec3::new(0.0, 90.0, 0.0);
        state.client_data = Some(qw_common::ClientDataMessage {
            bits: 0,
            view_height: 22,
            ideal_pitch: 0,
            punch_angle: qw_common::Vec3::default(),
            velocity: qw_common::Vec3::default(),
            items: 0,
            onground: true,
            inwater: false,
            weapon_frame: 0,
            armor: 0,
            weapon: 0,
            health: 100,
            ammo: 0,
            ammo_counts: [0; 4],
            active_weapon: 0,
        });

        let view = build_render_view(&state);
        assert_eq!(view.origin, qw_common::Vec3::new(1.0, 2.0, 25.0));
        assert_eq!(view.angles, qw_common::Vec3::new(0.0, 90.0, 0.0));
    }

    #[test]
    fn render_view_uses_intermission() {
        let mut state = ClientState::new();
        state.intermission = Some((
            qw_common::Vec3::new(10.0, 20.0, 30.0),
            qw_common::Vec3::new(1.0, 2.0, 3.0),
        ));
        state.sim_origin = qw_common::Vec3::new(0.0, 0.0, 0.0);
        state.sim_angles = qw_common::Vec3::new(0.0, 0.0, 0.0);

        let view = build_render_view(&state);
        assert_eq!(view.origin, qw_common::Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(view.angles, qw_common::Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn updates_render_world_once() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let mut state = ClientState::new();
        state.render_world_map = Some("maps/start.bsp".to_string());
        state.render_world = Some(qw_common::BspRender {
            vertices: Vec::new(),
            edges: Vec::new(),
            surf_edges: Vec::new(),
            texinfo: Vec::new(),
            faces: Vec::new(),
            textures: Vec::new(),
            lighting: Vec::new(),
        });

        let mut last_world = None;
        update_render_world(&mut renderer, &state, &mut last_world);
        assert_eq!(last_world.as_deref(), Some("maps/start.bsp"));
        assert!(renderer.world().is_some());
    }
}
