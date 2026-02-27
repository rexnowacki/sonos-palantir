# Sonos TUI

A beautiful terminal interface for controlling Sonos speakers, built with Ratatui (Rust) talking to a lightweight Python daemon that wraps `soco`.

## Architecture

```
┌─────────────────────────────────┐
│   Ratatui TUI (Rust)            │
│   Beautiful terminal interface  │
│   Keyboard-driven controls      │
│   Real-time speaker status      │
└──────────┬──────────────────────┘
           │ HTTP (localhost:9271)
┌──────────▼──────────────────────┐
│   sonosd — Python Daemon        │
│   Wraps soco library            │
│   UPnP discovery & control      │
│   JSON REST API                 │
└─────────────────────────────────┘
```

Two processes, one system. The Python daemon does the ugly UPnP/SOAP work via `soco`. The Rust TUI does the beautiful rendering via `ratatui`. They talk over a simple JSON API on localhost.

---

## Project Structure

```
sonos-tui/
├── daemon/                     # Python — sonosd
│   ├── pyproject.toml
│   ├── sonosd/
│   │   ├── __init__.py
│   │   ├── server.py           # FastAPI app
│   │   ├── sonos.py            # soco wrapper / speaker manager
│   │   └── models.py           # Pydantic response models
│   └── config.yaml             # playlist aliases, defaults
│
├── tui/                        # Rust — sonos-tui
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # entry point, event loop
│       ├── app.rs              # app state
│       ├── ui.rs               # layout and rendering
│       ├── api.rs              # HTTP client to daemon
│       ├── widgets/
│       │   ├── speakers.rs     # speaker list panel
│       │   ├── now_playing.rs  # current track display
│       │   ├── playlists.rs    # playlist browser
│       │   └── volume.rs       # volume bar widget
│       └── theme.rs            # colors and style constants
│
└── README.md
```

---

## Part 1: Python Daemon (`sonosd`)

Minimal. Its only job is bridging `soco` to a JSON API. No rendering, no UI, no opinions.

### Dependencies

```toml
[project]
name = "sonosd"
version = "0.1.0"
requires-python = ">=3.11"
dependencies = [
    "soco>=0.30",
    "fastapi>=0.110",
    "uvicorn>=0.29",
    "pyyaml>=6.0",
]

[project.scripts]
sonosd = "sonosd.server:main"
```

### Config (`config.yaml`)

```yaml
# Playlist aliases — map short names to Sonos Favorites
playlists:
  lotr: "LOTR Complete Soundtrack"
  shire: "Shire Ambience"
  focus: "Deep Focus"
  cubs: "Cubs Walk-Up Songs"
  metal: "Blackened Everything"
  chill: "Late Night Lo-Fi"

# Speaker aliases (optional — TUI also shows raw names)
speakers:
  office: "Office"
  bedroom: "Bedroom"
  kitchen: "Kitchen"
  living: "Living Room"

# Defaults
default_speaker: office
default_volume: 25

# Server
host: "127.0.0.1"
port: 9271
```

### API Endpoints

All responses are JSON.

#### `GET /speakers`

Returns all discovered speakers with current state.

```json
{
  "speakers": [
    {
      "name": "Office",
      "alias": "office",
      "ip": "192.168.1.42",
      "volume": 25,
      "muted": false,
      "state": "PLAYING",
      "group_coordinator": "Office",
      "track": {
        "title": "Concerning Hobbits",
        "artist": "Howard Shore",
        "album": "The Lord of the Rings: The Fellowship of the Ring",
        "duration": 163,
        "position": 47,
        "art_uri": "http://..."
      }
    },
    {
      "name": "Kitchen",
      "alias": "kitchen",
      "ip": "192.168.1.43",
      "volume": 30,
      "muted": false,
      "state": "STOPPED",
      "group_coordinator": null,
      "track": null
    }
  ]
}
```

#### `GET /favorites`

Returns all Sonos Favorites available on the network.

