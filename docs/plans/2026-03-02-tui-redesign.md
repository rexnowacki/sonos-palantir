# TUI Visual Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the sonos-palantir TUI for a sleek, modern aesthetic — rounded borders, volume bars, segmented progress, top status bar — while preserving all existing functionality.

**Architecture:** Mostly `tui/src/ui.rs` changes (rendering), with small additions to `daemon/sonosd/sonos.py` (source/quality fields), `tui/src/api.rs` (new struct fields), and name fixes across splash/help/history. No changes to event loop, command parsing, or app state logic.

**Tech Stack:** Rust (ratatui 0.29, crossterm 0.28), Python (soco, FastAPI)

---

### Task 1: Fix Project Name Everywhere

The project name is "sonos-palantir" but several places say "sono-palantir". Fix all occurrences.

**Files:**
- Modify: `tui/src/ui.rs:44` (splash screen title)
- Modify: `tui/src/ui.rs:497` (help overlay title)
- Modify: `tui/src/history.rs:23` (config directory path)

**Step 1: Fix splash screen title**

In `tui/src/ui.rs`, line 44, change:
```rust
// OLD
"S O N O - P A L A N T I R",
// NEW
"S O N O S - P A L A N T I R",
```

**Step 2: Fix help overlay title**

In `tui/src/ui.rs`, line 497, change:
```rust
// OLD
.title(" ? The Lore of sono-palantir — Esc or ? to close ")
// NEW
.title(" ? The Lore of sonos-palantir — Esc or ? to close ")
```

**Step 3: Fix history config directory**

In `tui/src/history.rs`, line 23, change:
```rust
// OLD
let dir = PathBuf::from(home).join(".config/sono-palantir");
// NEW
let dir = PathBuf::from(home).join(".config/sonos-palantir");
```

