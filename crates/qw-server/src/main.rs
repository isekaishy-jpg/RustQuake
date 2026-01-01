use qw_common::{
    A2A_ACK, A2A_ECHO, Bsp, BspError, Clc, ClientDataMessage, DataPathError, Entity, EntityDelta,
    EntityError, EntityState, FsError, MoveVars, MsgReadError, MsgReader, Netchan, NetchanError,
    OobMessage, PORT_SERVER, PROTOCOL_VERSION, PacketEntitiesUpdate, PlayerInfoMessage, QuakeFs,
    S2C_CHALLENGE, S2C_CONNECTION, ServerData, SizeBuf, StringListChunk, SvcMessage, UserCmd, Vec3,
    build_out_of_band, find_game_dir, find_id1_dir, locate_data_dir, out_of_band_payload,
    parse_entities, parse_oob_message, write_svc_message,
};
use qw_qc::{ProgsDat, ProgsError, Vm, VmError};
use std::collections::HashMap;
use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

mod qc;

const MAX_QC_STEPS: usize = 200_000;

#[derive(Clone)]
struct ServerInfo {
    server_count: i32,
    game_dir: String,
    level_name: String,
    movevars: MoveVars,
    sound_list: Vec<String>,
    model_list: Vec<String>,
    lightstyles: Vec<Option<String>>,
}

#[derive(Clone)]
struct ServerWorld {
    static_entities: Vec<EntityState>,
    static_sounds: Vec<StaticSoundInfo>,
    player_baseline: EntityState,
    player_info: PlayerInfoMessage,
}

#[derive(Clone)]
struct StaticSoundInfo {
    origin: Vec3,
    sound: u8,
    volume: u8,
    attenuation: u8,
}

struct ClientState {
    netchan: Netchan,
    signon: u8,
    last_heard: Instant,
    userinfo: String,
    last_frame: Instant,
}

impl ClientState {
    fn new(qport: u16, userinfo: String) -> Self {
        Self {
            netchan: Netchan::new(qport),
            signon: 0,
            last_heard: Instant::now(),
            userinfo,
            last_frame: Instant::now(),
        }
    }
}

struct ServerContext {
    info: ServerInfo,
    world: ServerWorld,
    start: Instant,
}

