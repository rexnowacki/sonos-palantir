# Sonos TUI — Design Document

**Date:** 2026-02-27

## Goal

Implement the project described in `sonos_project.md` exactly as specified: a Python daemon (`sonosd`) plus a Rust terminal UI (`sonos-tui`) for controlling Sonos speakers from the terminal.

## Approach

Daemon first, then TUI. Build and verify the Python daemon against real hardware before writing any Rust. This makes debugging straightforward — if something breaks in the TUI, the API is already known-good.

## Configuration

```yaml
speakers:
  cthulhu: "cthulhu"

playlists:
  altwave: "Alt Wave"

default_speaker: cthulhu
default_volume: 25
host: "127.0.0.1"
port: 9271
```

## Directory Structure

```
sonos/
├── sonos_project.md
├── CLAUDE.md
├── docs/plans/
├── daemon/
│   ├── pyproject.toml
│   ├── config.yaml
│   └── sonosd/
│       ├── __init__.py
│       ├── server.py
│       ├── sonos.py
│       └── models.py
└── tui/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── app.rs
        ├── ui.rs
        ├── api.rs
        └── theme.rs
```

Note: `widgets/` subdirectory from the spec's structure diagram is not used — all rendering lives in `ui.rs` as the spec's actual code shows.

## Phase 1: Python Daemon

- Scaffold `daemon/` with `pyproject.toml`, `config.yaml`, and `sonosd/` package
- Implement `sonos.py` (`SonosManager`), `server.py` (FastAPI routes), `models.py` (Pydantic models)
- Use plain `venv` + `pip install -e .`
- Verify with `curl localhost:9271/speakers | jq .`

## Phase 2: Rust TUI

- Scaffold `tui/` with `Cargo.toml` and `src/`
- Implement `api.rs`, `app.rs`, `ui.rs`, `main.rs` from spec
- Build with `cargo build --release`
- Run against the live daemon

## Toolchain

- Python: `venv` + `pip` (plain, no uv/Poetry)
- Rust: standard `cargo`
- Testing: real Sonos speaker ("cthulhu") on local network
