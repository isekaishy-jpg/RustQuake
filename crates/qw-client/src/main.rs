mod client;
mod config;
mod handshake;
mod net;
mod prediction;
mod runner;
mod session;
mod state;

fn main() {
    println!("RustQuake client stub");
    match qw_common::locate_data_dir() {
        Ok(dir) => {
            println!("Data dir: {}", dir.display());
            if let Some(id1) = qw_common::find_id1_dir(&dir) {
                println!("id1 dir: {}", id1.display());
                let mut fs = qw_common::QuakeFs::new();
                if let Err(err) = fs.add_game_dir(&id1) {
                    println!("Failed to add game dir: {:?}", err);
                    return;
                }
                if fs.contains("gfx.wad") {
                    println!("Found gfx.wad");
                    match fs.read("gfx.wad") {
                        Ok(bytes) => match qw_common::Wad::from_bytes(bytes) {
                            Ok(wad) => println!("gfx.wad lumps: {}", wad.entries().len()),
                            Err(err) => println!("Failed to parse gfx.wad: {:?}", err),
                        },
                        Err(err) => println!("Failed to read gfx.wad: {:?}", err),
                    }
                } else {
                    println!("gfx.wad not found");
                }

                if fs.contains("maps/start.bsp") {
                    match fs.read("maps/start.bsp") {
                        Ok(bytes) => match qw_common::Bsp::from_bytes(bytes) {
                            Ok(bsp) => println!("start.bsp version: {}", bsp.version),
                            Err(err) => println!("Failed to parse start.bsp: {:?}", err),
                        },
                        Err(err) => println!("Failed to read start.bsp: {:?}", err),
                    }
                }
            } else {
                println!("id1 directory not found under data dir.");
            }
        }
        Err(_) => {
            println!("Data dir not configured. See docs/data-paths.md.");
        }
    }
}