```json
{
  "favorites": [
    { "title": "LOTR Complete Soundtrack", "type": "playlist" },
    { "title": "Deep Focus", "type": "playlist" },
    { "title": "NPR News", "type": "radio" }
  ]
}
```

#### `GET /playlists`

Returns configured playlist aliases from config.

```json
{
  "playlists": {
    "lotr": "LOTR Complete Soundtrack",
    "shire": "Shire Ambience",
    "focus": "Deep Focus"
  }
}
```

#### `POST /play`

```json
{
  "speaker": "office",
  "playlist": "lotr"
}
```

`speaker` can be an alias, a raw speaker name, or `"all"`. `playlist` can be an alias or a raw Sonos Favorite name. Returns `200` with the resolved speaker/playlist names or `404` if not found.

#### `POST /pause`

```json
{ "speaker": "office" }
```

#### `POST /resume`

```json
{ "speaker": "office" }
```

#### `POST /stop`

```json
{ "speaker": "office" }
```

#### `POST /volume`

```json
{
  "speaker": "office",
  "volume": 30
}
```

Absolute value (0–100). For relative adjustments, the TUI handles the math and sends the absolute result.

#### `POST /group`

```json
{
  "speakers": ["office", "bedroom", "kitchen"]
}
```

First speaker becomes coordinator. Use `["all"]` to group everything.

#### `POST /ungroup`

```json
{
  "speaker": "bedroom"
}
```

Omit `speaker` or pass `"all"` to ungroup everything.

#### `POST /next`

```json
{ "speaker": "office" }
```

#### `POST /previous`

```json
{ "speaker": "office" }
```

### Implementation (`sonosd/sonos.py`)

```python
import soco
from dataclasses import dataclass

class SonosManager:
    """Manages speaker discovery and provides control methods."""

    def __init__(self, config: dict):
        self.config = config
        self._speakers: dict[str, soco.SoCo] = {}
        self._alias_map: dict[str, str] = config.get("speakers", {})
        self._reverse_alias: dict[str, str] = {v: k for k, v in self._alias_map.items()}
        self._playlist_map: dict[str, str] = config.get("playlists", {})
        self.refresh()

    def refresh(self) -> None:
        """Re-discover speakers on the network."""
        discovered = soco.discover(timeout=5)
        if discovered:
            self._speakers = {sp.player_name: sp for sp in discovered}

    def get_speaker(self, name_or_alias: str) -> soco.SoCo:
        """Resolve alias or name to a SoCo instance."""
        # Try alias first
        real_name = self._alias_map.get(name_or_alias, name_or_alias)
        if real_name in self._speakers:
            return self._speakers[real_name]
        raise KeyError(f"Speaker not found: {name_or_alias}")

    def get_all_speakers(self) -> dict[str, soco.SoCo]:
        return self._speakers

    def get_speaker_info(self, speaker: soco.SoCo) -> dict:
        """Build the full status dict for a speaker."""
        info = speaker.get_current_transport_info()
        track_info = speaker.get_current_track_info()

        track = None
        if track_info.get("title"):
            track = {
                "title": track_info.get("title", ""),
                "artist": track_info.get("artist", ""),
                "album": track_info.get("album", ""),
                "duration": _parse_duration(track_info.get("duration", "0:00:00")),
                "position": _parse_duration(track_info.get("position", "0:00:00")),
                "art_uri": track_info.get("album_art", ""),
            }

        coordinator = speaker.group.coordinator.player_name if speaker.group else None

        return {
            "name": speaker.player_name,
            "alias": self._reverse_alias.get(speaker.player_name),
            "ip": speaker.ip_address,
            "volume": speaker.volume,
            "muted": speaker.mute,
            "state": info.get("current_transport_state", "UNKNOWN"),
            "group_coordinator": coordinator,
            "track": track,
        }

    def play_favorite(self, speaker: soco.SoCo, favorite_name: str) -> None:
        """Play a Sonos Favorite by exact name or alias."""
        # Resolve alias
        resolved = self._playlist_map.get(favorite_name, favorite_name)

        favorites = speaker.music_library.get_sonos_favorites()
        match = None
        for fav in favorites:
            if fav.title.lower() == resolved.lower():
                match = fav
                break

        if not match:
            available = [f.title for f in favorites]
            raise KeyError(
                f"Favorite '{resolved}' not found. Available: {available}"
            )

        uri = match.reference.get_uri()
        meta = match.resource_meta_data
        speaker.play_uri(uri, meta)

    def group_speakers(self, names_or_aliases: list[str]) -> soco.SoCo:
        """Group speakers. First becomes coordinator."""
        if names_or_aliases == ["all"]:
            speakers = list(self._speakers.values())
        else:
            speakers = [self.get_speaker(n) for n in names_or_aliases]

        coordinator = speakers[0]
        for sp in speakers[1:]:
            sp.join(coordinator)
        return coordinator

    def ungroup(self, name_or_alias: str | None = None) -> None:
        """Ungroup a specific speaker, or all."""
        if name_or_alias is None or name_or_alias == "all":
            for sp in self._speakers.values():
                sp.unjoin()
        else:
            self.get_speaker(name_or_alias).unjoin()


def _parse_duration(time_str: str) -> int:
    """Parse 'H:MM:SS' to total seconds."""
    parts = time_str.split(":")
    if len(parts) == 3:
        return int(parts[0]) * 3600 + int(parts[1]) * 60 + int(parts[2])
    return 0
```