#[derive(Clone, Copy, Default)]
struct SpawnPoint {
    origin: Vec3,
    angles: Vec3,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("[server] {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), ServerError> {
    let data_dir = locate_data_dir().map_err(ServerError::DataPath)?;
    let game_name = env::var("RUSTQUAKE_GAME").unwrap_or_else(|_| "id1".to_string());
    let game_dir = find_game_dir(&data_dir, &game_name)
        .or_else(|| find_id1_dir(&data_dir))
        .ok_or(ServerError::GameDirMissing)?;

    let mut fs = QuakeFs::new();
    fs.add_game_dir(&game_dir).map_err(ServerError::Fs)?;

    let progs_name = if fs.contains("progs.dat") {
        "progs.dat"
    } else if fs.contains("qwprogs.dat") {
        "qwprogs.dat"
    } else {
        return Err(ServerError::ProgsMissing);
    };

    let bytes = fs.read(progs_name).map_err(ServerError::Fs)?;
    let progs = ProgsDat::from_bytes(&bytes).map_err(ServerError::Progs)?;
    let map_name = env::var("RUSTQUAKE_MAP").unwrap_or_else(|_| "start".to_string());
    let mut vm = Vm::with_context(progs, qc::ServerQcContext::default());
    qc::configure_vm(&mut vm, &map_name).map_err(ServerError::Vm)?;

    let func_count = vm.progs().functions.len();
    let global_count = vm.progs().globals.len();
    println!("[server] loaded {progs_name} with {func_count} functions and {global_count} globals");
    if let Err(err) = vm.call_by_name("main", MAX_QC_STEPS) {
        println!(
            "[server] qc main not executed: {}",
            describe_vm_error(&vm, &err)
        );
    }

    let map_entities = load_map_entities(&fs, &map_name).ok();
    if let Some(entities) = map_entities.as_ref() {
        if let Err(err) = qc::apply_worldspawn(&mut vm, entities) {
            println!("[server] qc worldspawn not applied: {err:?}");
        }
        if let Err(err) = vm.call_by_name("worldspawn", MAX_QC_STEPS) {
            println!(
                "[server] qc worldspawn not executed: {}",
                describe_vm_error(&vm, &err)
            );
        }
        if let Err(err) = qc::spawn_entities(&mut vm, entities, MAX_QC_STEPS) {
            println!(
                "[server] qc entity spawn failed: {}",
                describe_vm_error(&vm, &err)
            );
        }
        if let Err(err) = vm.call_by_name("StartFrame", MAX_QC_STEPS) {
            println!(
                "[server] qc start frame failed: {}",
                describe_vm_error(&vm, &err)
            );
        }
    }

    let qc_snapshot = qc::snapshot(&vm);
    let server_info = build_server_info(&game_name, &map_name, qc_snapshot.clone());
    let spawn_point = map_entities
        .as_ref()
        .map(|entities| find_spawn_point(entities))
        .unwrap_or_default();
    let server_world = build_world_snapshot(&vm, &server_info, &qc_snapshot, spawn_point);
    run_network(server_info, server_world)?;

    Ok(())
}

#[derive(Debug)]
enum ServerError {
    DataPath(DataPathError),
    Fs(FsError),
    Progs(ProgsError),
    Vm(VmError),
    Bsp(BspError),
    Entities(EntityError),
    Net(std::io::Error),
    GameDirMissing,
    ProgsMissing,
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::DataPath(err) => write!(f, "data path error: {:?}", err),
            ServerError::Fs(err) => write!(f, "fs error: {:?}", err),
            ServerError::Progs(err) => write!(f, "progs error: {:?}", err),
            ServerError::Vm(err) => write!(f, "vm error: {:?}", err),
            ServerError::Bsp(err) => write!(f, "bsp error: {}", err),
            ServerError::Entities(err) => write!(f, "entity parse error: {:?}", err),
            ServerError::Net(err) => write!(f, "network error: {err}"),
            ServerError::GameDirMissing => write!(f, "game directory not found"),
            ServerError::ProgsMissing => write!(f, "progs.dat or qwprogs.dat not found"),
        }
    }
}

fn load_map_entities(fs: &QuakeFs, map_name: &str) -> Result<Vec<Entity>, ServerError> {
    let map_path = format!("maps/{map_name}.bsp");
    let bytes = fs.read(&map_path).map_err(ServerError::Fs)?;
    let bsp = Bsp::from_bytes(bytes).map_err(ServerError::Bsp)?;
    let text = bsp.entities_text().map_err(ServerError::Bsp)?;
    parse_entities(&text).map_err(ServerError::Entities)
}

fn build_server_info(
    game_name: &str,
    map_name: &str,
    snapshot: qc::ServerQcSnapshot,
) -> ServerInfo {
    let mut sound_list = Vec::new();
    sound_list.push(String::new());
    sound_list.extend(snapshot.precache_sounds);
    let sound_list = dedupe_case(sound_list);

    let mut model_list = Vec::new();
    model_list.push(String::new());
    model_list.push(format!("maps/{map_name}.bsp"));
    model_list.extend(snapshot.precache_models);
    let model_list = dedupe_case(model_list);

    ServerInfo {
        server_count: 1,
        game_dir: game_name.to_string(),
        level_name: map_name.to_string(),
        movevars: default_movevars(),
        sound_list,
        model_list,
        lightstyles: snapshot.lightstyles,
    }
}

fn default_movevars() -> MoveVars {
    MoveVars {
        gravity: 800.0,
        stopspeed: 100.0,
        maxspeed: 320.0,
        spectatormaxspeed: 500.0,
        accelerate: 10.0,
        airaccelerate: 0.0,
        wateraccelerate: 10.0,
        friction: 6.0,
        waterfriction: 1.0,
        entgravity: 1.0,
    }
}

fn dedupe_case(list: Vec<String>) -> Vec<String> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for item in list {
        let key = item.to_ascii_lowercase();
        if seen.insert(key, ()).is_none() {
            out.push(item);
        }
    }
    out
}