**Step 4: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add tui/src/ui.rs tui/src/history.rs
git commit -m "fix: even the smallest person can change the course of the future — corrected project name to sonos-palantir"
git push origin main
```

---

### Task 2: Add Rounded Borders

Replace all `Block::default()` borders with `BorderType::Rounded` for the `╭╮╰╯` aesthetic.

**Files:**
- Modify: `tui/src/ui.rs`

**Step 1: Add BorderType import**

In `tui/src/ui.rs`, line 5, add `BorderType` to the widgets import:
```rust
// OLD
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
// NEW
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, ListState, Paragraph},
```

**Step 2: Update `panel_block` to use rounded borders**

In `tui/src/ui.rs`, the `panel_block` function (~line 96-103), change:
```rust
// OLD
fn panel_block(title: &str, active: bool) -> Block<'_> {
    let border_color = if active { BORDER_ACTIVE } else { BORDER_INACTIVE };
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG))
}
// NEW
fn panel_block(title: &str, active: bool) -> Block<'_> {
    let border_color = if active { BORDER_ACTIVE } else { BORDER_INACTIVE };
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG))
}
```

**Step 3: Update splash screen border**

In `draw_splash`, the Block (~line 23-26):
```rust
// OLD
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
// NEW
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
```

**Step 4: Update help bar blocks**

In `draw_help_bar`, there are 3 Block instances (command mode, volume mode, default). Add `.border_type(BorderType::Rounded)` to each. For example the command mode block (~line 436):
```rust
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
```
Do the same for the volume input block (~line 453) and the default help bar block (~line 486).

**Step 5: Update help overlay border**

In `draw_help_overlay` (~line 496):
```rust
    let block = Block::default()
        .title(" ? The Lore of sonos-palantir — Esc or ? to close ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
```

**Step 6: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add tui/src/ui.rs
git commit -m "style: the elven-smiths wrought borders of great beauty — rounded borders across all panels"
git push origin main
```

---

### Task 3: Daemon — Add Source and Quality Fields

Extract media source and quality from soco track info and include them in the speaker info response.

**Files:**
- Modify: `daemon/sonosd/sonos.py` — `get_speaker_info()` method and a new `_detect_source()` helper

**Step 1: Add the `_detect_source` helper function**

At the bottom of `daemon/sonosd/sonos.py`, before the `_parse_duration` function (line ~199), add:

```python
def _detect_source(uri: str) -> str:
    """Best-effort source detection from track URI."""
    if not uri:
        return ""
    uri_lower = uri.lower()
    if "spotify" in uri_lower or "x-sonos-spotify" in uri_lower:
        return "Spotify"
    if "apple" in uri_lower or "raop" in uri_lower:
        return "Apple Music"
    if "amazon" in uri_lower:
        return "Amazon Music"
    if "tidal" in uri_lower:
        return "Tidal"
    if "soundcloud" in uri_lower:
        return "SoundCloud"
    if "plex" in uri_lower:
        return "Plex"
    if "x-file-cifs" in uri_lower or "x-smb" in uri_lower:
        return "Local Library"
    if "x-rincon-stream" in uri_lower:
        return "Line-In"
    if "aac" in uri_lower or "flac" in uri_lower or "mp3" in uri_lower:
        return "Local Library"
    return ""
```

**Step 2: Add source and quality to track dict in `get_speaker_info()`**

In `get_speaker_info()`, where the `track` dict is built (~lines 83-90), add two new keys. Change:
```python
        # OLD
            track = {
                "title": track_info.get("title", ""),
                "artist": track_info.get("artist", ""),
                "album": track_info.get("album", ""),
                "duration": _parse_duration(track_info.get("duration", "0:00:00")),
                "position": _parse_duration(track_info.get("position", "0:00:00")),
                "art_uri": track_info.get("album_art", ""),
            }
        # NEW
            uri = track_info.get("uri", "")
            track = {
                "title": track_info.get("title", ""),
                "artist": track_info.get("artist", ""),
                "album": track_info.get("album", ""),
                "duration": _parse_duration(track_info.get("duration", "0:00:00")),
                "position": _parse_duration(track_info.get("position", "0:00:00")),
                "art_uri": track_info.get("album_art", ""),
                "source": _detect_source(uri),
                "quality": "",
            }
```

Do the same for the coordinator fallback track dict (~lines 99-106):
```python
            # OLD
                track = {
                    "title": coord_track.get("title", ""),
                    "artist": coord_track.get("artist", ""),
                    "album": coord_track.get("album", ""),
                    "duration": _parse_duration(coord_track.get("duration", "0:00:00")),
                    "position": _parse_duration(coord_track.get("position", "0:00:00")),
                    "art_uri": coord_track.get("album_art", ""),
                }
            # NEW
                coord_uri = coord_track.get("uri", "")
                track = {
                    "title": coord_track.get("title", ""),
                    "artist": coord_track.get("artist", ""),
                    "album": coord_track.get("album", ""),
                    "duration": _parse_duration(coord_track.get("duration", "0:00:00")),
                    "position": _parse_duration(coord_track.get("position", "0:00:00")),
                    "art_uri": coord_track.get("album_art", ""),
                    "source": _detect_source(coord_uri),
                    "quality": "",
                }
```

**Step 3: Write a test for `_detect_source`**

Create a test in `daemon/tests/test_detect_source.py`:
```python
from sonosd.sonos import _detect_source


def test_spotify():
    assert _detect_source("x-sonos-spotify:spotify:track:abc") == "Spotify"


def test_apple():
    assert _detect_source("x-sonos-http:apple.com/stream") == "Apple Music"


def test_empty():
    assert _detect_source("") == ""


def test_none():
    assert _detect_source("") == ""


def test_local_library():
    assert _detect_source("x-file-cifs://nas/music/song.flac") == "Local Library"


def test_line_in():
    assert _detect_source("x-rincon-stream:RINCON_123") == "Line-In"


def test_unknown():
    assert _detect_source("http://example.com/stream") == ""
```

**Step 4: Run tests**

Run: `cd daemon && source .venv/bin/activate && pytest -q`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add daemon/sonosd/sonos.py daemon/tests/test_detect_source.py
git commit -m "feat: the palantir sees deeper now — source detection from track URI"
git push origin main
```

---

### Task 4: TUI — Add Source/Quality to Track Struct

Update the Rust `Track` struct to deserialize the new `source` and `quality` fields from the daemon.

**Files:**
- Modify: `tui/src/api.rs:17-24` — Track struct

**Step 1: Add optional fields to Track**

In `tui/src/api.rs`, change the Track struct:
```rust
// OLD
#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub position: u64,
}
// NEW
#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub position: u64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub quality: String,
}
```

Using `#[serde(default)]` ensures backward compatibility — if the daemon doesn't send these fields, they default to empty strings.