### Implementation (`sonosd/server.py`)

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import uvicorn
import yaml
from pathlib import Path
from .sonos import SonosManager

app = FastAPI(title="sonosd")
manager: SonosManager = None


class PlayRequest(BaseModel):
    speaker: str
    playlist: str

class SpeakerRequest(BaseModel):
    speaker: str = "all"

class VolumeRequest(BaseModel):
    speaker: str
    volume: int

class GroupRequest(BaseModel):
    speakers: list[str]


@app.on_event("startup")
def startup():
    global manager
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    manager = SonosManager(config)


@app.get("/speakers")
def get_speakers():
    manager.refresh()
    speakers = []
    for name, sp in manager.get_all_speakers().items():
        try:
            speakers.append(manager.get_speaker_info(sp))
        except Exception:
            speakers.append({"name": name, "error": "unreachable"})
    return {"speakers": speakers}


@app.get("/favorites")
def get_favorites():
    speakers = list(manager.get_all_speakers().values())
    if not speakers:
        raise HTTPException(404, "No speakers found")
    favs = speakers[0].music_library.get_sonos_favorites()
    return {"favorites": [{"title": f.title} for f in favs]}


@app.get("/playlists")
def get_playlists():
    return {"playlists": manager._playlist_map}


@app.post("/play")
def play(req: PlayRequest):
    try:
        if req.speaker == "all":
            speaker = manager.group_speakers(["all"])
        else:
            speaker = manager.get_speaker(req.speaker)
        manager.play_favorite(speaker, req.playlist)
        return {"status": "playing", "speaker": speaker.player_name, "playlist": req.playlist}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/pause")
