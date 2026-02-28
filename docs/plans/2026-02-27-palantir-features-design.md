# sono-palantir Feature Suite Design

> *"All we have to decide is what to do with the features that are given to us."*

**Date:** 2026-02-27
**Status:** Approved
**Approach:** TUI-centric (Approach A)

---

## Features in Scope

1. Play history — track playlist plays, sort by popularity (last 7 days)
2. Group topology visualizer + multi-group Now Playing
3. "Speak, Friend" command mode with ghost text autocomplete
4. Config hot-reload (auto every 300s + `:reload` command)
5. Startup splash, LOTR error messages, `?` help screen

*Out of scope for now: stereo pair detection.*

---

## 1. Layout

The outer vertical layout gains a permanent **status line** (height 1) between the main panels and the help bar.

```
┌─ Speakers/Topology ─┬─ Now Playing (dynamic) ──────────┐
│ ╔═ Fellowship ═════╗ │  ╔═ Fellowship ════════════════╗ │
│ ║ ► cthulhu  ◈    ║ │  ║ ♫ Penny in the Lake         ║ │
│ ║   family   ↳    ║ │  ║ Ratboys  ·  1:23 / 3:51     ║ │
│ ╚═════════════════╝ │  ║ Vol: ████░░ 25               ║ │
│   hermit (solo)     │  ╚═════════════════════════════╝ │
├─ Playlists ─────────┤  (hermit — Nothing playing)      │
│ ► altwave  Alt Wave │                                   │
└─────────────────────┴───────────────────────────────────┘
 Sleep: 28:14 remaining · Reloaded config.yaml             ← status line (height 1)
 Tab panel  ↑↓ nav  :cmd  ?help  v vol#  g group  q quit   ← help bar
```

### Speakers panel — topology mode
When any speaker is grouped, the flat speaker list is replaced by topology ASCII art:

```
╔═ Fellowship ═════╗
║ ► cthulhu  ◈    ║
║   family   ↳    ║
╚═════════════════╝
  hermit (solo)
```

When no speakers are grouped, the existing flat list renders as before.

### Now Playing panel — dynamic multi-group
One stacked block per distinct group + each ungrouped solo speaker. Scrollable if they don't all fit. When a single group or solo exists, renders identically to the current single Now Playing view.

### Status line
A single borderless line, dim by default. Used for:
- LOTR-flavored error messages (see §5)
- Sleep countdown: `Sleep: 28:14 remaining`
- Config reload confirmation: `Reloaded config.yaml` (shown for 3 seconds)

### Help bar / command input
Unchanged in normal mode. Replaced by command prompt when `:` is pressed (see §3).

---

## 2. Play History

### Storage
File: `~/.config/sono-palantir/history.json`
Format: append-only JSON array, trimmed to last 90 days on each write.

```json
[
  {"playlist": "altwave", "played_at": "2026-02-27T21:04:00Z"},
  {"playlist": "altwave", "played_at": "2026-02-28T09:12:00Z"},
  {"playlist": "jazzclassics", "played_at": "2026-02-28T11:00:00Z"}
]
```

A play is recorded when the user triggers playback via `Enter` in the playlists panel or `:play <name>` in command mode.

### Sorting
`config.yaml` new key:
```yaml
playlist_sort: popularity   # or: alphabetical (default if omitted)
```

On startup, if `playlist_sort: popularity`, the TUI reads `history.json`, counts plays per playlist in the last 7 days, and sorts the playlists panel descending by count. Ties broken alphabetically.

---

## 3. Command Mode

### Activation
Press `:` from any panel. The help bar area is replaced by a command prompt. `Esc` cancels. `Enter` executes.

### Prompt appearance
```
: play lord of▌the rings radio            [Tab] complete  [Esc] cancel
```
- Typed text in normal foreground color
- Ghost text (autocomplete suggestion) in DIM color
- `Tab` accepts ghost text
- Status line shows error if command is unknown/invalid

