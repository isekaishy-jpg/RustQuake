use qw_common::{
    Bsp, BspError, DataPathError, Entity, EntityError, FsError, QuakeFs, find_game_dir,
    find_id1_dir, locate_data_dir, parse_entities,
};
use qw_qc::{ProgsDat, ProgsError, Vm, VmError};
use std::env;

mod qc;

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
    if let Err(err) = vm.call_by_name("main", 10_000) {
        println!("[server] qc main not executed: {err:?}");
    }

    if let Ok(entities) = load_map_entities(&fs, &map_name) {
        if let Err(err) = qc::apply_worldspawn(&mut vm, &entities) {
            println!("[server] qc worldspawn not applied: {err:?}");
        }
        if let Err(err) = vm.call_by_name("worldspawn", 10_000) {
            println!("[server] qc worldspawn not executed: {err:?}");
        }
    }

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