**Step 2: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass. The `make_speaker` helper in `app.rs` tests doesn't construct Track objects directly, so no test changes needed.

**Step 3: Commit**

```bash
git add tui/src/api.rs
git commit -m "feat: the seeing-stone receives new visions — source and quality fields in Track"
git push origin main
```

---

### Task 5: Redesign Outer Layout — Add Top Status Bar

Change the outer layout from 3-row (main, status, help) to 4-row (top bar, main, status, help). Implement the top status bar.

**Files:**
- Modify: `tui/src/ui.rs` — `draw()` function and new `draw_top_bar()` function

**Step 1: Update the `draw()` function layout**

In `tui/src/ui.rs`, change the `draw()` function:
```rust
// OLD
pub fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(outer[0]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main[0]);

    draw_speakers(f, app, left[0]);
    draw_playlists(f, app, left[1]);
    draw_now_playing(f, app, main[1]);
    draw_status_line(f, app, outer[1]);
    draw_help_bar(f, app, outer[2]);

    if app.help_open {
        draw_help_overlay(f);
    }
}
// NEW
pub fn draw(f: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // top status bar
            Constraint::Min(1),     // main panels
            Constraint::Length(1),   // status line
            Constraint::Length(3),   // help bar / command input
        ])
        .split(f.area());

    draw_top_bar(f, app, outer[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(outer[1]);

    // Dynamic left column: Rooms takes what it needs, Playlists gets the rest
    let speaker_rows = if app.is_grouped() {
        // grouped: coordinator headers + 2 lines per speaker + blank lines
        let mut rows: u16 = 0;
        for coord in app.coordinators() {
            let members = app.group_members_of(&coord.name);
            rows += 1 + (members.len() as u16 * 2) + 1; // header + members*2 + blank
        }
        for _solo in app.solo_speakers() {
            rows += 2;
        }
        rows
    } else {
        app.speakers.len() as u16 * 2 // name line + volume bar per speaker
    };
    // +2 for border top/bottom of Rooms panel
    let rooms_height = speaker_rows + 2;

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(rooms_height), Constraint::Min(0)])
        .split(main[0]);

    draw_speakers(f, app, left[0]);
    draw_playlists(f, app, left[1]);
    draw_now_playing(f, app, main[1]);
    draw_status_line(f, app, outer[2]);
    draw_help_bar(f, app, outer[3]);

    if app.help_open {
        draw_help_overlay(f);
    }
}
```

**Step 2: Implement `draw_top_bar()`**

