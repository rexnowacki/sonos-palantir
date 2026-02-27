# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Status

This repository contains a single spec file (`sonos_project.md`) that is a complete blueprint for a Sonos TUI project. **No code has been implemented yet.** The spec includes full working code for both components — the task is to scaffold the directory structure and implement from it.

## What to Build

A two-process system for controlling Sonos speakers from the terminal:

- **`daemon/`** — Python FastAPI service (`sonosd`) that wraps the `soco` library and exposes a JSON REST API on `localhost:9271`
- **`tui/`** — Rust terminal UI using Ratatui that talks to the daemon over HTTP

## Commands (once implemented)

### Daemon
```bash
cd daemon
python -m venv .venv && source .venv/bin/activate
pip install -e .
sonosd                              # Start daemon on localhost:9271
curl localhost:9271/speakers | jq . # Verify it's working
```

### TUI
```bash
cd tui
cargo build --release
./target/release/sonos-tui
```

## Architecture

```
Ratatui TUI (Rust)  ──HTTP/JSON──>  sonosd (Python/FastAPI)  ──UPnP/SOAP──>  Sonos speakers
```

The Python daemon exists because `soco` (the best Sonos control library) is Python-only. The Rust TUI exists because Ratatui produces beautiful terminal UIs. They communicate over a simple JSON API — each process can be developed and tested independently.

### Daemon internals (`daemon/sonosd/`)
- `sonos.py` — `SonosManager` class: speaker discovery via `soco.discover()`, alias resolution, `play_favorite()` resolves aliases to Sonos Favorites
- `server.py` — FastAPI routes; config loaded from `config.yaml` on startup
- `models.py` — Pydantic response models

### TUI internals (`tui/src/`)
- `main.rs` — Event loop: 100ms tick rate, 2s polling interval for speaker state refresh
- `app.rs` — `App` struct holds `Vec<Speaker>`, `Vec<Playlist>`, active panel, selection indices
- `api.rs` — `ApiClient` wrapping `reqwest`; all methods are `async`
- `ui.rs` — Three-panel layout (45% left / 55% right), color palette constants at top

## Key Design Decisions from the Spec

- **Speaker/playlist aliases**: `config.yaml` maps short names (e.g., `"office"`) to real Sonos names. The `get_speaker()` method tries alias first, falls back to raw name.
- **Volume**: Daemon accepts absolute 0–100 values; TUI computes relative adjustments before sending.
- **`POST /play`** requires both a `speaker` and a `playlist` — plays a Sonos Favorite by alias or exact title.
- **Grouping**: First speaker in the list becomes the coordinator; use `["all"]` to group/ungroup everything.
- **Favorites only**: The daemon can only play content added to Sonos Favorites via the Sonos app.

## Networking Requirements

Daemon and Sonos speakers must be on the same LAN — no VPN, no Docker host networking.
