# RustQuake

Rust port of QuakeWorld from https://github.com/id-Software/Quake.

## Layout
- vendor/quake: upstream GPL source
- crates/qw-common: shared Rust code
- crates/qw-client: client binary (WIP)
- crates/qw-renderer-gl: OpenGL renderer (WIP)
- crates/qw-window-glfw: GLFW windowing (WIP)
- crates/qw-audio: audio backend (WIP)
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

Mode selection: `--mode qw` (default) for QuakeWorld, or `--mode sp` for
singleplayer (singleplayer runtime is pending).

## Renderer features
The OpenGL path is feature-gated and disabled by default.

- `qw-client` feature `glow`: enables the GLFW window backend and `glow`-based
  OpenGL renderer.
- Without `glow`, the window and renderer use stub backends that keep the build
  and tests headless.

Example:
```bash
cargo run -p qw-client --features glow -- --connect 127.0.0.1:27500
```

Runtime toggles are currently programmatic:
- `GlRenderer::set_wireframe(true)` switches the world to wireframe rendering.
- `GlRenderer::set_lightmap_debug(true)` draws lightmaps instead of textured
  surfaces.

## Upstream
See docs/upstream.md for the exact upstream commit.

## License
This project is licensed under GPL-2.0-only to match the upstream QuakeWorld
source. See LICENSE or COPYING.