Add this function in `tui/src/ui.rs`, after the `draw()` function:
```rust
fn draw_top_bar(f: &mut Frame, app: &App, area: Rect) {
    let selected = app.selected_speaker();

    // Playing indicator dot
    let (dot, dot_color) = match selected.map(|s| s.state.as_str()) {
        Some("PLAYING") => ("●", PLAYING),
        Some("PAUSED_PLAYBACK") => ("●", PAUSED),
        _ => ("●", DIM),
    };

    // Speaker name
    let speaker_name = selected
        .map(|s| s.alias.as_deref().unwrap_or(&s.name).to_string())
        .unwrap_or_else(|| "—".to_string());

    // Track info
    let track_info = selected
        .and_then(|s| s.track.as_ref())
        .map(|t| format!("{} — {}", t.title, t.artist))
        .unwrap_or_default();

    // Volume
    let vol = selected.map(|s| format!("VOL {}%", s.volume)).unwrap_or_default();

    // Daemon status — if speakers is empty and no status, daemon might be down
    let daemon_status = if app.speakers.is_empty() {
        Span::styled("palantir:ERR", Style::default().fg(Color::Rgb(220, 80, 80)))
    } else {
        Span::styled("palantir:OK", Style::default().fg(PLAYING))
    };

    // Speaker count
    let count = format!("Sonos:{}", app.speakers.len());

    // Build right-aligned section
    let right_text = format!("  {}  {}  {} ", vol, "", count);
    let right_len = vol.len() + 2 + 12 + 2 + count.len() + 1; // approximate

    // Truncate track info to fit
    let available = area.width as usize - speaker_name.len() - right_len - 8;
    let track_display = if track_info.len() > available {
        truncate(&track_info, available)
    } else {
        track_info
    };

    let spans = vec![
        Span::styled(format!(" {} ", dot), Style::default().fg(dot_color).bg(Color::Rgb(30, 30, 45))),
        Span::styled(format!("{} ", speaker_name), Style::default().fg(ACCENT).bg(Color::Rgb(30, 30, 45)).add_modifier(Modifier::BOLD)),
        Span::styled(track_display, Style::default().fg(FG).bg(Color::Rgb(30, 30, 45))),
        Span::styled("  ", Style::default().bg(Color::Rgb(30, 30, 45))),
        Span::styled(format!("{} ", vol), Style::default().fg(DIM).bg(Color::Rgb(30, 30, 45))),
        Span::styled(" ", Style::default().bg(Color::Rgb(30, 30, 45))),
        daemon_status.style(Style::default().bg(Color::Rgb(30, 30, 45))),
        Span::styled(format!("  {} ", count), Style::default().fg(DIM).bg(Color::Rgb(30, 30, 45))),
    ];

    let bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Rgb(30, 30, 45)));
    f.render_widget(bar, area);
}
```

**Step 3: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass (no UI tests exist — this is visual rendering only).

**Step 4: Build release and visually verify**

Run: `cd tui && cargo build --release`
Expected: Compiles without errors. Run `./target/release/sonos-tui` and verify the top bar appears with speaker info.

**Step 5: Commit**

```bash
git add tui/src/ui.rs
git commit -m "feat: the White Council convenes above — top status bar with speaker, track, and daemon status"
git push origin main
```

---

### Task 6: Redesign Rooms Panel — Volume Bars and Group Subsections

Replace the current speaker list and box topology with the new design: per-speaker volume bars and GROUPED subsection headers.

**Files:**
- Modify: `tui/src/ui.rs` — `draw_speakers()`, `draw_speaker_list()`, replace `draw_topology()`

**Step 1: Add volume bar color helper**

Add this helper function in `tui/src/ui.rs`:
```rust
/// Returns a color for the volume bar: green (0-50), yellow (51-80), red (81-100).
fn volume_color(vol: u8) -> Color {
    if vol <= 50 {
        Color::Rgb(120, 220, 140) // green
    } else if vol <= 80 {
        Color::Rgb(240, 200, 80) // yellow
    } else {
        Color::Rgb(220, 80, 80) // red
    }
}

/// Render a volume bar string using block characters: `███▓▒░`.
fn volume_bar(vol: u8, width: usize) -> (String, Color) {
    let color = volume_color(vol);
    let filled = (vol as usize * width) / 100;
    let remainder = (vol as usize * width) % 100;
    let partial = if filled < width && remainder > 0 {
        if remainder > 66 { "▓" } else if remainder > 33 { "▒" } else { "░" }
    } else {
        ""
    };
    let empty = width - filled - if partial.is_empty() { 0 } else { 1 };
    let bar = format!("{}{}{}", "█".repeat(filled), partial, " ".repeat(empty));
    (bar, color)
}
```

