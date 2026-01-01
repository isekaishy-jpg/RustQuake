use qw_common::{
    A2A_ACK, A2A_ECHO, Bsp, BspError, DataPathError, Entity, EntityError, FsError, OobMessage,
    PORT_SERVER, PROTOCOL_VERSION, QuakeFs, S2C_CHALLENGE, S2C_CONNECTION, build_out_of_band,
    find_game_dir, find_id1_dir, locate_data_dir, out_of_band_payload, parse_entities,
    parse_oob_message,
};
use qw_qc::{ProgsDat, ProgsError, Vm, VmError};
use std::collections::HashMap;
use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

mod qc;

const MAX_QC_STEPS: usize = 200_000;

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

    run_network()?;

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

fn run_network() -> Result<(), ServerError> {
    let bind_addr = format!("0.0.0.0:{PORT_SERVER}");
    let socket = UdpSocket::bind(&bind_addr).map_err(ServerError::Net)?;
    socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(ServerError::Net)?;
    println!("[server] listening on {bind_addr}");

    let mut rng_state = 0x1234_5678u32;
    let mut challenges: HashMap<SocketAddr, i32> = HashMap::new();
    let run_once = env::var("RUSTQUAKE_RUN_ONCE").is_ok();
    let start = Instant::now();
    let mut buf = [0u8; 1400];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, addr)) => {
                let packet = &buf[..len];
                handle_packet(&socket, addr, packet, &mut challenges, &mut rng_state)
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
    challenges: &mut HashMap<SocketAddr, i32>,
    rng_state: &mut u32,
) -> Result<(), std::io::Error> {
    let Some(payload) = out_of_band_payload(packet) else {
        return Ok(());
    };
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