def pause(req: SpeakerRequest):
    try:
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.pause()
        else:
            manager.get_speaker(req.speaker).pause()
        return {"status": "paused"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/resume")
def resume(req: SpeakerRequest):
    try:
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.play()
        else:
            manager.get_speaker(req.speaker).play()
        return {"status": "resumed"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/stop")
def stop(req: SpeakerRequest):
    try:
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.stop()
        else:
            manager.get_speaker(req.speaker).stop()
        return {"status": "stopped"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/volume")
def set_volume(req: VolumeRequest):
    try:
        vol = max(0, min(100, req.volume))
        if req.speaker == "all":
            for sp in manager.get_all_speakers().values():
                sp.volume = vol
        else:
            manager.get_speaker(req.speaker).volume = vol
        return {"status": "ok", "volume": vol}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/group")
def group(req: GroupRequest):
    try:
        coordinator = manager.group_speakers(req.speakers)
        return {"status": "grouped", "coordinator": coordinator.player_name}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/ungroup")
def ungroup(req: SpeakerRequest):
    manager.ungroup(req.speaker)
    return {"status": "ungrouped"}


@app.post("/next")
def next_track(req: SpeakerRequest):
    try:
        manager.get_speaker(req.speaker).next()
        return {"status": "ok"}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/previous")
def prev_track(req: SpeakerRequest):
    try:
        manager.get_speaker(req.speaker).previous()
        return {"status": "ok"}
    except KeyError as e:
        raise HTTPException(404, str(e))


def main():
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    host = config.get("host", "127.0.0.1")
    port = config.get("port", 9271)
    uvicorn.run(app, host=host, port=port)
```

### Running the Daemon

```bash
cd daemon
python -m venv .venv
source .venv/bin/activate
pip install -e .

# Start the daemon
sonosd
# → Uvicorn running on http://127.0.0.1:9271

# Test it
curl http://localhost:9271/speakers | python -m json.tool
```

---

## Part 2: Ratatui TUI (`sonos-tui`)

This is where you make it beautiful.

### Dependencies (`Cargo.toml`)

```toml
[package]
name = "sonos-tui"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

### Data Models (`api.rs`)

```rust
use serde::{Deserialize, Serialize};

const BASE_URL: &str = "http://127.0.0.1:9271";

#[derive(Debug, Clone, Deserialize)]
pub struct Speaker {
    pub name: String,
    pub alias: Option<String>,
    pub ip: String,
    pub volume: u8,
    pub muted: bool,
    pub state: String,
    pub group_coordinator: Option<String>,
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub position: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Playlist {
    pub alias: String,
    pub favorite_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayRequest {
    pub speaker: String,
    pub playlist: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeakerRequest {
    pub speaker: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VolumeRequest {
    pub speaker: String,
    pub volume: u8,
}

pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: BASE_URL.to_string(),
        }
    }

    pub async fn get_speakers(&self) -> anyhow::Result<Vec<Speaker>> {
        let resp: serde_json::Value = self.client
            .get(format!("{}/speakers", self.base_url))
            .send().await?
            .json().await?;
        let speakers: Vec<Speaker> = serde_json::from_value(resp["speakers"].clone())?;
        Ok(speakers)
    }

    pub async fn get_playlists(&self) -> anyhow::Result<Vec<Playlist>> {
        let resp: serde_json::Value = self.client
            .get(format!("{}/playlists", self.base_url))
            .send().await?
            .json().await?;
        let map: std::collections::HashMap<String, String> =
            serde_json::from_value(resp["playlists"].clone())?;
        Ok(map.into_iter().map(|(alias, favorite_name)| {
            Playlist { alias, favorite_name }
        }).collect())
    }

    pub async fn play(&self, speaker: &str, playlist: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/play", self.base_url))
            .json(&PlayRequest {
                speaker: speaker.to_string(),
                playlist: playlist.to_string(),
            })
            .send().await?;
        Ok(())
    }

    pub async fn pause(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/pause", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn resume(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/resume", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn set_volume(&self, speaker: &str, volume: u8) -> anyhow::Result<()> {
        self.client.post(format!("{}/volume", self.base_url))
            .json(&VolumeRequest {
                speaker: speaker.to_string(),
                volume,
            })
            .send().await?;
        Ok(())
    }

    pub async fn next(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/next", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }

    pub async fn previous(&self, speaker: &str) -> anyhow::Result<()> {
        self.client.post(format!("{}/previous", self.base_url))
            .json(&SpeakerRequest { speaker: speaker.to_string() })
            .send().await?;
        Ok(())
    }
}
```

### App State (`app.rs`)

```rust
use crate::api::{Speaker, Playlist};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Panel {
    Speakers,
    Playlists,
    NowPlaying,
}

pub struct App {
    pub speakers: Vec<Speaker>,
    pub playlists: Vec<Playlist>,
    pub active_panel: Panel,
    pub speaker_index: usize,
    pub playlist_index: usize,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub last_refresh: std::time::Instant,
}

impl App {
    pub fn new() -> Self {
        Self {
            speakers: vec![],
            playlists: vec![],
            active_panel: Panel::Speakers,
            speaker_index: 0,
            playlist_index: 0,
            should_quit: false,
            status_message: None,
            last_refresh: std::time::Instant::now(),
        }
    }

    pub fn selected_speaker(&self) -> Option<&Speaker> {
        self.speakers.get(self.speaker_index)
    }

    pub fn selected_playlist(&self) -> Option<&Playlist> {
        self.playlists.get(self.playlist_index)
    }

    pub fn speaker_id(&self) -> Option<String> {
        self.selected_speaker().map(|s| {
            s.alias.clone().unwrap_or_else(|| s.name.clone())
        })
    }

    pub fn next_in_list(&mut self) {
        match self.active_panel {
            Panel::Speakers => {
                if !self.speakers.is_empty() {
                    self.speaker_index = (self.speaker_index + 1) % self.speakers.len();
                }
            }
            Panel::Playlists => {
                if !self.playlists.is_empty() {
                    self.playlist_index = (self.playlist_index + 1) % self.playlists.len();
                }
            }
            _ => {}
        }
    }

    pub fn prev_in_list(&mut self) {
        match self.active_panel {
            Panel::Speakers => {
                if !self.speakers.is_empty() {
                    self.speaker_index = self.speaker_index
                        .checked_sub(1)
                        .unwrap_or(self.speakers.len() - 1);
                }
            }
            Panel::Playlists => {
                if !self.playlists.is_empty() {
                    self.playlist_index = self.playlist_index
                        .checked_sub(1)
                        .unwrap_or(self.playlists.len() - 1);
                }
            }
            _ => {}
        }
    }

    pub fn cycle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Speakers => Panel::Playlists,
            Panel::Playlists => Panel::NowPlaying,
            Panel::NowPlaying => Panel::Speakers,
        };
    }
}
```

### TUI Layout (`ui.rs`)

Target layout:

```
┌─ Speakers ──────────────────┬─ Now Playing ─────────────────┐
│ ► Office        ■ 25  ▶     │                               │
│   Bedroom       ■ 20  ⏸     │  ♫ Concerning Hobbits         │
│   Kitchen       ■ 30  ·     │  Howard Shore                 │
│   Living Room   ■ 25  ·     │  The Fellowship of the Ring   │
│                              │                               │
│                              │   advancement bar here         │
│                              │  1:23 ━━━━━━━━━━━━━━━ 2:43   │
│                              │                               │
├─ Playlists ─────────────────┤  Volume: ████████░░░░░░ 25    │
│ ► lotr    LOTR Complete...  │                               │
│   shire   Shire Ambience    │                               │
│   focus   Deep Focus        │                               │
│   cubs    Cubs Walk-Up...   │                               │
│   metal   Blackened Every.. │                               │
│   chill   Late Night Lo-Fi  │                               │
├──────────────────────────────┴───────────────────────────────┤
│ [Tab] switch panel  [↑↓] navigate  [Enter] play  [Space]   │
│ pause/resume  [+/-] volume  [n/p] next/prev  [q] quit      │
└──────────────────────────────────────────────────────────────┘
```

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use crate::app::{App, Panel};

// Color palette — adjust to taste
const BG: Color = Color::Rgb(20, 20, 30);
const FG: Color = Color::Rgb(200, 200, 210);
const ACCENT: Color = Color::Rgb(130, 170, 255);    // soft blue
const PLAYING: Color = Color::Rgb(120, 220, 140);    // green
const PAUSED: Color = Color::Rgb(240, 200, 80);      // amber
const DIM: Color = Color::Rgb(80, 80, 100);
const HIGHLIGHT_BG: Color = Color::Rgb(40, 45, 65);
const BORDER_ACTIVE: Color = ACCENT;
const BORDER_INACTIVE: Color = Color::Rgb(50, 50, 70);

pub fn draw(f: &mut Frame, app: &App) {
    // Overall layout: top area + bottom help bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(f.area());

    // Top area: left column (speakers + playlists) | right column (now playing)
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(outer[0]);

    // Left column: speakers on top, playlists on bottom
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main[0]);

    draw_speakers(f, app, left[0]);
    draw_playlists(f, app, left[1]);
    draw_now_playing(f, app, main[1]);
    draw_help_bar(f, app, outer[1]);
}

fn panel_block(title: &str, active: bool) -> Block<'_> {
    let border_color = if active { BORDER_ACTIVE } else { BORDER_INACTIVE };
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG))
}

