# sonos-tui

A terminal interface for controlling Sonos speakers. Python daemon wraps `soco` and exposes a JSON REST API; Rust TUI renders it with Ratatui.

```
┌─ Speakers ──────────────────┬─ Now Playing ─────────────────┐
│ ► cthulhu       ■ 25  ▶     │                               │
│   family        ■ 30  ▶     │  ♫ Penny in the Lake          │
│                              │  Ratboys                      │
│                              │  Penny in the Lake            │
│                              │                               │
│                              │  1:23 ━━━━━━━━━━━━━━━ 3:51   │
│                              │                               │
├─ Playlists ─────────────────┤  Vol: ████████░░░░░░ 25       │
│ ► altwave  Alt Wave         │                               │
└──────────────────────────────┴───────────────────────────────┘
│ Tab panel  ↑↓ nav  Enter play  Space pause  +/- vol  g group  q quit │
```

## Architecture

```
Ratatui TUI (Rust)  ──HTTP/JSON──>  sonosd (Python/FastAPI)  ──UPnP──>  Sonos speakers
     tui/                               daemon/                          (local network)
```

The daemon handles all Sonos communication via `soco`. The TUI is a thin client that polls the daemon every 2 seconds.

## Setup

### Daemon

```bash
cd daemon
python -m venv .venv && source .venv/bin/activate
pip install -e .
```

Edit `daemon/config.yaml` to match your setup:

```yaml
playlists:
  altwave: "Alt Wave"      # short alias: exact Sonos Favorite name

speakers:
  cthulhu: "cthulhu"
  family: "Family Room"

default_speaker: cthulhu
default_volume: 25
host: "127.0.0.1"
port: 9271
```

Playlists must be added to Sonos Favorites via the Sonos iOS/Android app first.

Run the daemon:

```bash
sonosd
```

### TUI

```bash
cd tui
cargo build --release
./target/release/sonos-tui
```

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` | Cycle panels (Speakers → Playlists → Now Playing) |
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `Enter` | Play selected playlist on selected speaker |
| `Space` | Pause / resume |
| `+` / `=` | Volume up 5 |
| `-` | Volume down 5 |
| `n` | Next track |
| `p` | Previous track |
| `g` | Toggle group all speakers |
| `q` | Quit |

## Running tests

```bash
# Daemon
cd daemon && source .venv/bin/activate && pytest

# TUI
cd tui && cargo test
```

## Requirements

- Python 3.11+
- Rust 1.88+
- Sonos speakers on the same LAN (no VPN)
