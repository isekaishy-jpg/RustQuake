# Game Data Paths

RustQuake reads the original game data from a local install. The data files
must remain local and should not be committed to GitHub.

## Options
1) Environment variable: set `RUSTQUAKE_DATA_DIR` to your Quake install path.
2) Config file: copy `config/data_paths.example.toml` to
   `config/data_paths.toml` and set `quake_dir`.
3) CLI: pass `--data-dir <path>` to override discovery.

Downloads are written under `RUSTQUAKE_DOWNLOAD_DIR` (or via
`--download-dir <path>`). They are stored per-game-dir and should be kept
out of version control.

## Known Steam layout (Windows)
`C:/Program Files (x86)/Steam/steamapps/common/Quake`

Base content is in `id1/PAK0.PAK` and `id1/PAK1.PAK`.