fn draw_speakers(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
    let block = panel_block("Speakers", active);

    let items: Vec<ListItem> = app.speakers.iter().enumerate().map(|(i, sp)| {
        let state_icon = match sp.state.as_str() {
            "PLAYING" => Span::styled("▶", Style::default().fg(PLAYING)),
            "PAUSED_PLAYBACK" => Span::styled("⏸", Style::default().fg(PAUSED)),
            _ => Span::styled("·", Style::default().fg(DIM)),
        };

        let display_name = sp.alias.as_deref().unwrap_or(&sp.name);
        let name_style = if i == app.speaker_index && active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };

        let line = Line::from(vec![
            Span::raw(if i == app.speaker_index { " ► " } else { "   " }),
            Span::styled(format!("{:<16}", display_name), name_style),
            Span::styled(format!(" {:>3}", sp.volume), Style::default().fg(DIM)),
            Span::raw("  "),
            state_icon,
        ]);

        let mut item = ListItem::new(line);
        if i == app.speaker_index && active {
            item = item.style(Style::default().bg(HIGHLIGHT_BG));
        }
        item
    }).collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_playlists(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Playlists;
    let block = panel_block("Playlists", active);

    let items: Vec<ListItem> = app.playlists.iter().enumerate().map(|(i, pl)| {
        let style = if i == app.playlist_index && active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };

        let line = Line::from(vec![
            Span::raw(if i == app.playlist_index { " ► " } else { "   " }),
            Span::styled(format!("{:<10}", pl.alias), style),
            Span::styled(
                truncate(&pl.favorite_name, 24),
                Style::default().fg(DIM),
            ),
        ]);

        let mut item = ListItem::new(line);
        if i == app.playlist_index && active {
            item = item.style(Style::default().bg(HIGHLIGHT_BG));
        }
        item
    }).collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_now_playing(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::NowPlaying;
    let block = panel_block("Now Playing", active);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let speaker = app.selected_speaker();

    // Check if the selected speaker has a track
    if let Some(sp) = speaker {
        if let Some(track) = &sp.track {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),  // spacing
                    Constraint::Length(1),  // track title
                    Constraint::Length(1),  // artist
                    Constraint::Length(1),  // album
                    Constraint::Length(2),  // spacing
                    Constraint::Length(1),  // progress bar
                    Constraint::Length(1),  // time display
                    Constraint::Length(2),  // spacing
                    Constraint::Length(1),  // volume bar
                    Constraint::Min(0),    // fill
                ])
                .split(inner);

            // Track title
            let title = Paragraph::new(Line::from(vec![
                Span::styled("  ♫ ", Style::default().fg(PLAYING)),
                Span::styled(
                    &track.title,
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
            ]));
            f.render_widget(title, chunks[1]);

            // Artist
            let artist = Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.artist, Style::default().fg(ACCENT)),
            ]));
            f.render_widget(artist, chunks[2]);

            // Album
            let album = Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.album, Style::default().fg(DIM)),
            ]));
            f.render_widget(album, chunks[3]);

            // Progress bar
            let ratio = if track.duration > 0 {
                track.position as f64 / track.duration as f64
            } else {
                0.0
            };
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(ACCENT).bg(Color::Rgb(40, 40, 55)))
                .ratio(ratio)
                .label("");
            // Indent the gauge
            let gauge_area = Rect {
                x: chunks[5].x + 4,
                width: chunks[5].width.saturating_sub(8),
                ..chunks[5]
            };
            f.render_widget(gauge, gauge_area);

            // Time
            let time_str = format!(
                "    {}  /  {}",
                format_time(track.position),
                format_time(track.duration),
            );
            let time = Paragraph::new(
                Span::styled(time_str, Style::default().fg(DIM))
            );
            f.render_widget(time, chunks[6]);

            // Volume
            let vol_ratio = sp.volume as f64 / 100.0;
            let vol_gauge = Gauge::default()
                .gauge_style(Style::default().fg(PLAYING).bg(Color::Rgb(40, 40, 55)))
                .ratio(vol_ratio)
                .label(format!("Vol: {}", sp.volume));
            let vol_area = Rect {
                x: chunks[8].x + 4,
                width: chunks[8].width.saturating_sub(8),
                ..chunks[8]
            };
            f.render_widget(vol_gauge, vol_area);

            return;
        }
    }

    // No track playing — show idle state
    let idle = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Nothing playing",
            Style::default().fg(DIM),
        )),
    ]);
    f.render_widget(idle, inner);
}