fn build_world_snapshot(
    vm: &Vm,
    server_info: &ServerInfo,
    snapshot: &qc::ServerQcSnapshot,
    spawn: SpawnPoint,
) -> ServerWorld {
    let mut static_entities = Vec::new();
    for ent in &snapshot.static_entities {
        if let Some(state) = qc::entity_state(vm, *ent, &server_info.model_list) {
            static_entities.push(state);
        }
    }

    let sound_index = build_index_map(&server_info.sound_list);
    let mut static_sounds = Vec::new();
    for sound in &snapshot.ambient_sounds {
        let key = sound.sample.to_ascii_lowercase();
        let index = sound_index.get(&key).or_else(|| {
            key.strip_prefix("sound/")
                .and_then(|name| sound_index.get(name))
        });
        let Some(index) = index else {
            continue;
        };
        static_sounds.push(StaticSoundInfo {
            origin: sound.origin,
            sound: *index,
            volume: clamp_u8(sound.volume * 255.0),
            attenuation: clamp_u8(sound.attenuation * 64.0),
        });
    }

    ServerWorld {
        static_entities,
        static_sounds,
        player_baseline: build_player_baseline(spawn, server_info),
        player_info: build_player_info(spawn),
    }
}

fn build_index_map(list: &[String]) -> HashMap<String, u8> {
    let mut map = HashMap::new();
    for (index, item) in list.iter().enumerate() {
        if index > u8::MAX as usize {
            break;
        }
        map.insert(item.to_ascii_lowercase(), index as u8);
    }
    map
}

fn clamp_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn build_player_baseline(spawn: SpawnPoint, server_info: &ServerInfo) -> EntityState {
    let model_index = model_index_for("progs/player.mdl", &server_info.model_list);
    EntityState {
        number: 1,
        flags: 0,
        origin: spawn.origin,
        angles: spawn.angles,
        modelindex: model_index as i32,
        frame: 0,
        colormap: 0,
        skinnum: 0,
        effects: 0,
    }
}

fn build_player_info(spawn: SpawnPoint) -> PlayerInfoMessage {
    PlayerInfoMessage {
        num: 0,
        flags: 0,
        origin: spawn.origin,
        frame: 0,
        msec: None,
        command: None,
        velocity: [0; 3],
        model_index: None,
        skin_num: None,
        effects: None,
        weapon_frame: None,
    }
}

fn model_index_for(name: &str, model_list: &[String]) -> u8 {
    for (index, entry) in model_list.iter().enumerate() {
        if index > u8::MAX as usize {
            break;
        }
        if entry.eq_ignore_ascii_case(name) {
            return index as u8;
        }
    }
    0
}

fn run_network(server_info: ServerInfo, server_world: ServerWorld) -> Result<(), ServerError> {
    let bind_addr = format!("0.0.0.0:{PORT_SERVER}");
    let socket = UdpSocket::bind(&bind_addr).map_err(ServerError::Net)?;
    socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(ServerError::Net)?;
    println!("[server] listening on {bind_addr}");

    let context = ServerContext {
        info: server_info,
        world: server_world,
        start: Instant::now(),
    };
    let mut rng_state = 0x1234_5678u32;
    let mut challenges: HashMap<SocketAddr, i32> = HashMap::new();
    let mut clients: HashMap<SocketAddr, ClientState> = HashMap::new();
    let run_once = env::var("RUSTQUAKE_RUN_ONCE").is_ok();
    let mut buf = [0u8; 1400];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let packet = &buf[..len];
                handle_packet(
                    &socket,
                    addr,
                    packet,
                    &context,
                    &mut clients,
                    &mut challenges,
                    &mut rng_state,
                )
                .map_err(ServerError::Net)?;
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(err) => return Err(ServerError::Net(err)),
        }

        if run_once && context.start.elapsed() > Duration::from_millis(200) {
            break;
        }
    }

    Ok(())
}