**Step 2: Rewrite `draw_speakers()` to always use the new unified renderer**

Replace `draw_speakers()`, `draw_speaker_list()`, and `draw_topology()` with a single unified function:

```rust
fn draw_speakers(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
    let block = panel_block("Rooms", active);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = vec![];
    let bar_width = (inner.width as usize).saturating_sub(6); // 3 left pad + 3 right pad

    if app.is_grouped() {
        // Grouped speakers: subsection headers
        for coord in app.coordinators() {
            let members = app.group_members_of(&coord.name);
            let member_names: Vec<&str> = members.iter()
                .map(|m| m.alias.as_deref().unwrap_or(&m.name))
                .collect();
            lines.push(Line::from(vec![
                Span::styled(" GROUPED ", Style::default().fg(DIM)),
                Span::styled(member_names.join(" + "), Style::default().fg(ACCENT)),
            ]));
            for (i, m) in members.iter().enumerate() {
                let sp_index = app.speakers.iter().position(|s| s.name == m.name);
                let is_selected = active && sp_index == Some(app.speaker_index);
                render_speaker_row(&mut lines, m, is_selected, bar_width);
            }
        }
        // Solo speakers below groups
        for sp in app.solo_speakers() {
            let sp_index = app.speakers.iter().position(|s| s.name == sp.name);
            let is_selected = active && sp_index == Some(app.speaker_index);
            render_speaker_row(&mut lines, sp, is_selected, bar_width);
        }
    } else {
        // Ungrouped: simple list
        for (i, sp) in app.speakers.iter().enumerate() {
            let is_selected = active && i == app.speaker_index;
            render_speaker_row(&mut lines, sp, is_selected, bar_width);
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

fn render_speaker_row(lines: &mut Vec<Line>, sp: &crate::api::Speaker, selected: bool, bar_width: usize) {
    let name = sp.alias.as_deref().unwrap_or(&sp.name);
    let marker = if selected { "▸" } else { " " };
    let (state_icon, state_color) = match sp.state.as_str() {
        "PLAYING" => ("▶", PLAYING),
        "PAUSED_PLAYBACK" => ("‖", PAUSED),
        _ => ("·", DIM),
    };
    let name_style = if selected {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG)
    };

    let name_line = Line::from(vec![
        Span::styled(format!(" {} ", marker), if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) }),
        Span::styled(format!("{:<12}", name), name_style),
        Span::styled(format!(" {} ", state_icon), Style::default().fg(state_color)),
        Span::styled(format!("{:>3}", sp.volume), Style::default().fg(DIM)),
    ]);
    lines.push(name_line);

    // Volume bar below speaker name
    let (bar, color) = volume_bar(sp.volume, bar_width);
    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled(bar, Style::default().fg(color)),
    ]));
}
```

**Step 3: Remove the old `draw_speaker_list()` and `draw_topology()` functions**

Delete these functions entirely:
- `draw_speaker_list()` (~lines 116-151)
- `draw_topology()` (~lines 153-218)

They are replaced by the unified `draw_speakers()` and `render_speaker_row()` above.

**Step 4: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass.

**Step 5: Build release and visually verify**

Run: `cd tui && cargo build --release`
Expected: Compiles without errors. Run `./target/release/sonos-tui` and verify:
- Each speaker has a name row + colored volume bar below it
- Volume bars are green (low), yellow (mid), red (high)
- When grouped, a "GROUPED cth + family" header appears above members
- Rooms panel takes only the space it needs; Playlists gets the rest

**Step 6: Commit**

```bash
git add tui/src/ui.rs
git commit -m "feat: each realm now shows its power — Rooms panel with volume bars and group subsections"
git push origin main
```

---

### Task 7: Redesign Now Playing — Segmented Progress Bar and Source Line

Replace the Gauge-based progress bar with a custom segmented text bar, and add source/quality metadata.

**Files:**
- Modify: `tui/src/ui.rs` — `draw_track_block()` function

