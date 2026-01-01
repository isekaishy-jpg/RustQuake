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
use crate::model_cache::{ModelAsset, ModelKind};
use crate::net::NetClient;
use crate::runner::{ClientRunner, RunnerError};
use crate::session::{Session, SessionState};
use crate::sound::SoundManager;
use crate::state::ClientState;
use qw_audio::{AudioConfig, AudioSystem};
use qw_common::{
    InfoError, InfoString, STAT_AMMO, STAT_ARMOR, STAT_HEALTH, UPDATE_MASK, value_for_key,
};
use qw_renderer::{
    RenderBeam, RenderDynamicLight, RenderEntity, RenderEntityKind, RenderModel, RenderModelKind,
    RenderModelTexture, RenderParticle, RenderView, RenderWorld, Renderer, RendererConfig, UiLayer,
    UiText,
};
#[cfg(feature = "glow")]
use qw_renderer_gl::GlDevice;
use qw_renderer_gl::GlRenderer;
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
    #[cfg(feature = "glow")]
    {
        let device = unsafe { GlDevice::from_loader(|name| window.get_proc_address(name)) };
        renderer.set_device(device);
    }
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
    let mut last_model_count: usize = 0;
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
        let model_assets = runner.model_assets();
        if model_assets.len() != last_model_count {
            renderer.set_models(build_render_models(model_assets));
            last_model_count = model_assets.len();
        }
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
            let entity_origins = build_entity_origin_map(&runner.state, incoming, render_time);
            renderer.set_particles(build_render_particles(&runner.state));
            renderer.set_beams(build_render_beams(&runner.state));
            renderer.set_dynamic_lights(build_dynamic_lights(&runner.state, &entity_origins));
            let fog = runner
                .state
                .client_data
                .as_ref()
                .and_then(|data| data.inwater.then_some([0.08, 0.12, 0.2, 0.04]));
            renderer.set_fog(fog);
            renderer.set_ui(build_ui_layer(&runner.state, input_state.showscores()));
            renderer.update_lightmaps(&runner.state.lightstyles, runner.state.server_time);
            renderer.begin_frame();
            renderer.end_frame();
            window.swap_buffers();
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

fn build_entity_origin_map(
    state: &ClientState,
    incoming_sequence: u32,
    render_time: f64,
) -> std::collections::HashMap<u16, qw_common::Vec3> {
    let frame_index = (incoming_sequence as usize) & UPDATE_MASK;
    let prev_index = (incoming_sequence.wrapping_sub(1) as usize) & UPDATE_MASK;
    let frame = &state.frames[frame_index];
    let prev_frame = &state.frames[prev_index];
    let frac = interpolation_fraction(render_time, prev_frame.receivedtime, frame.receivedtime);
    let mut origins = std::collections::HashMap::new();

    let mut prev_lookup = std::collections::HashMap::new();
    let prev_count = prev_frame.packet_entities.num_entities;
    for ent in prev_frame.packet_entities.entities.iter().take(prev_count) {
        prev_lookup.insert(ent.number, *ent);
    }

    let entity_count = frame.packet_entities.num_entities;
    for ent in frame.packet_entities.entities.iter().take(entity_count) {
        let prev = prev_lookup.get(&ent.number);
        let origin = match prev {
            Some(previous) => lerp_vec3(previous.origin, ent.origin, frac),
            None => ent.origin,
        };
        if ent.number >= 0 && ent.number <= u16::MAX as i32 {
            origins.insert(ent.number as u16, origin);
        }
    }

    for ent in &state.static_entities {
        if ent.number >= 0 && ent.number <= u16::MAX as i32 {
            origins.entry(ent.number as u16).or_insert(ent.origin);
        }
    }

    origins
}

