use qw_common::{
    A2A_ACK, A2A_ECHO, Bsp, BspError, Clc, DataPathError, Entity, EntityError, FsError, MoveVars,
    MsgReadError, MsgReader, Netchan, NetchanError, OobMessage, PORT_SERVER, PROTOCOL_VERSION,
    QuakeFs, S2C_CHALLENGE, S2C_CONNECTION, ServerData, SizeBuf, StringListChunk, SvcMessage,
    UserCmd, build_out_of_band, find_game_dir, find_id1_dir, locate_data_dir, out_of_band_payload,
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

struct ClientState {
    netchan: Netchan,
    signon: u8,
    last_heard: Instant,
}

impl ClientState {
    fn new(qport: u16) -> Self {
        Self {
            netchan: Netchan::new(qport),
            signon: 0,
            last_heard: Instant::now(),
        }
    }
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

    if let Ok(entities) = load_map_entities(&fs, &map_name) {
        if let Err(err) = qc::apply_worldspawn(&mut vm, &entities) {
            println!("[server] qc worldspawn not applied: {err:?}");
        }
        if let Err(err) = vm.call_by_name("worldspawn", MAX_QC_STEPS) {
            println!(
                "[server] qc worldspawn not executed: {}",
                describe_vm_error(&vm, &err)
            );
        }
        if let Err(err) = qc::spawn_entities(&mut vm, &entities, MAX_QC_STEPS) {
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
    let server_info = build_server_info(&game_name, &map_name, qc_snapshot);
    run_network(server_info)?;

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

fn run_network(server_info: ServerInfo) -> Result<(), ServerError> {
    let bind_addr = format!("0.0.0.0:{PORT_SERVER}");
    let socket = UdpSocket::bind(&bind_addr).map_err(ServerError::Net)?;
    socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(ServerError::Net)?;
    println!("[server] listening on {bind_addr}");

    let mut rng_state = 0x1234_5678u32;
    let mut challenges: HashMap<SocketAddr, i32> = HashMap::new();
    let mut clients: HashMap<SocketAddr, ClientState> = HashMap::new();
    let run_once = env::var("RUSTQUAKE_RUN_ONCE").is_ok();
    let start = Instant::now();
    let mut buf = [0u8; 1400];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let packet = &buf[..len];
                handle_packet(
                    &socket,
                    addr,
                    packet,
                    &server_info,
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

        if run_once && start.elapsed() > Duration::from_millis(200) {
            break;
        }
    }

    Ok(())
}

fn handle_packet(
    socket: &UdpSocket,
    addr: SocketAddr,
    packet: &[u8],
    server_info: &ServerInfo,
    clients: &mut HashMap<SocketAddr, ClientState>,
    challenges: &mut HashMap<SocketAddr, i32>,
    rng_state: &mut u32,
) -> Result<(), std::io::Error> {
    if let Some(payload) = out_of_band_payload(packet) {
        return handle_oob(socket, addr, payload, clients, challenges, rng_state);
    }

    handle_inband(socket, addr, packet, server_info, clients)
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
                    clients.insert(addr, ClientState::new(connect._qport));
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
    server_info: &ServerInfo,
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
                handle_string_cmd(socket, addr, client, server_info, &text)?;
            }
            Clc::Move => {
                pending = skip_move(&mut reader).map_err(msg_to_io)?;
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

    Ok(())
}

fn handle_string_cmd(
    socket: &UdpSocket,
    addr: SocketAddr,
    client: &mut ClientState,
    server_info: &ServerInfo,
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
            send_prespawn(socket, addr, client)?;
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
) -> Result<(), std::io::Error> {
    client.signon = 3;
    send_svc_messages(
        socket,
        addr,
        client,
        &[SvcMessage::SignonNum(2), SvcMessage::SignonNum(3)],
    )
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
    _qport: u16,
    challenge: i32,
    _userinfo: String,
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
        _qport: qport,
        challenge,
        _userinfo: userinfo,
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
