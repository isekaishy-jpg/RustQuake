# RustQuake

Rust port of QuakeWorld from https://github.com/id-Software/Quake.

## Layout
- vendor/quake: upstream GPL source
- crates/qw-common: shared Rust code
- crates/qw-client: client binary (WIP)
- crates/qw-server: server binary (WIP)
- docs: notes on upstream and porting

## Running the client
The client expects local Quake data files. See `docs/data-paths.md` for how to
point the client at your install.

Example:
```bash
cargo run -p qw-client -- --connect 127.0.0.1:27500 --name unit
```

Optional flags include `--data-dir`, `--download-dir`, `--qport`, `--rate`,
`--topcolor`, and `--bottomcolor`.

While running, you can type console commands into stdin (e.g. `name`, `skin`,
`say`) and the client forwards them to the server.

## Upstream
See docs/upstream.md for the exact upstream commit.

## License
This project is licensed under GPL-2.0-only to match the upstream QuakeWorld
source. See LICENSE or COPYING.