fn build_render_particles(state: &ClientState) -> Vec<RenderParticle> {
    const MAX_PARTICLES_PER_EVENT: usize = 128;
    let mut particles = Vec::new();
    let palette = state.palette.as_ref();

    for effect in &state.particle_effects {
        let count = (effect.count as usize).min(MAX_PARTICLES_PER_EVENT);
        let color = rgba_from_palette(palette, effect.color, 200);
        let offsets = spread_offsets(count, 6.0);
        for (idx, offset) in offsets.into_iter().enumerate() {
            let drift = effect.direction.scale(idx as f32 * 0.3);
            let position = qw_common::Vec3::new(
                effect.origin.x + drift.x + offset.x,
                effect.origin.y + drift.y + offset.y,
                effect.origin.z + drift.z + offset.z,
            );
            particles.push(RenderParticle {
                position,
                color,
                size: 4.0,
            });
        }
    }

    for temp in &state.temp_entities {
        let Some(origin) = temp.origin else {
            continue;
        };
        let (count, color, size, spread) = match temp.kind {
            qw_common::protocol::TE_GUNSHOT => (12, rgba_from_rgb([255, 220, 160], 200), 4.0, 6.0),
            qw_common::protocol::TE_BLOOD => (18, rgba_from_rgb([200, 40, 40], 220), 4.0, 6.0),
            qw_common::protocol::TE_SPIKE
            | qw_common::protocol::TE_SUPERSPIKE
            | qw_common::protocol::TE_WIZSPIKE
            | qw_common::protocol::TE_KNIGHTSPIKE => {
                (10, rgba_from_rgb([255, 255, 255], 200), 3.0, 5.0)
            }
            qw_common::protocol::TE_EXPLOSION => {
                (32, rgba_from_rgb([255, 160, 60], 220), 6.0, 10.0)
            }
            qw_common::protocol::TE_TAREXPLOSION => {
                (24, rgba_from_rgb([140, 90, 255], 220), 6.0, 9.0)
            }
            qw_common::protocol::TE_LAVASPLASH => (28, rgba_from_rgb([255, 80, 20], 220), 5.0, 9.0),
            qw_common::protocol::TE_TELEPORT => (24, rgba_from_rgb([120, 80, 255], 220), 5.0, 8.0),
            qw_common::protocol::TE_LIGHTNINGBLOOD => {
                (16, rgba_from_rgb([255, 40, 40], 220), 5.0, 7.0)
            }
            _ => continue,
        };
        let offsets = spread_offsets(count, spread);
        for offset in offsets {
            let position = qw_common::Vec3::new(
                origin.x + offset.x,
                origin.y + offset.y,
                origin.z + offset.z,
            );
            particles.push(RenderParticle {
                position,
                color,
                size,
            });
        }
    }

    for nail in &state.nails {
        particles.push(RenderParticle {
            position: nail.origin,
            color: rgba_from_rgb([255, 220, 180], 220),
            size: 3.0,
        });
    }

    particles
}

fn build_render_beams(state: &ClientState) -> Vec<RenderBeam> {
    let mut beams = Vec::new();
    for temp in &state.temp_entities {
        if !matches!(
            temp.kind,
            qw_common::protocol::TE_LIGHTNING1
                | qw_common::protocol::TE_LIGHTNING2
                | qw_common::protocol::TE_LIGHTNING3
        ) {
            continue;
        }
        let (Some(start), Some(end)) = (temp.start, temp.end) else {
            continue;
        };
        let color = match temp.kind {
            qw_common::protocol::TE_LIGHTNING1 => rgba_from_rgb([120, 160, 255], 220),
            qw_common::protocol::TE_LIGHTNING2 => rgba_from_rgb([160, 120, 255], 220),
            _ => rgba_from_rgb([120, 200, 255], 220),
        };
        beams.push(RenderBeam {
            start,
            end,
            color,
            width: 2.0,
        });
    }
    beams
}

fn build_dynamic_lights(
    state: &ClientState,
    entity_origins: &std::collections::HashMap<u16, qw_common::Vec3>,
) -> Vec<RenderDynamicLight> {
    let mut lights = Vec::new();

    for &entity in &state.muzzle_flashes {
        if let Some(origin) = entity_origins.get(&entity) {
            lights.push(RenderDynamicLight {
                origin: *origin,
                radius: 220.0,
                color: [1.0, 0.8, 0.6],
            });
        }
    }

    for damage in &state.damage_events {
        lights.push(RenderDynamicLight {
            origin: damage.origin,
            radius: 180.0,
            color: [1.0, 0.3, 0.3],
        });
    }

    for temp in &state.temp_entities {
        let Some(origin) = temp.origin else {
            continue;
        };
        let (radius, color) = match temp.kind {
            qw_common::protocol::TE_EXPLOSION => (300.0, [1.0, 0.6, 0.2]),
            qw_common::protocol::TE_TAREXPLOSION => (240.0, [0.7, 0.4, 1.0]),
            qw_common::protocol::TE_LAVASPLASH => (200.0, [1.0, 0.4, 0.1]),
            qw_common::protocol::TE_TELEPORT => (220.0, [0.6, 0.4, 1.0]),
            qw_common::protocol::TE_LIGHTNINGBLOOD => (180.0, [1.0, 0.2, 0.2]),
            _ => continue,
        };
        lights.push(RenderDynamicLight {
            origin,
            radius,
            color,
        });
    }

    lights
}