### Supported commands

| Command | Action |
|---|---|
| `:play <name>` | Fuzzy-match against loaded favorites, play on selected speaker |
| `:vol <0-100>` | Set exact volume on selected speaker |
| `:group all` | Group all speakers |
| `:ungroup` | Ungroup all speakers |
| `:next` | Skip to next track |
| `:prev` | Previous track |
| `:sleep <minutes>` | Start sleep timer countdown; pause all speakers at 0 |
| `:sleep 0` / `:sleep cancel` | Cancel active sleep timer |
| `:reload` | Reload `config.yaml` immediately via `POST /reload` |

### Autocomplete logic
- `:play <text>` — fuzzy-matches `<text>` against `app.playlists` (alias + favorite_name). First match becomes ghost text.
- Command name only (e.g., `:s` → ghost `leep`, `:sl` → ghost `eep`) for all other commands.
- No match → no ghost text shown.

### Sleep timer
- Stored in `App` as `sleep_until: Option<Instant>`
- Status line shows `Sleep: MM:SS remaining` updated each tick
- On expiry: `POST /pause` all speakers, clear `sleep_until`
- `:sleep 0` or `:sleep cancel` clears `sleep_until` and removes status line message

---

## 4. Config Hot-Reload

### Daemon side
- `SonosManager` stores `_config_mtime: f64` (file modification timestamp)
- The existing background discovery thread (runs every 30s) also checks mtime every 300s
- If mtime changed: re-read `config.yaml`, update `_alias_map`, `_reverse_alias`, `_playlist_map` under `_lock`
- New endpoint: `POST /reload` — triggers immediate re-read, responds `{"status": "reloaded"}`

### TUI side
- `:reload` command sends `POST /reload`, then triggers an immediate refresh of `/playlists` + `/favorites`
- After either auto or manual reload, status line shows `Reloaded config.yaml` for 3 seconds

---

## 5. Polish

### Startup splash
Renders for 1 second (blocking) before the main TUI loop starts:

```
  ╔══════════════════════════════════════╗
  ║   S O N O - P A L A N T I R         ║
  ║   ══════════════════════════         ║
  ║   Seeing through sound...            ║
  ╚══════════════════════════════════════╝
```

Implemented as a separate `draw_splash()` pass on the terminal, followed by a `std::thread::sleep(Duration::from_secs(1))`, then the main loop begins.

### LOTR error messages (status line)
| Condition | Status line message |
|---|---|
| Daemon unreachable | *The gates of Moria are sealed.* |
| Speaker not found | *Not all those who wander are found in this network.* |
| UPnP error (422) | *Even the very wise cannot see all ends.* |
| Volume set to 100 | *You shall not pass... 100.* |
| Unknown command | *Speak, friend — but speak clearly.* |

Messages persist until the next status update or for 5 seconds, whichever comes first.

### Help screen
`?` toggles a full-screen overlay rendered over the main panels. Lists all keybindings and `:commands` with LOTR flavor text beside each. `?` or `Esc` dismisses.

---

## Files Touched

### Daemon (`daemon/`)
- `sonosd/sonos.py` — add mtime tracking + reload logic to background thread
- `sonosd/server.py` — add `POST /reload` endpoint

### TUI (`tui/src/`)
- `main.rs` — startup splash, sleep timer tick, command mode key handling, status line management, `:reload` command sending
- `app.rs` — add `command_input: Option<String>`, `sleep_until: Option<Instant>`, `status_message_until: Option<Instant>`, `help_open: bool`, playlist sort logic
- `api.rs` — add `reload()` method
- `ui.rs` — topology view in speakers panel, stacked Now Playing, status line row, command prompt in help bar, splash screen, help overlay

### New files
- `~/.config/sono-palantir/history.json` — created by TUI on first play
- `tui/src/history.rs` — read/write/trim history, compute popularity sort
- `tui/src/command.rs` — parse command string, fuzzy match, return `Command` enum