**Step 1: Add segmented progress bar helper**

Add this helper in `tui/src/ui.rs`:
```rust
/// Render a segmented progress bar: `═══════●─────────` with time labels.
fn segmented_progress(position: u64, duration: u64, width: usize) -> Line<'static> {
    if duration == 0 || width < 4 {
        return Line::from("");
    }
    let ratio = (position as f64 / duration as f64).min(1.0);
    let filled = (ratio * width as f64) as usize;
    let filled = filled.min(width.saturating_sub(1)); // leave room for cursor

    let before = "═".repeat(filled);
    let after = "─".repeat(width.saturating_sub(filled + 1));

    Line::from(vec![
        Span::styled(before, Style::default().fg(ACCENT)),
        Span::styled("●", Style::default().fg(Color::Rgb(255, 255, 255))),
        Span::styled(after, Style::default().fg(DIM)),
    ])
}
```

**Step 2: Rewrite `draw_track_block()`**

Replace the entire `draw_track_block()` function:
```rust
fn draw_track_block(f: &mut Frame, sp: &crate::api::Speaker, area: Rect, show_vol: bool) {
    if area.height == 0 {
        return;
    }
    // Speaker label
    let label_area = Rect { y: area.y, height: 1, ..area };
    let label = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("  {} ", sp.alias.as_deref().unwrap_or(&sp.name)),
            Style::default().fg(DIM),
        ),
    ]));
    f.render_widget(label, label_area);

    let content_area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };

    if let Some(track) = &sp.track {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // title
                Constraint::Length(1), // artist — album
                Constraint::Length(1), // spacer
                Constraint::Length(1), // source / quality
                Constraint::Length(1), // spacer
                Constraint::Length(1), // progress bar
                Constraint::Length(1), // time
                Constraint::Min(0),
            ])
            .split(content_area);

        // Track title
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ♫ ", Style::default().fg(PLAYING)),
                Span::styled(&track.title, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            ])),
            chunks[0],
        );
        // Artist — Album
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.artist, Style::default().fg(ACCENT)),
                Span::styled(" — ", Style::default().fg(DIM)),
                Span::styled(&track.album, Style::default().fg(DIM)),
            ])),
            chunks[1],
        );

        // Source / Quality line
        let source_line = if !track.source.is_empty() {
            let mut spans = vec![
                Span::raw("    "),
                Span::styled(format!("Source: {}", track.source), Style::default().fg(DIM)),
            ];
            if !track.quality.is_empty() {
                spans.push(Span::styled(format!(" · Quality: {}", track.quality), Style::default().fg(DIM)));
            }
            Line::from(spans)
        } else {
            Line::from("")
        };
        f.render_widget(Paragraph::new(source_line), chunks[3]);

        // Segmented progress bar
        let bar_width = chunks[5].width.saturating_sub(8) as usize;
        let progress = segmented_progress(track.position, track.duration, bar_width);
        let bar_area = Rect {
            x: chunks[5].x + 4,
            width: chunks[5].width.saturating_sub(8),
            ..chunks[5]
        };
        f.render_widget(Paragraph::new(progress), bar_area);

        // Time display
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("    {} / {}", format_time(track.position), format_time(track.duration)),
                Style::default().fg(DIM),
            )),
            chunks[6],
        );
    } else {
        f.render_widget(
            Paragraph::new(Span::styled("  Nothing playing", Style::default().fg(DIM))),
            content_area,
        );
    }
}
```

**Step 3: Remove the Gauge import if no longer used**

Check: after this change, if `Gauge` is no longer used anywhere in ui.rs, remove it from the import:
```rust
// If Gauge is unused, change:
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, ListState, Paragraph},
// To:
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
```

**Step 4: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass.

**Step 5: Build release and visually verify**

Run: `cd tui && cargo build --release`
Expected: Compiles without errors. Run `./target/release/sonos-tui` and verify:
- Progress bar shows `═══════●─────────` style instead of the old filled gauge
- Source line shows "Source: Spotify" (or appropriate source) when playing
- Artist and album appear on one line: "Artist — Album"