fn rgba_from_palette(palette: Option<&qw_common::Palette>, index: u8, alpha: u8) -> [u8; 4] {
    let mut rgba = palette
        .map(|palette| palette.rgba_for(index, None))
        .unwrap_or([255, 255, 255, 255]);
    rgba[3] = alpha;
    rgba
}

fn rgba_from_rgb(color: [u8; 3], alpha: u8) -> [u8; 4] {
    [color[0], color[1], color[2], alpha]
}

fn spread_offsets(count: usize, spread: f32) -> Vec<qw_common::Vec3> {
    let mut offsets = Vec::with_capacity(count);
    if count == 0 {
        return offsets;
    }
    let golden_angle = 2.399963f32;
    let total = count as f32;
    for i in 0..count {
        let t = i as f32 / total;
        let radius = spread * t.sqrt();
        let angle = i as f32 * golden_angle;
        offsets.push(qw_common::Vec3::new(
            radius * angle.cos(),
            radius * angle.sin(),
            (t - 0.5) * spread,
        ));
    }
    offsets
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

fn build_render_models(models: &[Option<ModelAsset>]) -> Vec<Option<RenderModel>> {
    models
        .iter()
        .map(|model| {
            model.as_ref().map(|asset| {
                let kind = match &asset.kind {
                    ModelKind::Alias(model) => RenderModelKind::Alias(model.clone()),
                    ModelKind::Sprite(sprite) => RenderModelKind::Sprite(sprite.clone()),
                };
                let textures = asset
                    .textures
                    .iter()
                    .map(|texture| RenderModelTexture {
                        width: texture.width,
                        height: texture.height,
                        rgba: texture.rgba.clone(),
                        fullbright: texture.fullbright.clone(),
                    })
                    .collect();
                RenderModel { kind, textures }
            })
        })
        .collect()
}

fn build_ui_layer(state: &ClientState, show_scores: bool) -> UiLayer {
    let mut texts = Vec::new();
    if state.render_world.is_none() {
        texts.push(UiText {
            text: "Loading...".to_string(),
            x: 10,
            y: 10,
            color: [255, 255, 255, 255],
        });
    }

    let health = state.stats[STAT_HEALTH];
    let armor = state.stats[STAT_ARMOR];
    let ammo = state.stats[STAT_AMMO];
    texts.push(UiText {
        text: format!("Health: {health}"),
        x: 10,
        y: 30,
        color: [255, 80, 80, 255],
    });
    texts.push(UiText {
        text: format!("Armor: {armor}"),
        x: 10,
        y: 44,
        color: [80, 160, 255, 255],
    });
    texts.push(UiText {
        text: format!("Ammo: {ammo}"),
        x: 10,
        y: 58,
        color: [255, 255, 160, 255],
    });

    if show_scores {
        let mut entries = Vec::new();
        for player in &state.players {
            let name = value_for_key(player.userinfo.as_str(), "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            entries.push((player.frags, player.ping, name));
        }
        entries.sort_by(|a, b| b.0.cmp(&a.0));

        let mut y = 80;
        for (frags, ping, name) in entries.into_iter().take(8) {
            texts.push(UiText {
                text: format!("{frags:>3} {name} ({ping})"),
                x: 10,
                y,
                color: [255, 255, 255, 255],
            });
            y += 12;
        }
    }

    UiLayer { texts }
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
            models: Vec::new(),
        });

        let mut last_world = None;
        update_render_world(&mut renderer, &state, &mut last_world);
        assert_eq!(last_world.as_deref(), Some("maps/start.bsp"));
        assert!(renderer.world().is_some());
    }

    #[test]
    fn builds_render_models_from_assets() {
        let sprite = qw_common::Sprite {
            header: qw_common::SpriteHeader {
                sprite_type: 0,
                bounding_radius: 0.0,
                width: 1,
                height: 1,
                num_frames: 0,
                beam_length: 0.0,
                sync_type: 0,
            },
            frames: Vec::new(),
        };
        let asset = ModelAsset {
            kind: ModelKind::Sprite(sprite.clone()),
            textures: vec![crate::model_cache::ModelTexture {
                width: 1,
                height: 1,
                rgba: vec![1, 2, 3, 4],
                fullbright: None,
            }],
        };

        let models = build_render_models(&[Some(asset), None]);
        assert_eq!(models.len(), 2);
        let first = models[0].as_ref().unwrap();
        match &first.kind {
            RenderModelKind::Sprite(sprite_model) => {
                assert_eq!(sprite_model.header.width, 1);
            }
            _ => panic!("expected sprite model"),
        }
        assert_eq!(first.textures[0].rgba, vec![1, 2, 3, 4]);
        assert!(models[1].is_none());
    }
}