fn handle_packet(
    socket: &UdpSocket,
    addr: SocketAddr,
    packet: &[u8],
    context: &ServerContext,
    clients: &mut HashMap<SocketAddr, ClientState>,
    challenges: &mut HashMap<SocketAddr, i32>,
    rng_state: &mut u32,
) -> Result<(), std::io::Error> {
    if let Some(payload) = out_of_band_payload(packet) {
        return handle_oob(socket, addr, payload, clients, challenges, rng_state);
    }

    handle_inband(socket, addr, packet, context, clients)
}

fn handle_oob(
    socket: &UdpSocket,
    addr: SocketAddr,
    payload: &[u8],
    clients: &mut HashMap<SocketAddr, ClientState>,
    challenges: &mut HashMap<SocketAddr, i32>,
    rng_state: &mut u32,
) -> Result<(), std::io::Error> {
    let text = String::from_utf8_lossy(payload);
    let trimmed = text.trim_matches(|ch| ch == '\0' || ch == '\n' || ch == '\r');

    if trimmed.starts_with("getchallenge") {
        let challenge = next_challenge(rng_state);
        challenges.insert(addr, challenge);
        let mut reply = Vec::new();
        reply.push(S2C_CHALLENGE);
        reply.extend_from_slice(challenge.to_string().as_bytes());
        reply.push(0);
        let packet = build_out_of_band(&reply);
        socket.send_to(&packet, addr)?;
        return Ok(());
    }

    if trimmed.starts_with("connect") {
        if let Some(connect) = parse_connect(trimmed) {
            if connect.protocol == PROTOCOL_VERSION {
                let matches = challenges.get(&addr).copied() == Some(connect.challenge);
                if matches {
                    clients.insert(addr, ClientState::new(connect.qport, connect.userinfo));
                    let packet = build_out_of_band(&[S2C_CONNECTION, 0]);
                    socket.send_to(&packet, addr)?;
                }
            }
        }
        return Ok(());
    }

    if let Some(msg) = parse_oob_message(payload) {
        match msg {
            OobMessage::Ping => {
                let packet = build_out_of_band(&[A2A_ACK, b'\n']);
                socket.send_to(&packet, addr)?;
            }
            OobMessage::Echo(value) => {
                let mut reply = Vec::new();
                reply.push(A2A_ECHO);
                reply.extend_from_slice(value.as_bytes());
                reply.push(0);
                let packet = build_out_of_band(&reply);
                socket.send_to(&packet, addr)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn handle_inband(
    socket: &UdpSocket,
    addr: SocketAddr,
    packet: &[u8],
    context: &ServerContext,
    clients: &mut HashMap<SocketAddr, ClientState>,
) -> Result<(), std::io::Error> {
    let Some(client) = clients.get_mut(&addr) else {
        return Ok(());
    };
    client.last_heard = Instant::now();
    let payload = client
        .netchan
        .process_packet(packet, true)
        .map_err(netchan_to_io)?;
    let mut reader = MsgReader::new(payload);
    let mut pending = None;
    let mut saw_move = false;

    while reader.remaining() > 0 {
        let cmd = match pending.take() {
            Some(value) => value,
            None => reader.read_u8().map_err(msg_to_io)?,
        };
        let Ok(clc) = Clc::try_from(cmd) else {
            break;
        };

        match clc {
            Clc::Nop => {}
            Clc::StringCmd => {
                let text = reader.read_string().map_err(msg_to_io)?;
                handle_string_cmd(socket, addr, client, &context.info, &context.world, &text)?;
            }
            Clc::Move => {
                pending = skip_move(&mut reader).map_err(msg_to_io)?;
                saw_move = true;
            }
            Clc::Delta => {
                let _ = reader.read_u8().map_err(msg_to_io)?;
            }
            Clc::TMove => {
                if reader.remaining() >= 6 {
                    let _ = reader.read_i16().map_err(msg_to_io)?;
                    let _ = reader.read_i16().map_err(msg_to_io)?;
                    let _ = reader.read_i16().map_err(msg_to_io)?;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    if saw_move {
        maybe_send_frame(socket, addr, client, context)?;
    }

    Ok(())
}

fn handle_string_cmd(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_info: &ServerInfo,
    server_world: &ServerWorld,
    text: &str,
) -> Result<(), std::io::Error> {
    let mut parts = text.split_whitespace();
    let Some(cmd) = parts.next() else {
        return Ok(());
    };

    match cmd {
        "new" => {
            send_serverdata(socket, addr, client, server_info)?;
        }
        "soundlist" => {
            let _ = parts.next();
            let start = parts
                .next()
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(0);
            send_soundlist(socket, addr, client, server_info, start)?;
        }
        "modellist" => {
            let _ = parts.next();
            let start = parts
                .next()
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(0);
            send_modellist(socket, addr, client, server_info, start)?;
        }
        "prespawn" => {
            send_prespawn(socket, addr, client, server_world)?;
        }
        "spawn" => {
            send_spawn(socket, addr, client, server_info)?;
        }
        "begin" => {
            send_begin(client);
        }
        _ => {}
    }

    Ok(())
}

fn send_serverdata(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_info: &ServerInfo,
) -> Result<(), std::io::Error> {
    let data = ServerData {
        protocol: PROTOCOL_VERSION,
        server_count: server_info.server_count,
        game_dir: server_info.game_dir.clone(),
        player_num: 0,
        spectator: false,
        level_name: server_info.level_name.clone(),
        movevars: server_info.movevars,
    };
    let mut messages = Vec::new();
    messages.push(SvcMessage::ServerData(data));
    messages.push(SvcMessage::SignonNum(1));
    for (index, style) in server_info.lightstyles.iter().enumerate() {
        if let Some(value) = style {
            messages.push(SvcMessage::LightStyle {
                style: index as u8,
                value: value.clone(),
            });
        }
    }
    client.signon = 1;
    send_svc_messages(socket, addr, client, &messages)
}

fn send_soundlist(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_info: &ServerInfo,
    start: u8,
) -> Result<(), std::io::Error> {
    let chunk = build_list_chunk(&server_info.sound_list, start);
    send_svc_messages(socket, addr, client, &[SvcMessage::SoundList(chunk)])
}

fn send_modellist(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_info: &ServerInfo,
    start: u8,
) -> Result<(), std::io::Error> {
    let chunk = build_list_chunk(&server_info.model_list, start);
    send_svc_messages(socket, addr, client, &[SvcMessage::ModelList(chunk)])
}

fn send_prespawn(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_world: &ServerWorld,
) -> Result<(), std::io::Error> {
    let mut messages = Vec::new();
    messages.push(SvcMessage::SignonNum(2));
    for entity in &server_world.static_entities {
        messages.push(SvcMessage::SpawnStatic(*entity));
    }
    for sound in &server_world.static_sounds {
        messages.push(SvcMessage::SpawnStaticSound {
            origin: sound.origin,
            sound: sound.sound,
            volume: sound.volume,
            attenuation: sound.attenuation,
        });
    }
    messages.push(SvcMessage::SpawnBaseline {
        entity: server_world.player_baseline.number as u16,
        baseline: server_world.player_baseline,
    });
    messages.push(SvcMessage::StuffText("cmd spawn 0 0\n".to_string()));
    client.signon = 2;
    send_svc_messages(socket, addr, client, &messages)
}

fn send_spawn(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_info: &ServerInfo,
) -> Result<(), std::io::Error> {
    let mut messages = Vec::new();
    messages.push(SvcMessage::SignonNum(3));
    messages.extend(server_info_messages(server_info));
    messages.push(SvcMessage::UpdateUserInfo {
        slot: 0,
        user_id: 1,
        userinfo: client.userinfo.clone(),
    });
    messages.push(SvcMessage::SetView { entity: 1 });
    messages.push(SvcMessage::ClientData(default_client_data()));
    messages.push(SvcMessage::StuffText("cmd begin\n".to_string()));
    client.signon = 3;
    send_svc_messages(socket, addr, client, &messages)
}

fn send_begin(client: &mut ClientState) {
    client.signon = 3;
}

fn send_svc_messages(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    messages: &[SvcMessage],
) -> Result<(), std::io::Error> {
    let mut buf = SizeBuf::new(2048);
    for message in messages {
        write_svc_message(&mut buf, message).map_err(sizebuf_to_io)?;
    }
    client
        .netchan
        .queue_reliable(buf.as_slice())
        .map_err(netchan_to_io)?;
    let packet = client
        .netchan
        .build_packet(&[], false)
        .map_err(netchan_to_io)?;
    socket.send_to(&packet, addr)?;
    Ok(())
}

fn send_unreliable_messages(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    messages: &[SvcMessage],
) -> Result<(), std::io::Error> {
    let mut buf = SizeBuf::new(2048);
    for message in messages {
        write_svc_message(&mut buf, message).map_err(sizebuf_to_io)?;
    }
    let packet = client
        .netchan
        .build_packet(buf.as_slice(), false)
        .map_err(netchan_to_io)?;
    socket.send_to(&packet, addr)?;
    Ok(())
}

fn build_list_chunk(list: &[String], start: u8) -> StringListChunk {
    let start_index = start as usize;
    if start_index >= list.len() {
        return StringListChunk {
            start,
            items: Vec::new(),
            next: 0,
        };
    }

    let max_items = 64usize;
    let items: Vec<String> = list
        .iter()
        .skip(start_index)
        .take(max_items)
        .cloned()
        .collect();
    let next_index = start_index + items.len();
    let next = if next_index < list.len() && next_index <= u8::MAX as usize {
        next_index as u8
    } else {
        0
    };

    StringListChunk { start, items, next }
}

fn find_spawn_point(entities: &[Entity]) -> SpawnPoint {
    let candidates = ["info_player_start", "info_player_deathmatch"];
    for name in candidates {
        if let Some(entity) = entities.iter().find(|entity| {
            entity
                .get("classname")
                .map(|value| value.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        }) {
            return spawn_from_entity(entity);
        }
    }
    SpawnPoint::default()
}

fn spawn_from_entity(entity: &Entity) -> SpawnPoint {
    let origin = entity
        .get("origin")
        .and_then(parse_vec3)
        .unwrap_or_default();
    let angles = entity
        .get("angles")
        .and_then(parse_vec3)
        .or_else(|| {
            entity
                .get("angle")
                .and_then(|value| value.trim().parse::<f32>().ok())
                .map(|yaw| Vec3::new(0.0, yaw, 0.0))
        })
        .unwrap_or_default();
    SpawnPoint { origin, angles }
}

fn parse_vec3(value: &str) -> Option<Vec3> {
    let mut iter = value
        .split(|ch: char| ch == ' ' || ch == '\t')
        .filter(|part| !part.is_empty());
    let x = iter.next()?.parse::<f32>().ok()?;
    let y = iter.next()?.parse::<f32>().ok()?;
    let z = iter.next()?.parse::<f32>().ok()?;
    Some(Vec3::new(x, y, z))
}

fn server_info_messages(server_info: &ServerInfo) -> Vec<SvcMessage> {
    vec![
        SvcMessage::ServerInfo {
            key: "hostname".to_string(),
            value: "RustQuake".to_string(),
        },
        SvcMessage::ServerInfo {
            key: "map".to_string(),
            value: server_info.level_name.clone(),
        },
        SvcMessage::ServerInfo {
            key: "maxclients".to_string(),
            value: "1".to_string(),
        },
    ]
}

fn default_client_data() -> ClientDataMessage {
    ClientDataMessage {
        bits: 0,
        view_height: 22,
        ideal_pitch: 0,
        punch_angle: Vec3::default(),
        velocity: Vec3::default(),
        items: 0,
        onground: false,
        inwater: false,
        weapon_frame: 0,
        armor: 0,
        weapon: 0,
        health: 100,
        ammo: 0,
        ammo_counts: [0; 4],
        active_weapon: 0,
    }
}

fn maybe_send_frame(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    context: &ServerContext,
) -> Result<(), std::io::Error> {
    if client.signon < 3 {
        return Ok(());
    }
    if client.last_frame.elapsed() < Duration::from_millis(50) {
        return Ok(());
    }

    let server_time = context.start.elapsed().as_secs_f32();
    let delta = entity_delta_from(&context.world.player_baseline);
    let update = PacketEntitiesUpdate {
        delta_from: None,
        entities: vec![delta],
    };
    let messages = [
        SvcMessage::Time(server_time),
        SvcMessage::PlayerInfo(context.world.player_info.clone()),
        SvcMessage::PacketEntities(update),
    ];
    send_unreliable_messages(socket, addr, client, &messages)?;
    client.last_frame = Instant::now();
    Ok(())
}

fn entity_delta_from(state: &EntityState) -> EntityDelta {
    let number = state.number.max(0).min(u16::MAX as i32) as u16;
    EntityDelta {
        number,
        remove: false,
        flags: 0,
        model_index: clamp_entity_u8(state.modelindex),
        frame: clamp_entity_u8(state.frame),
        colormap: clamp_entity_u8(state.colormap),
        skin_num: clamp_entity_u8(state.skinnum),
        effects: clamp_entity_u8(state.effects),
        origin: [
            Some(state.origin.x),
            Some(state.origin.y),
            Some(state.origin.z),
        ],
        angles: [
            Some(state.angles.x),
            Some(state.angles.y),
            Some(state.angles.z),
        ],
        solid: false,
    }
}

fn clamp_entity_u8(value: i32) -> Option<u8> {
    if value < 0 {
        return Some(0);
    }
    if value > u8::MAX as i32 {
        return Some(u8::MAX);
    }
    Some(value as u8)
}

fn skip_move(reader: &mut MsgReader) -> Result<Option<u8>, MsgReadError> {
    let _checksum = reader.read_u8()?;
    let _lost = reader.read_u8()?;
    let base = UserCmd::default();
    let cmd0 = reader.read_delta_usercmd(&base)?;
    let cmd1 = reader.read_delta_usercmd(&cmd0)?;
    let _cmd2 = reader.read_delta_usercmd(&cmd1)?;

    if reader.remaining() > 0 {
        let next = reader.read_u8()?;
        if next == Clc::Delta as u8 {
            let _ = reader.read_u8()?;
            Ok(None)
        } else {
            Ok(Some(next))
        }
    } else {
        Ok(None)
    }
}

fn netchan_to_io(err: NetchanError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, format!("netchan: {err:?}"))
}

fn msg_to_io(err: MsgReadError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, format!("message: {err:?}"))
}

fn sizebuf_to_io(err: qw_common::SizeBufError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, format!("sizebuf: {err:?}"))
}

fn next_challenge(state: &mut u32) -> i32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state & 0x7FFF_FFFF) as i32
}

#[derive(Debug, Clone)]
struct ConnectInfo {
    protocol: i32,
    qport: u16,
    challenge: i32,
    userinfo: String,
}

fn parse_connect(text: &str) -> Option<ConnectInfo> {
    let quote_start = text.find('"')?;
    let quote_end = text.rfind('"')?;
    if quote_end <= quote_start {
        return None;
    }
    let userinfo = text[quote_start + 1..quote_end].to_string();
    let head = &text[..quote_start];
    let mut parts = head.split_whitespace();
    let cmd = parts.next()?;
    if cmd != "connect" {
        return None;
    }
    let protocol = parts.next()?.parse::<i32>().ok()?;
    let qport = parts.next()?.parse::<u16>().ok()?;
    let challenge = parts.next()?.parse::<i32>().ok()?;
    Some(ConnectInfo {
        protocol,
        qport,
        challenge,
        userinfo,
    })
}

fn describe_vm_error(vm: &Vm, err: &VmError) -> String {
    match err {
        VmError::StepLimit {
            statement,
            function,
        } => {
            let name = vm
                .progs()
                .functions
                .get(*function as usize)
                .map(|func| func.name.as_str())
                .unwrap_or("unknown");
            let stmt = vm.progs().statements.get(*statement as usize).copied();
            let op = stmt.map(|value| value.op).unwrap_or(0);
            let (a, b, c) = stmt
                .map(|value| (value.a, value.b, value.c))
                .unwrap_or((0, 0, 0));
            format!(
                "step limit at {name} (fn {function}, statement {statement}, op {op}, a {a}, b {b}, c {c})"
            )
        }
        other => format!("{other:?}"),
    }
}
