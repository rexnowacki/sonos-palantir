# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Name & Theme

This project is called **sono-palantir** — Gandalf/wizard/Lord of the Rings themed Sonos TUI.

## Commit Workflow (MANDATORY)

After completing any feature, bug fix, or meaningful set of changes:

1. **Write a LOTR/Gandalf/wizard themed commit message.** Channel Tolkien. Use quotes, lore, character voice, dramatic imagery. Examples:
   - `"feat: you shall not skip — UPnP errors now return 422 instead of 500"`
   - `"fix: even the very wise cannot see all ends — follower speakers now show coordinator's track"`
   - `"perf: fly, you fools — commands no longer blocked by the great eye of soco.discover()"`
2. **Push to main** (`git push origin main`) immediately after committing.

This applies after every logical unit of work. Do not wait to be asked.

## Project Status

Fully implemented. Both daemon and TUI are working against real hardware. See `todo.md` for the feature backlog, `dream_features.md` for the long-term wishlist.

## Commands

### Daemon
```bash
cd daemon
python -m venv .venv && source .venv/bin/activate
pip install -e .
sonosd                              # Start daemon on localhost:9271
curl localhost:9271/speakers | jq . # Verify
```

### TUI
```bash
cd tui
cargo build --release
./target/release/sonos-tui
```

### Tests
```bash
cd daemon && source .venv/bin/activate && pytest -q
cd tui && cargo test
```

## Architecture

```
Ratatui TUI (Rust)  ──HTTP/JSON──>  sonosd (Python/FastAPI)  ──UPnP/SOAP──>  Sonos speakers
     tui/src/                           daemon/sonosd/                         (local LAN)
```

The Python daemon handles all Sonos UPnP via `soco`. The Rust TUI is a thin async client that polls every 2s via a background tokio task (never blocking the event loop).

### Daemon internals (`daemon/sonosd/`)
- `sonos.py` — `SonosManager`: discovery runs once at startup + every 30s in a background thread; `get_speaker_info()` falls back to coordinator's track for grouped followers
- `server.py` — FastAPI routes; `/speakers` reads cached dict (no blocking discovery per request)
- `config.yaml` — speaker and playlist aliases, host/port

### TUI internals (`tui/src/`)
- `main.rs` — Event loop (100ms tick); background `tokio::spawn` refresh via `mpsc::channel`; `handle_key` intercepts volume input mode before normal keys
- `app.rs` — `App` struct: speakers, playlists, active panel, selection indices, `volume_input: Option<String>`
- `api.rs` — `Arc<ApiClient>` wrapping `reqwest`; all methods async
- `ui.rs` — Three-panel layout; `◈` = coordinator, `↳` = follower in speaker list; help bar shows `Vol: [##▌]` in volume input mode

## Key Design Decisions

- **Speaker/playlist aliases**: `config.yaml` maps short names to real Sonos names.
- **Group routing**: All soco `@only_on_master` methods (play, pause, next, prev, stop) go through `get_coordinator()` — never call on followers directly.
- **Volume**: Daemon accepts absolute 0–100; TUI computes relative adjustments. Press `v` for exact input.
- **Favorites**: Daemon can only play Sonos Favorites. `GET /favorites` merges unaliased ones into the playlists panel on TUI startup.
- **Networking**: Daemon and Sonos speakers must be on the same LAN — no VPN.