fn draw_help_bar(f: &mut Frame, _app: &App, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" Tab", Style::default().fg(ACCENT)),
        Span::styled(" panel  ", Style::default().fg(DIM)),
        Span::styled("↑↓", Style::default().fg(ACCENT)),
        Span::styled(" nav  ", Style::default().fg(DIM)),
        Span::styled("Enter", Style::default().fg(ACCENT)),
        Span::styled(" play  ", Style::default().fg(DIM)),
        Span::styled("Space", Style::default().fg(ACCENT)),
        Span::styled(" pause  ", Style::default().fg(DIM)),
        Span::styled("+/-", Style::default().fg(ACCENT)),
        Span::styled(" vol  ", Style::default().fg(DIM)),
        Span::styled("n/p", Style::default().fg(ACCENT)),
        Span::styled(" track  ", Style::default().fg(DIM)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(DIM)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_INACTIVE))
        .style(Style::default().bg(BG));
    let paragraph = Paragraph::new(help).block(block);
    f.render_widget(paragraph, area);
}

fn format_time(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
    } else {
        s.to_string()
    }
}
```

### Event Loop (`main.rs`)

```rust
mod api;
mod app;
mod ui;

use std::time::Duration;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::prelude::*;
use crate::api::ApiClient;
use crate::app::App;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const TICK_RATE: Duration = Duration::from_millis(100);

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let client = ApiClient::new();
    let mut app = App::new();

    // Initial data load
    if let Ok(speakers) = client.get_speakers().await {
        app.speakers = speakers;
    }
    if let Ok(playlists) = client.get_playlists().await {
        app.playlists = playlists;
    }

    loop {
        // Draw
        terminal.draw(|f| ui::draw(f, &app))?;

        // Handle input
        if event::poll(TICK_RATE)? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, &client, key).await?;
            }
        }

        // Periodic refresh
        if app.last_refresh.elapsed() >= POLL_INTERVAL {
            if let Ok(speakers) = client.get_speakers().await {
                app.speakers = speakers;
            }
            app.last_refresh = std::time::Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_key(app: &mut App, client: &ApiClient, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab => app.cycle_panel(),

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => app.prev_in_list(),
        KeyCode::Down | KeyCode::Char('j') => app.next_in_list(),

        // Play selected playlist on selected speaker
        KeyCode::Enter => {
            if let (Some(speaker_id), Some(playlist)) =
                (app.speaker_id(), app.selected_playlist())
            {
                let _ = client.play(&speaker_id, &playlist.alias).await;
                app.status_message = Some(format!(
                    "Playing {} on {}", playlist.alias, speaker_id
                ));
            }
        }

        // Pause / Resume toggle
        KeyCode::Char(' ') => {
            if let Some(sp) = app.selected_speaker() {
                let id = sp.alias.as_deref().unwrap_or(&sp.name);
                match sp.state.as_str() {
                    "PLAYING" => { let _ = client.pause(id).await; }
                    _ => { let _ = client.resume(id).await; }
                }
            }
        }

        // Volume
        KeyCode::Char('+') | KeyCode::Char('=') => {
            if let Some(sp) = app.selected_speaker() {
                let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
                let new_vol = (sp.volume + 5).min(100);
                let _ = client.set_volume(&id, new_vol).await;
            }
        }
        KeyCode::Char('-') => {
            if let Some(sp) = app.selected_speaker() {
                let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
                let new_vol = sp.volume.saturating_sub(5);
                let _ = client.set_volume(&id, new_vol).await;
            }
        }

        // Next / Previous track
        KeyCode::Char('n') => {
            if let Some(id) = app.speaker_id() {
                let _ = client.next(&id).await;
            }
        }
        KeyCode::Char('p') => {
            if let Some(id) = app.speaker_id() {
                let _ = client.previous(&id).await;
            }
        }

        _ => {}
    }
    Ok(())
}
```

### Theme Notes

The color palette above is a cool, dark theme. Some alternatives to consider:

- **Gruvbox dark**: Warm, muted. Amber accents. Feels like a record player.
- **Catppuccin Mocha**: Pastel on dark. Popular in the terminal ricing community.
- **Nord**: Cool blues and teals. Clean, Scandinavian feel.
- **Custom dark fantasy**: Deep purples and blood reds. On-brand for Skulldrinker.

You could make the theme configurable via config.yaml or just pick one and commit to it. The color constants at the top of `ui.rs` make this easy to swap.

---

## Keybindings Summary

| Key         | Action                                    |
|-------------|-------------------------------------------|
| `Tab`       | Cycle active panel                        |
| `↑` / `k`  | Move selection up                         |
| `↓` / `j`  | Move selection down                       |
| `Enter`     | Play selected playlist on selected speaker|
| `Space`     | Pause / resume selected speaker           |
| `+` / `=`   | Volume up 5                               |
| `-`         | Volume down 5                             |
| `n`         | Next track                                |
| `p`         | Previous track                            |
| `q`         | Quit                                      |

---

## Getting Started

### 1. Start the daemon

```bash
cd daemon
python -m venv .venv && source .venv/bin/activate
pip install -e .
# Edit config.yaml with your speakers and Sonos Favorites
sonosd
```

### 2. Build and run the TUI

```bash
cd tui
cargo build --release
./target/release/sonos-tui
```

### 3. Quick test without TUI

```bash
# Verify daemon is working
curl localhost:9271/speakers | jq .
curl -X POST localhost:9271/play -H 'Content-Type: application/json' \
  -d '{"speaker": "office", "playlist": "lotr"}'