**Step 6: Commit**

```bash
git add tui/src/ui.rs
git commit -m "feat: the road goes ever on — segmented progress bar and source metadata in Now Playing"
git push origin main
```

---

### Task 8: Playlist Panel Polish

Simplify the playlist display now that the left column is narrower (30%).

**Files:**
- Modify: `tui/src/ui.rs` — `draw_playlists()` function

**Step 1: Update `draw_playlists()` for narrow column**

Since the left column is now 30% width, playlists need a more compact display. Change the item rendering to show just the alias (or favorite_name if no alias differs):

```rust
fn draw_playlists(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Playlists;
    let block = panel_block("Playlists", active);
    let inner_width = area.width.saturating_sub(2) as usize; // minus borders

    let items: Vec<ListItem> = app.playlists.iter().enumerate().map(|(i, pl)| {
        let selected = i == app.playlist_index;
        let style = if selected && active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(FG)
        };

        let marker = if selected { "▸" } else { " " };
        let display = truncate(&pl.alias, inner_width.saturating_sub(4));

        let line = Line::from(vec![
            Span::styled(format!(" {} ", marker), if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) }),
            Span::styled(display, style),
        ]);

        let mut item = ListItem::new(line);
        if selected && active {
            item = item.style(Style::default().bg(HIGHLIGHT_BG));
        }
        item
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default());

    let mut state = ListState::default();
    if !app.playlists.is_empty() {
        state.select(Some(app.playlist_index));
    }
    f.render_stateful_widget(list, area, &mut state);
}
```

**Step 2: Run tests**

Run: `cd tui && cargo test`
Expected: All tests pass.

**Step 3: Build release and visually verify**

Run: `cd tui && cargo build --release`
Expected: Playlists panel renders cleanly in the narrower column with the `▸` selector marker matching the Rooms style.

**Step 4: Commit**

```bash
git add tui/src/ui.rs
git commit -m "style: the scrolls are neatly arranged — playlist panel polished for narrow column"
git push origin main
```

---

### Task 9: Final Build, Full Verification, and Cleanup

Run all tests across both daemon and TUI, rebuild release, and do a final visual check.

**Files:**
- No new modifications (verification only)

**Step 1: Run daemon tests**

Run: `cd daemon && source .venv/bin/activate && pytest -q`
Expected: All tests pass.

**Step 2: Run TUI tests**

Run: `cd tui && cargo test`
Expected: All tests pass.

**Step 3: Build release**

Run: `cd tui && cargo build --release`
Expected: Compiles with no errors and no warnings.

**Step 4: Verify visually**

Start daemon: `cd daemon && source .venv/bin/activate && sonosd`
Start TUI: `cd tui && ./target/release/sonos-tui`

Verify checklist:
- [ ] Top status bar shows: dot, speaker name, track, VOL, palantir:OK, Sonos:N
- [ ] Rooms panel has volume bars below each speaker name
- [ ] Volume bars are green/yellow/red based on level
- [ ] When grouped, "GROUPED cth + family" header appears
- [ ] Playlists panel scrolls correctly and gets all remaining vertical space
- [ ] Now Playing shows segmented `═══●───` progress bar
- [ ] Source line shows "Source: Spotify" or similar
- [ ] All rounded borders `╭╮╰╯` everywhere
- [ ] Splash screen says "S O N O S - P A L A N T I R"
- [ ] Help overlay says "sonos-palantir"
- [ ] All keybindings still work: Tab, j/k, Enter, Space, +/-, v, n/p, g, :, ?
- [ ] :vol, :play, :sleep, :reload commands all work
- [ ] q quits

**Step 5: Final commit if any cleanup needed**

If any small fixes were needed during verification:
```bash
git add -A
git commit -m "fix: the last alliance stands firm — final polish for TUI redesign"
git push origin main
```
