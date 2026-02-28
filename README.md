# sono-palantir

A Lord of the Rings themed terminal interface for controlling Sonos speakers. Python daemon wraps `soco` and exposes a JSON REST API; Rust TUI renders it with Ratatui.

```
┌─ Speakers/Topology ─┬─ Now Playing ─────────────────────────────┐
│ ╔═ Fellowship ═════╗ │  ╔═ Fellowship ══════════════════════════╗ │
│ ║ ► cthulhu  ◈    ║ │  ║ ♫ Penny in the Lake                  ║ │
│ ║   family   ↳    ║ │  ║ Ratboys  ·  1:23 / 3:51              ║ │
│ ╚═════════════════╝ │  ║ Vol: ████░░ 25                        ║ │
│   hermit (solo) ·   │  ╚══════════════════════════════════════╝ │
├─ Playlists ─────────┤  (hermit — Nothing playing)               │
│ ► altwave  Alt Wave │                                            │
└─────────────────────┴────────────────────────────────────────────┘
 Sleep: 28:14 remaining · The fellowship is assembled.
 Tab panel  ↑↓ nav  : cmd  ? help  v vol#  g group  q quit
```

## Architecture

```
Ratatui TUI (Rust)  ──HTTP/JSON──>  sonosd (Python/FastAPI)  ──UPnP──>  Sonos speakers
     tui/                               daemon/                          (local network)
```

The daemon handles all Sonos communication via `soco`. The TUI is a thin async client that polls the daemon every 2 seconds via a background tokio task — the event loop never blocks.

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

# playlist_sort: popularity   # options: alphabetical (default), popularity
```

Playlists must be added to Sonos Favorites via the Sonos iOS/Android app first. Any Favorites not in `config.yaml` are merged in automatically on startup.

Run the daemon:

```bash
sonosd
```

The daemon re-reads `config.yaml` automatically every 5 minutes, or immediately via `:reload`.

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
| `v` | Set exact volume (type digits, Enter to confirm) |
| `n` | Next track |
| `p` | Previous track |
| `g` | Toggle group all speakers |
| `:` | Enter command mode (see below) |
| `?` | Toggle help screen |
| `q` | Quit |

## Command Mode

Press `:` to enter command mode. Ghost text autocomplete appears as you type; press `Tab` to accept.

| Command | Action |
|---------|--------|
| `:play <name>` | Fuzzy-match a favorite and play it |
| `:vol <0-100>` | Set exact volume |
| `:group all` | Group all speakers |
| `:ungroup` | Ungroup all speakers |
| `:next` | Skip to next track |
| `:prev` | Previous track |
| `:sleep <minutes>` | Sleep timer — pauses all speakers after N minutes |
| `:sleep cancel` | Cancel active sleep timer |
| `:reload` | Reload `config.yaml` immediately |

Press `Esc` to cancel.

## Features

- **Group topology view** — when speakers are grouped, the Speakers panel shows a live ASCII topology map (`◈` coordinator, `↳` follower)
- **Multi-group Now Playing** — stacked track blocks, one per active group and solo speaker
- **Play history** — tracks which playlists you play; set `playlist_sort: popularity` in `config.yaml` to sort by 7-day play count
- **Sleep timer** — countdown shown in the status line; all speakers pause on expiry
- **Config hot-reload** — automatic every 5 minutes, or on demand via `:reload`
- **LOTR error messages** — the status line speaks in the voice of Middle-earth

## Running tests

```bash
# Daemon
cd daemon && source .venv/bin/activate && pytest

# TUI
cd tui && cargo test
```

## Requirements

- Python 3.11+
- Rust 1.70+
- Sonos speakers on the same LAN (no VPN)