```

---

## Gotchas & Tips

- **Same network**: Both daemon and Sonos speakers must be on the same LAN. No VPN, no Docker host networking weirdness.
- **Daemon must be running**: The TUI is just a client. If the daemon isn't running, the TUI will show empty panels. Consider adding a connection status indicator.
- **Favorites first**: Add Spotify playlists to Sonos Favorites via the Sonos app. The daemon can only play Sonos Favorites.
- **Speaker names**: Run `curl localhost:9271/speakers` to get the exact names Sonos reports before filling in config.yaml.
- **Volume safety**: The daemon caps volume at 0–100 but you might want a lower max in practice. Easy to add a `max_volume` config field.
- **Refresh rate**: The TUI polls every 2 seconds. This is fine for status updates. If you want snappier track position updates, lower `POLL_INTERVAL` but watch for rate limiting on the Sonos side.
- **Error handling**: The skeleton silently ignores most API errors. In production you'd want to surface these in the status bar or as toast notifications in the TUI.

---

## Future Ideas

- **Pomodoro mode**: Timer overlay in the TUI. Switches playlists between focus and break periods. Tracks completed sessions.
- **Speaker grouping UI**: A panel or modal for creating/managing speaker groups visually.
- **Search**: Fuzzy search across playlists with `/` keybinding.
- **Album art**: Render album art in the terminal using sixel or kitty image protocol (if your terminal supports it).
- **Systemd service**: Run sonosd as a system service so it starts on boot.
- **WebSocket events**: Replace polling with `soco`'s event subscription system for instant updates.
