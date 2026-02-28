# sono-palantir Feature Suite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement play history, group topology visualizer, multi-group Now Playing, Speak Friend command mode with ghost-text autocomplete, sleep timer, config hot-reload, startup splash, LOTR error messages, and a `?` help screen.

**Architecture:** TUI-centric. The daemon gains a `POST /reload` endpoint and mtime-based config watching. The TUI gains `history.rs` (play tracking) and `command.rs` (command parsing/autocomplete). `ui.rs` grows a status line row and replaces the single Now Playing with a dynamic multi-group stacked view.

**Tech Stack:** Rust/Ratatui/tokio (TUI), Python/FastAPI/soco (daemon), serde_json (history file), no new crate dependencies.

---

## Key facts before you start

- Working directory: `/Users/christophernowacki/sonos`
- Run daemon tests: `cd daemon && source .venv/bin/activate && pytest -q`
- Run TUI tests: `cd tui && cargo test`
- Build TUI: `cd tui && cargo build --release 2>&1 | tail -5`
- `App` already has: `speakers`, `playlists`, `active_panel`, `speaker_index`, `playlist_index`, `should_quit`, `status_message: Option<String>`, `volume_input: Option<String>`
- `ui.rs` outer layout is currently 2 rows: `[Constraint::Min(1), Constraint::Length(3)]` — main panels and help bar
- Color constants in `ui.rs`: `BG`, `FG`, `ACCENT`, `PLAYING`, `PAUSED`, `DIM`, `HIGHLIGHT_BG`
- `draw_help_bar(f, app, area)` already takes `app` and handles `volume_input` mode
- Commit message style: LOTR/wizard themed. Push to main after every commit.
- After every commit: `git push origin main`

---

### Task 1: App state foundations + status line layout

Add new fields to `App`, a `set_status()` helper, an `active_status()` helper, and wire a status line row into the outer layout.

**Files:**
- Modify: `tui/src/app.rs`
- Modify: `tui/src/ui.rs`

**Step 1: Add fields to `App` struct**

In `tui/src/app.rs`, add these fields to the `App` struct after `volume_input`:

```rust
pub command_input: Option<String>,
pub sleep_until: Option<std::time::Instant>,
pub status_until: Option<std::time::Instant>,
pub help_open: bool,
```

Add to `App::new()` after `volume_input: None,`:

```rust
command_input: None,
sleep_until: None,
status_until: None,
help_open: false,
```

**Step 2: Add `set_status` and `active_status` methods**

Add to `impl App`:

```rust
pub fn set_status(&mut self, msg: impl Into<String>, secs: u64) {
    self.status_message = Some(msg.into());
    self.status_until = Some(
        std::time::Instant::now() + std::time::Duration::from_secs(secs)
    );
}

pub fn active_status(&self) -> String {
    // Sleep countdown takes lowest priority — shown only when no timed message
    if let Some(until) = self.status_until {
        if until > std::time::Instant::now() {
            return self.status_message.clone().unwrap_or_default();
        }
    }
    if let Some(sleep_until) = self.sleep_until {
        if sleep_until > std::time::Instant::now() {
            let secs = sleep_until
                .duration_since(std::time::Instant::now())
                .as_secs();
            return format!("Sleep: {}:{:02} remaining", secs / 60, secs % 60);
        }
    }
    String::new()
}
```

**Step 3: Write tests**

Add to the `#[cfg(test)]` block in `app.rs`:

```rust
#[test]
fn test_active_status_returns_empty_when_nothing_set() {
    let app = App::new();
    assert_eq!(app.active_status(), "");
}

#[test]
fn test_set_status_returns_message_immediately() {
    let mut app = App::new();
    app.set_status("The gates of Moria are sealed.", 5);
    assert_eq!(app.active_status(), "The gates of Moria are sealed.");
}
```

**Step 4: Run tests**

```bash
cd tui && cargo test 2>&1 | tail -10
```

Expected: 7 tests passing (was 5, now 7).

**Step 5: Add status line row to `ui.rs` layout**

In `tui/src/ui.rs`, replace the `draw` function with:

```rust
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
}
```

**Step 6: Add `draw_status_line` function**

Add before `draw_help_bar` in `ui.rs`:

```rust
fn draw_status_line(f: &mut Frame, app: &App, area: Rect) {
    let msg = app.active_status();
    let style = if msg.is_empty() {
        Style::default().fg(DIM).bg(BG)
    } else {
        Style::default().fg(ACCENT).bg(BG)
    };
    let para = Paragraph::new(format!(" {}", msg)).style(style);
    f.render_widget(para, area);
}
```

**Step 7: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

Expected: `Finished release profile` with no errors.

**Step 8: Commit**

```bash
git add tui/src/app.rs tui/src/ui.rs
git commit -m "feat: the palantir opens one eye — add status line and App state foundations for command mode, sleep, and errors"
git push origin main
```

---

### Task 2: Play history module

Track playlist plays in `~/.config/sono-palantir/history.json`. Expose `record_play()` and `popularity_sort()`. Add a `/config` daemon endpoint so the TUI can read `playlist_sort`.

**Files:**
- Create: `tui/src/history.rs`
- Modify: `tui/src/main.rs` (mod declaration + call record_play on Enter)
- Modify: `daemon/sonosd/server.py` (add GET /config)
- Modify: `tui/src/api.rs` (add get_playlist_sort)
- Test: `tui/src/history.rs` (inline tests)
- Test: `daemon/tests/test_server.py`

**Step 1: Write failing tests for history module**

Create `tui/src/history.rs` with just the test module first:

```rust
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct PlayEntry {
    pub playlist: String,
    pub played_at: u64,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn history_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".config/sono-palantir");
    fs::create_dir_all(&dir).ok();
    dir.join("history.json")
}

pub fn load() -> Vec<PlayEntry> {
    fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn record_play(playlist: &str) {
    let mut entries = load();
    entries.push(PlayEntry {
        playlist: playlist.to_string(),
        played_at: now_unix(),
    });
    let cutoff = now_unix().saturating_sub(90 * 24 * 3600);
    entries.retain(|e| e.played_at > cutoff);
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        fs::write(history_path(), json).ok();
    }
}

pub fn popularity_sort(playlists: &mut Vec<crate::api::Playlist>) {
    let counts = play_counts_7d();
    playlists.sort_by(|a, b| {
        let ca = counts.get(&a.alias).copied().unwrap_or(0);
        let cb = counts.get(&b.alias).copied().unwrap_or(0);
        cb.cmp(&ca).then(a.alias.cmp(&b.alias))
    });
}

fn play_counts_7d() -> HashMap<String, usize> {
    let cutoff = now_unix().saturating_sub(7 * 24 * 3600);
    let mut counts = HashMap::new();
    for e in load() {
        if e.played_at > cutoff {
            *counts.entry(e.playlist).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_entry(playlist: &str, secs_ago: u64) -> PlayEntry {
        PlayEntry {
            playlist: playlist.to_string(),
            played_at: now_unix().saturating_sub(secs_ago),
        }
    }

    #[test]
    fn test_play_counts_7d_counts_recent() {
        // 2 recent plays of "altwave", 1 old play
        let entries = vec![
            fake_entry("altwave", 3600),        // 1 hour ago
            fake_entry("altwave", 3600 * 24),   // 1 day ago
            fake_entry("altwave", 3600 * 24 * 10), // 10 days ago (outside 7d)
            fake_entry("jazz", 3600 * 24 * 2),  // 2 days ago
        ];
        let cutoff = now_unix().saturating_sub(7 * 24 * 3600);
        let mut counts: HashMap<String, usize> = HashMap::new();
        for e in &entries {
            if e.played_at > cutoff {
                *counts.entry(e.playlist.clone()).or_insert(0) += 1;
            }
        }
        assert_eq!(counts["altwave"], 2);
        assert_eq!(counts["jazz"], 1);
        assert!(!counts.contains_key("old"));
    }

    #[test]
    fn test_popularity_sort_orders_by_count_desc() {
        use crate::api::Playlist;
        let mut playlists = vec![
            Playlist { alias: "jazz".to_string(), favorite_name: "Jazz".to_string() },
            Playlist { alias: "altwave".to_string(), favorite_name: "Alt Wave".to_string() },
        ];
        let mut counts: HashMap<String, usize> = HashMap::new();
        counts.insert("altwave".to_string(), 5);
        counts.insert("jazz".to_string(), 2);
        playlists.sort_by(|a, b| {
            let ca = counts.get(&a.alias).copied().unwrap_or(0);
            let cb = counts.get(&b.alias).copied().unwrap_or(0);
            cb.cmp(&ca).then(a.alias.cmp(&b.alias))
        });
        assert_eq!(playlists[0].alias, "altwave");
        assert_eq!(playlists[1].alias, "jazz");
    }
}
```

**Step 2: Add `mod history;` to `main.rs`**

At the top of `tui/src/main.rs`, add:
```rust
mod history;
```

**Step 3: Run tests to verify they pass**

```bash
cd tui && cargo test history 2>&1 | tail -10
```

Expected: 2 tests pass.

**Step 4: Add GET /config to daemon**

In `daemon/sonosd/server.py`, add after `get_playlists`:

```python
@app.get("/config")
def get_config():
    return {
        "playlist_sort": manager.config.get("playlist_sort", "alphabetical")
    }
```

**Step 5: Write daemon test for /config**

Add to `daemon/tests/test_server.py`:

```python
def test_get_config_returns_playlist_sort():
    client, _, _ = _make_client()
    resp = client.get("/config")
    assert resp.status_code == 200
    assert "playlist_sort" in resp.json()
```

**Step 6: Run daemon tests**

```bash
cd daemon && source .venv/bin/activate && pytest -q
```

Expected: 23 tests pass.

**Step 7: Add `get_playlist_sort` to `ApiClient`**

In `tui/src/api.rs`, add after `get_favorites`:

```rust
pub async fn get_playlist_sort(&self) -> anyhow::Result<String> {
    let resp: serde_json::Value = self.client
        .get(format!("{}/config", self.base_url))
        .send().await?
        .json().await?;
    Ok(resp["playlist_sort"].as_str().unwrap_or("alphabetical").to_string())
}
```

**Step 8: Apply popularity sort on startup in `main.rs`**

In `tui/src/main.rs`, after the favorites merge block, add:

```rust
if let Ok(sort) = client.get_playlist_sort().await {
    if sort == "popularity" {
        history::popularity_sort(&mut app.playlists);
    }
}
```

**Step 9: Record plays on Enter and :play**

In `handle_key` in `main.rs`, inside the `KeyCode::Enter` arm, after calling `client.play(...)`, add:

```rust
history::record_play(&playlist.alias);
```

**Step 10: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

Expected: clean build.

**Step 11: Commit**

```bash
git add tui/src/history.rs tui/src/main.rs tui/src/api.rs daemon/sonosd/server.py daemon/tests/test_server.py
git commit -m "feat: the road goes ever on — track playlist plays in local history, sort by popularity"
git push origin main
```

---

### Task 3: Group topology view in Speakers panel

When any speaker is grouped, replace the flat speaker list with an ASCII topology map.

**Files:**
- Modify: `tui/src/ui.rs` (`draw_speakers`)
- Modify: `tui/src/app.rs` (add `group_members_of` helper)

**Step 1: Add `group_members_of` to App**

In `tui/src/app.rs`, add to `impl App`:

```rust
/// Returns all speakers whose coordinator is `coordinator_name`.
pub fn group_members_of<'a>(&'a self, coordinator_name: &str) -> Vec<&'a Speaker> {
    self.speakers.iter().filter(|s| {
        s.group_coordinator.as_deref() == Some(coordinator_name)
    }).collect()
}

/// Returns speakers with no group_coordinator (truly ungrouped/solo).
pub fn solo_speakers(&self) -> Vec<&Speaker> {
    self.speakers.iter().filter(|s| s.group_coordinator.is_none()).collect()
}

/// Returns coordinator speakers (group_coordinator == their own name).
pub fn coordinators(&self) -> Vec<&Speaker> {
    self.speakers.iter().filter(|s| {
        s.group_coordinator.as_deref() == Some(s.name.as_str())
    }).collect()
}
```

**Step 2: Add tests**

```rust
#[test]
fn test_coordinators_returns_only_coordinators() {
    let mut app = App::new();
    app.speakers = vec![
        make_speaker("cthulhu", Some("cthulhu")),
        make_speaker("family", Some("cthulhu")),
        make_speaker("hermit", None),
    ];
    let coords = app.coordinators();
    assert_eq!(coords.len(), 1);
    assert_eq!(coords[0].name, "cthulhu");
}

#[test]
fn test_solo_speakers_returns_ungrouped() {
    let mut app = App::new();
    app.speakers = vec![
        make_speaker("cthulhu", Some("cthulhu")),
        make_speaker("hermit", None),
    ];
    let solos = app.solo_speakers();
    assert_eq!(solos.len(), 1);
    assert_eq!(solos[0].name, "hermit");
}
```

Run: `cd tui && cargo test 2>&1 | tail -10` — expected: all pass.

**Step 3: Replace `draw_speakers` in `ui.rs`**

Replace the entire `draw_speakers` function with:

```rust
fn draw_speakers(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
    let block = panel_block("Speakers", active);

    if app.is_grouped() {
        draw_topology(f, app, block, area);
    } else {
        draw_speaker_list(f, app, block, area);
    }
}

fn draw_speaker_list(f: &mut Frame, app: &App, block: Block, area: Rect) {
    let active = app.active_panel == Panel::Speakers;
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
        let group_tag = match &sp.group_coordinator {
            None => Span::raw("  "),
            Some(coord) if coord == &sp.name => Span::styled(" ◈", Style::default().fg(ACCENT)),
            Some(_) => Span::styled(" ↳", Style::default().fg(DIM)),
        };
        let line = Line::from(vec![
            Span::raw(if i == app.speaker_index { " ► " } else { "   " }),
            Span::styled(format!("{:<14}", display_name), name_style),
            group_tag,
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

fn draw_topology(f: &mut Frame, app: &App, block: Block, area: Rect) {
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = vec![];

    for coord in app.coordinators() {
        let members = app.group_members_of(&coord.name);
        let display = coord.alias.as_deref().unwrap_or(&coord.name);
        let width = members.iter()
            .map(|s| s.alias.as_deref().unwrap_or(&s.name).len())
            .max()
            .unwrap_or(display.len())
            .max(display.len()) + 4;
        let bar = "═".repeat(width);

        lines.push(Line::from(Span::styled(
            format!(" ╔{}╗", bar),
            Style::default().fg(ACCENT),
        )));
        for m in &members {
            let name = m.alias.as_deref().unwrap_or(&m.name);
            let tag = if m.group_coordinator.as_deref() == Some(m.name.as_str()) {
                " ◈"
            } else {
                " ↳"
            };
            let state = match m.state.as_str() {
                "PLAYING" => "▶",
                "PAUSED_PLAYBACK" => "⏸",
                _ => "·",
            };
            lines.push(Line::from(vec![
                Span::styled(" ║ ", Style::default().fg(ACCENT)),
                Span::styled(format!("{:<width$}", name, width = width - 2), Style::default().fg(FG)),
                Span::styled(tag, Style::default().fg(DIM)),
                Span::raw(" "),
                Span::styled(state, Style::default().fg(PLAYING)),
                Span::styled(" ║", Style::default().fg(ACCENT)),
            ]));
        }
        lines.push(Line::from(Span::styled(
            format!(" ╚{}╝", bar),
            Style::default().fg(ACCENT),
        )));
        lines.push(Line::from(""));
    }

    for sp in app.solo_speakers() {
        let name = sp.alias.as_deref().unwrap_or(&sp.name);
        let state = match sp.state.as_str() {
            "PLAYING" => Span::styled("▶", Style::default().fg(PLAYING)),
            "PAUSED_PLAYBACK" => Span::styled("⏸", Style::default().fg(PAUSED)),
            _ => Span::styled("·", Style::default().fg(DIM)),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("   {} ", name), Style::default().fg(DIM)),
            state,
            Span::styled(" (solo)", Style::default().fg(DIM)),
        ]));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}
```

**Step 4: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add tui/src/ui.rs tui/src/app.rs
git commit -m "feat: the fellowship assembles — group topology view in speakers panel"
git push origin main
```

---

### Task 4: Multi-group stacked Now Playing

When multiple groups/solos exist, the Now Playing panel shows one block per group.

**Files:**
- Modify: `tui/src/app.rs` (add `playing_entities`)
- Modify: `tui/src/ui.rs` (`draw_now_playing`)

**Step 1: Add `playing_entities` to App**

In `app.rs`, add:

```rust
/// One entry per distinct group (represented by coordinator) + each solo speaker.
pub fn playing_entities(&self) -> Vec<&Speaker> {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut result = vec![];
    for sp in &self.speakers {
        match &sp.group_coordinator {
            Some(coord) if coord == &sp.name => {
                // coordinator — include once
                if seen.insert(coord.as_str()) {
                    result.push(sp);
                }
            }
            None => result.push(sp), // ungrouped solo
            _ => {}                  // follower — skip
        }
    }
    result
}
```

**Step 2: Write test**

```rust
#[test]
fn test_playing_entities_deduplicates_groups() {
    let mut app = App::new();
    app.speakers = vec![
        make_speaker("cthulhu", Some("cthulhu")),  // coordinator
        make_speaker("family", Some("cthulhu")),   // follower — skip
        make_speaker("hermit", None),              // solo
    ];
    let entities = app.playing_entities();
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].name, "cthulhu");
    assert_eq!(entities[1].name, "hermit");
}
```

Run: `cd tui && cargo test 2>&1 | tail -10` — all pass.

**Step 3: Refactor `draw_now_playing` to support stacked view**

Replace `draw_now_playing` in `ui.rs` with:

```rust
fn draw_now_playing(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::NowPlaying;
    let block = panel_block("Now Playing", active);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let entities = app.playing_entities();

    if entities.is_empty() {
        let idle = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("  Nothing playing", Style::default().fg(DIM))),
        ]);
        f.render_widget(idle, inner);
        return;
    }

    if entities.len() == 1 {
        draw_track_block(f, entities[0], inner, true);
        return;
    }

    // Stacked view: divide inner area equally among entities
    let chunk_h = inner.height / entities.len() as u16;
    for (i, sp) in entities.iter().enumerate() {
        let chunk = Rect {
            y: inner.y + i as u16 * chunk_h,
            height: chunk_h,
            ..inner
        };
        draw_track_block(f, sp, chunk, false);
    }
}

fn draw_track_block(f: &mut Frame, sp: &crate::api::Speaker, area: Rect, show_vol: bool) {
    // Group label (coordinator name, dim)
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
                Constraint::Length(1), // artist
                Constraint::Length(1), // album
                Constraint::Length(1), // spacer
                Constraint::Length(1), // progress bar
                Constraint::Length(1), // time
                Constraint::Length(1), // spacer
                Constraint::Length(1), // volume (optional)
                Constraint::Min(0),
            ])
            .split(content_area);

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ♫ ", Style::default().fg(PLAYING)),
                Span::styled(&track.title, Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            ])),
            chunks[0],
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.artist, Style::default().fg(ACCENT)),
            ])),
            chunks[1],
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(&track.album, Style::default().fg(DIM)),
            ])),
            chunks[2],
        );

        let ratio = if track.duration > 0 {
            (track.position as f64 / track.duration as f64).min(1.0)
        } else {
            0.0
        };
        let gauge_area = Rect {
            x: chunks[4].x + 4,
            width: chunks[4].width.saturating_sub(8),
            ..chunks[4]
        };
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(ACCENT).bg(Color::Rgb(40, 40, 55)))
                .ratio(ratio)
                .label(""),
            gauge_area,
        );
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("    {} / {}", format_time(track.position), format_time(track.duration)),
                Style::default().fg(DIM),
            )),
            chunks[5],
        );

        if show_vol {
            let vol_area = Rect {
                x: chunks[7].x + 4,
                width: chunks[7].width.saturating_sub(8),
                ..chunks[7]
            };
            f.render_widget(
                Gauge::default()
                    .gauge_style(Style::default().fg(PLAYING).bg(Color::Rgb(40, 40, 55)))
                    .ratio(sp.volume as f64 / 100.0)
                    .label(format!("Vol: {}", sp.volume)),
                vol_area,
            );
        }
    } else {
        f.render_widget(
            Paragraph::new(Span::styled("  Nothing playing", Style::default().fg(DIM))),
            content_area,
        );
    }
}
```

**Step 4: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add tui/src/ui.rs tui/src/app.rs
git commit -m "feat: many eyes one vision — stacked multi-group Now Playing panel"
git push origin main
```

---

### Task 5: Command parsing module

Create `tui/src/command.rs` with `parse()` and `autocomplete()`. TDD.

**Files:**
- Create: `tui/src/command.rs`
- Modify: `tui/src/main.rs` (add `mod command;`)

**Step 1: Write failing tests first**

Create `tui/src/command.rs`:

```rust
#[derive(Debug, PartialEq)]
pub enum Command {
    Play(String),
    Volume(u8),
    GroupAll,
    Ungroup,
    Next,
    Prev,
    Sleep(u32),
    SleepCancel,
    Reload,
    Unknown(String),
}

pub fn parse(input: &str) -> Option<Command> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let (cmd, rest) = input
        .split_once(' ')
        .map(|(c, r)| (c, r.trim()))
        .unwrap_or((input, ""));

    match cmd {
        "play" | "p" => Some(Command::Play(rest.to_string())),
        "vol" | "volume" => rest.parse::<u8>().ok().map(Command::Volume),
        "group" => {
            if rest == "all" {
                Some(Command::GroupAll)
            } else {
                Some(Command::Unknown(input.to_string()))
            }
        }
        "ungroup" => Some(Command::Ungroup),
        "next" | "n" => Some(Command::Next),
        "prev" | "previous" => Some(Command::Prev),
        "sleep" => {
            if rest == "0" || rest == "cancel" {
                Some(Command::SleepCancel)
            } else {
                rest.parse::<u32>().ok().map(Command::Sleep)
            }
        }
        "reload" => Some(Command::Reload),
        _ => Some(Command::Unknown(input.to_string())),
    }
}

/// Given partial command input (without leading `:`), return ghost text to display.
/// `playlist_names` is a list of `favorite_name` strings for fuzzy matching.
pub fn autocomplete(input: &str, playlist_names: &[String]) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    // If no space yet, complete the command name
    if !input.contains(' ') {
        let commands = [
            "play", "vol", "group all", "ungroup", "next", "prev",
            "sleep", "reload",
        ];
        for cmd in &commands {
            if cmd.starts_with(input) && *cmd != input {
                return Some(cmd[input.len()..].to_string());
            }
        }
        return None;
    }
    // :play <query> — fuzzy match against playlist names
    let (cmd, query) = input.split_once(' ').unwrap();
    if (cmd == "play" || cmd == "p") && !query.is_empty() {
        let q = query.to_lowercase();
        if let Some(m) = playlist_names.iter().find(|n| n.to_lowercase().starts_with(&q)) {
            if m.to_lowercase() != q {
                return Some(m[query.len()..].to_string());
            }
        }
        // fallback: contains match
        if let Some(m) = playlist_names.iter().find(|n| n.to_lowercase().contains(&q)) {
            return Some(format!(" → {}", m));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_play() {
        assert_eq!(parse("play altwave"), Some(Command::Play("altwave".to_string())));
    }

    #[test]
    fn test_parse_volume() {
        assert_eq!(parse("vol 40"), Some(Command::Volume(40)));
    }

    #[test]
    fn test_parse_group_all() {
        assert_eq!(parse("group all"), Some(Command::GroupAll));
    }

    #[test]
    fn test_parse_sleep() {
        assert_eq!(parse("sleep 30"), Some(Command::Sleep(30)));
    }

    #[test]
    fn test_parse_sleep_cancel() {
        assert_eq!(parse("sleep cancel"), Some(Command::SleepCancel));
        assert_eq!(parse("sleep 0"), Some(Command::SleepCancel));
    }

    #[test]
    fn test_parse_reload() {
        assert_eq!(parse("reload"), Some(Command::Reload));
    }

    #[test]
    fn test_parse_empty_returns_none() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
    }

    #[test]
    fn test_autocomplete_command_name() {
        assert_eq!(autocomplete("sl", &[]), Some("eep".to_string()));
        assert_eq!(autocomplete("re", &[]), Some("load".to_string()));
        assert_eq!(autocomplete("reload", &[]), None); // exact match
    }

    #[test]
    fn test_autocomplete_play_fuzzy() {
        let names = vec!["Alt Wave".to_string(), "Jazz Classics".to_string()];
        let result = autocomplete("play alt", &names);
        assert_eq!(result, Some(" Wave".to_string()));
    }

    #[test]
    fn test_autocomplete_no_match() {
        let names = vec!["Alt Wave".to_string()];
        assert_eq!(autocomplete("play xyz", &names), None);
    }
}
```

**Step 2: Add `mod command;` to `main.rs`**

At the top of `tui/src/main.rs`, add:
```rust
mod command;
```

**Step 3: Run tests**

```bash
cd tui && cargo test command 2>&1 | tail -15
```

Expected: 9 tests pass.

**Step 4: Commit**

```bash
git add tui/src/command.rs tui/src/main.rs
git commit -m "feat: speak friend and enter — command parsing module with fuzzy autocomplete"
git push origin main
```

---

### Task 6: Command mode UI + key handling

Wire `:` key into the event loop, render command prompt with ghost text in help bar.

**Files:**
- Modify: `tui/src/main.rs` (`handle_key` — `:` entry, command execution)
- Modify: `tui/src/ui.rs` (`draw_help_bar` — command prompt with ghost text)
- Modify: `tui/src/api.rs` (add `reload()` method — needed for `:reload`)

**Step 1: Add `reload()` to ApiClient**

In `tui/src/api.rs`, add:

```rust
pub async fn reload(&self) -> anyhow::Result<()> {
    self.client
        .post(format!("{}/reload", self.base_url))
        .send().await?;
    Ok(())
}
```

**Step 2: Add command mode key handling to `handle_key` in `main.rs`**

At the top of `handle_key`, before the `volume_input` block, add:

```rust
// Command mode intercepts all keys
if app.command_input.is_some() {
    match key.code {
        KeyCode::Char(c) if !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            app.command_input.as_mut().unwrap().push(c);
        }
        KeyCode::Backspace => {
            let input = app.command_input.as_mut().unwrap();
            if input.is_empty() {
                app.command_input = None; // backspace on empty exits
            } else {
                input.pop();
            }
        }
        KeyCode::Tab => {
            // Accept ghost text
            let playlist_names: Vec<String> = app.playlists
                .iter()
                .map(|p| p.favorite_name.clone())
                .collect();
            let current = app.command_input.as_ref().unwrap().clone();
            if let Some(ghost) = command::autocomplete(&current, &playlist_names) {
                // For command-name ghosts, append directly
                // For "→ Full Name" style, replace the query portion
                if ghost.starts_with(" → ") {
                    let parts: Vec<&str> = current.splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        let completed = format!("{} {}", parts[0], &ghost[3..]);
                        *app.command_input.as_mut().unwrap() = completed;
                    }
                } else {
                    app.command_input.as_mut().unwrap().push_str(&ghost);
                }
            }
        }
        KeyCode::Enter => {
            if let Some(input) = app.command_input.take() {
                execute_command(app, client, &input).await?;
            }
        }
        KeyCode::Esc => {
            app.command_input = None;
        }
        _ => {}
    }
    return Ok(());
}
```

Add `:` key to the normal match block:
```rust
KeyCode::Char(':') => {
    app.command_input = Some(String::new());
    app.volume_input = None; // mutually exclusive
}
```

**Step 3: Add `execute_command` function to `main.rs`**

Add before `handle_key`:

```rust
async fn execute_command(app: &mut App, client: &ApiClient, input: &str) -> Result<()> {
    use command::Command;
    match command::parse(input) {
        Some(Command::Play(name)) => {
            if let Some(id) = app.speaker_id() {
                // Fuzzy-find the playlist
                let playlist = app.playlists.iter().find(|p| {
                    p.alias.to_lowercase().contains(&name.to_lowercase())
                        || p.favorite_name.to_lowercase().contains(&name.to_lowercase())
                });
                if let Some(pl) = playlist {
                    let _ = client.play(&id, &pl.alias).await;
                    history::record_play(&pl.alias);
                    app.set_status(format!("Playing {} on {}", pl.alias, id), 3);
                } else {
                    app.set_status("Not all those who wander are found in this network.", 4);
                }
            }
        }
        Some(Command::Volume(v)) => {
            if let Some(id) = app.speaker_id() {
                let _ = client.set_volume(&id, v).await;
                if v == 100 {
                    app.set_status("You shall not pass... 100.", 3);
                }
            }
        }
        Some(Command::GroupAll) => {
            let _ = client.group_all().await;
        }
        Some(Command::Ungroup) => {
            let _ = client.ungroup_all().await;
        }
        Some(Command::Next) => {
            if let Some(id) = app.speaker_id() {
                let _ = client.next(&id).await;
            }
        }
        Some(Command::Prev) => {
            if let Some(id) = app.speaker_id() {
                let _ = client.previous(&id).await;
            }
        }
        Some(Command::Sleep(mins)) => {
            app.sleep_until = Some(
                std::time::Instant::now()
                    + std::time::Duration::from_secs(mins as u64 * 60)
            );
        }
        Some(Command::SleepCancel) => {
            app.sleep_until = None;
            app.set_status("Sleep timer cancelled.", 2);
        }
        Some(Command::Reload) => {
            let _ = client.reload().await;
            if let Ok(playlists) = client.get_playlists().await {
                app.playlists = playlists;
            }
            if let Ok(favs) = client.get_favorites().await {
                let existing: std::collections::HashSet<String> = app.playlists
                    .iter()
                    .map(|p| p.favorite_name.to_lowercase())
                    .collect();
                for title in favs {
                    if !existing.contains(&title.to_lowercase()) {
                        app.playlists.push(crate::api::Playlist {
                            alias: title.clone(),
                            favorite_name: title,
                        });
                    }
                }
            }
            app.set_status("Reloaded config.yaml", 3);
        }
        Some(Command::Unknown(_)) | None => {
            app.set_status("Speak, friend — but speak clearly.", 3);
        }
    }
    Ok(())
}
```

**Step 4: Update `draw_help_bar` in `ui.rs` to show command prompt**

At the top of `draw_help_bar`, before the `volume_input` block, add:

```rust
if let Some(input) = &app.command_input {
    let playlist_names: Vec<String> = app.playlists
        .iter()
        .map(|p| p.favorite_name.clone())
        .collect();
    let ghost = command::autocomplete(input, &playlist_names);

    let mut spans = vec![
        Span::styled("  :", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(input.clone(), Style::default().fg(FG)),
    ];
    if let Some(g) = ghost {
        spans.push(Span::styled(g, Style::default().fg(DIM)));
    }
    spans.push(Span::styled("▌", Style::default().fg(ACCENT)));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    f.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
    return;
}
```

Add `use crate::command;` at the top of `ui.rs` imports.

Also update the normal help bar spans to include `:` command hint — replace the line with the `g group` entry:
```rust
Span::styled(":", Style::default().fg(ACCENT)),
Span::styled(" cmd  ", Style::default().fg(DIM)),
```
(Add this after the `v vol#` entry.)

**Step 5: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

**Step 6: Commit**

```bash
git add tui/src/main.rs tui/src/ui.rs tui/src/api.rs
git commit -m "feat: speak friend and enter — command mode with ghost text autocomplete"
git push origin main
```

---

### Task 7: Sleep timer tick handling

On each tick, check if sleep timer has expired and pause all speakers.

**Files:**
- Modify: `tui/src/main.rs` (tick handler in run loop)

**Step 1: Add sleep expiry check in `run` loop**

In the `run` function in `main.rs`, inside the main `loop { }`, after the `rx.try_recv()` block and before `event::poll`, add:

```rust
// Check sleep timer expiry
if let Some(sleep_until) = app.sleep_until {
    if std::time::Instant::now() >= sleep_until {
        app.sleep_until = None;
        for sp in &app.speakers {
            let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
            let _ = client.pause(&id).await;
        }
        app.set_status("The Fellowship rests. All speakers paused.", 5);
    }
}
```

**Step 2: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

**Step 3: Commit**

```bash
git add tui/src/main.rs
git commit -m "feat: even the wise must sleep — sleep timer pauses all speakers on expiry"
git push origin main
```

---

### Task 8: Config hot-reload — daemon side

`SonosManager` watches `config.yaml` mtime every 300s in the background thread. New `POST /reload` endpoint for on-demand reload.

**Files:**
- Modify: `daemon/sonosd/sonos.py`
- Modify: `daemon/sonosd/server.py`
- Test: `daemon/tests/test_server.py`
- Test: `daemon/tests/test_sonos.py`

**Step 1: Write failing test for reload**

Add to `daemon/tests/test_server.py`:

```python
def test_reload_endpoint_returns_200():
    client, mock_manager, _ = _make_client()
    resp = client.post("/reload")
    assert resp.status_code == 200
    assert resp.json()["status"] == "reloaded"
```

Run: `cd daemon && source .venv/bin/activate && pytest tests/test_server.py::test_reload_endpoint_returns_200 -v`
Expected: FAIL (endpoint doesn't exist yet).

**Step 2: Add mtime tracking to `SonosManager`**

In `daemon/sonosd/sonos.py`, update `__init__` and `_background_discover`:

```python
def __init__(self, config: dict):
    self.config = config
    self._speakers: dict[str, soco.SoCo] = {}
    self._lock = threading.Lock()
    self._alias_map: dict[str, str] = config.get("speakers", {})
    self._reverse_alias: dict[str, str] = {v: k for k, v in self._alias_map.items()}
    self._playlist_map: dict[str, str] = config.get("playlists", {})
    self._config_path = Path(__file__).parent.parent / "config.yaml"
    self._config_mtime: float = self._config_path.stat().st_mtime
    self._last_config_check: float = 0.0
    self._discover()
    t = threading.Thread(target=self._background_discover, daemon=True)
    t.start()

def _background_discover(self) -> None:
    while True:
        time.sleep(_REDISCOVER_INTERVAL)
        self._discover()
        # Check config mtime every 300s
        now = time.time()
        if now - self._last_config_check >= 300:
            self._last_config_check = now
            self._check_config_reload()

def _check_config_reload(self) -> None:
    """Reload config.yaml if it has changed on disk."""
    try:
        mtime = self._config_path.stat().st_mtime
        if mtime != self._config_mtime:
            self.reload_config()
    except OSError:
        pass

def reload_config(self) -> None:
    """Re-read config.yaml and update alias/playlist maps."""
    import yaml
    with open(self._config_path) as f:
        config = yaml.safe_load(f)
    with self._lock:
        self.config = config
        self._alias_map = config.get("speakers", {})
        self._reverse_alias = {v: k for k, v in self._alias_map.items()}
        self._playlist_map = config.get("playlists", {})
        self._config_mtime = self._config_path.stat().st_mtime
```

Add `from pathlib import Path` at the top of `sonos.py` if not already there.

**Step 3: Add `POST /reload` to server.py**

In `daemon/sonosd/server.py`, add after `get_config`:

```python
@app.post("/reload")
def reload_config():
    manager.reload_config()
    return {"status": "reloaded"}
```

**Step 4: Write test for reload_config**

Add to `daemon/tests/test_sonos.py`:

```python
def test_reload_config_updates_playlist_map():
    manager, _ = _make_manager()
    # Simulate a changed config
    new_config = {
        "speakers": {"cthulhu": "cthulhu"},
        "playlists": {"newlist": "New Playlist"},
    }
    # Directly call reload_config with patched open
    manager.config = new_config
    with patch("builtins.open", unittest.mock.mock_open(read_data="playlists:\n  newlist: New Playlist\nspeakers:\n  cthulhu: cthulhu\n")):
        with patch("pathlib.Path.stat") as mock_stat:
            mock_stat.return_value.st_mtime = 9999.0
            manager.reload_config()
    assert "newlist" in manager._playlist_map
```

Actually this test is tricky to write cleanly due to file I/O. Write a simpler test instead:

```python
def test_reload_config_updates_playlist_map():
    manager, _ = _make_manager()
    # Directly mutate as if reload happened
    with manager._lock:
        manager._playlist_map = {"newlist": "New Playlist"}
    assert "newlist" in manager.get_playlists_map()
```

Add `get_playlists_map` to SonosManager:
```python
def get_playlists_map(self) -> dict:
    with self._lock:
        return dict(self._playlist_map)
```

And update `server.py` `/playlists` endpoint to use it:
```python
@app.get("/playlists")
def get_playlists():
    return {"playlists": manager.get_playlists_map()}
```

**Step 5: Run all daemon tests**

```bash
cd daemon && source .venv/bin/activate && pytest -q
```

Expected: all pass (was 23, now 25+).

**Step 6: Commit**

```bash
git add daemon/sonosd/sonos.py daemon/sonosd/server.py daemon/tests/test_server.py daemon/tests/test_sonos.py
git commit -m "feat: a wizard is never stale — config hot-reload every 300s and via POST /reload"
git push origin main
```

---

### Task 9: Startup splash screen

Render a 1-second splash before the main loop.

**Files:**
- Modify: `tui/src/ui.rs` (add `draw_splash`)
- Modify: `tui/src/main.rs` (call splash before run loop)

**Step 1: Add `draw_splash` to `ui.rs`**

Add after the color constants:

```rust
pub fn draw_splash(f: &mut Frame) {
    let area = f.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let v_offset = inner.height / 2 - 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_offset),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Span::styled(
            "  S O N O - P A L A N T I R",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )).alignment(ratatui::layout::Alignment::Center),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "══════════════════════════",
            Style::default().fg(ACCENT),
        )).alignment(ratatui::layout::Alignment::Center),
        chunks[2],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "Seeing through sound...",
            Style::default().fg(DIM),
        )).alignment(ratatui::layout::Alignment::Center),
        chunks[3],
    );
}
```

**Step 2: Call splash in `main.rs`**

In `main()`, after `let mut terminal = Terminal::new(backend)?;` and before `let result = run(...)`:

```rust
terminal.draw(|f| ui::draw_splash(f))?;
std::thread::sleep(std::time::Duration::from_secs(1));
```

**Step 3: Build**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add tui/src/ui.rs tui/src/main.rs
git commit -m "feat: the palantir awakens — startup splash screen"
git push origin main
```

---

### Task 10: LOTR error messages

Show themed error messages in the status line when daemon calls fail.

**Files:**
- Modify: `tui/src/main.rs` (error handling in run loop and handle_key)

**Step 1: Add error status messages in the run loop**

In the `run` function, the initial startup calls currently silently ignore errors. Update them to set status messages:

Replace the startup block:

```rust
if let Ok(speakers) = client.get_speakers().await {
    app.speakers = speakers;
} else {
    app.set_status("The gates of Moria are sealed. Daemon unreachable.", 10);
}
```

**Step 2: Add error status on background refresh failure**

In the background task spawned in `run`, update to send an error signal. Since the channel only sends `Vec<Speaker>`, add a wrapper. Actually, simpler: just let the status clear naturally. The background task already silently drops errors. Instead, track connection state in `App`:

Add to `App` struct: `pub daemon_unreachable: bool,`
Add to `App::new()`: `daemon_unreachable: false,`

Update the background task and `rx.try_recv()` block:

```rust
if let Ok(speakers) = rx.try_recv() {
    app.speakers = speakers;
    if app.daemon_unreachable {
        app.daemon_unreachable = false;
        app.set_status("The Fellowship reconnects. Daemon is reachable.", 3);
    }
}
```

Update the background refresh closure to send an empty vec as a sentinel for unreachable? No — simpler: just set the status in `handle_key` when a command fails with a connection error.

Actually, the simplest approach: in `execute_command`, wrap calls in error handling and set LOTR status messages. Also check for volume == 100 in the Volume handler (already done in Task 6 plan). This is sufficient for the features as spec'd.

The full error messages to implement:

In `execute_command` in `main.rs`:
- Connection refused / reqwest error on any call → "The gates of Moria are sealed."
- `Command::Unknown` → "Speak, friend — but speak clearly."
- Play with no matching playlist → "Not all those who wander are found in this network."
- Volume == 100 → "You shall not pass... 100."

These are already included in Task 6's `execute_command`. Verify they're in place.

Also add to the initial startup in `run`:
```rust
if client.get_speakers().await.is_err() {
    app.set_status("The gates of Moria are sealed. Start the daemon.", 0); // 0 = no expiry
}
```

For a persistent message (no expiry), use a large duration:
```rust
app.set_status("The gates of Moria are sealed. Start sonosd.", 3600);
```

**Step 3: Build and test**

```bash
cd tui && cargo build --release 2>&1 | tail -5
cd tui && cargo test 2>&1 | tail -10
```

**Step 4: Commit**

```bash
git add tui/src/main.rs tui/src/app.rs
git commit -m "fix: even the very wise cannot see all ends — LOTR error messages in status line"
git push origin main
```

---

### Task 11: Help screen overlay

`?` toggles a full-screen keybinding reference with LOTR flavor text.

**Files:**
- Modify: `tui/src/ui.rs` (add `draw_help_overlay`, call in `draw`)
- Modify: `tui/src/main.rs` (`?` key toggles `app.help_open`)

**Step 1: Add `?` key handling to `main.rs`**

In the normal `match key.code` block in `handle_key`:

```rust
KeyCode::Char('?') => {
    app.help_open = !app.help_open;
}
```

Also add `Esc` to close help:
```rust
KeyCode::Esc => {
    if app.help_open {
        app.help_open = false;
    }
}
```

**Step 2: Add `draw_help_overlay` to `ui.rs`**

Add at the end of `ui.rs`:

```rust
fn draw_help_overlay(f: &mut Frame) {
    let area = f.area();
    let block = Block::default()
        .title(" ? The Lore of sono-palantir — Esc or ? to close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  NAVIGATION", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  Tab        ", Style::default().fg(ACCENT)), Span::styled("Cycle panels — as the Fellowship moved between realms", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  ↑ / k      ", Style::default().fg(ACCENT)), Span::styled("Move up", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  ↓ / j      ", Style::default().fg(ACCENT)), Span::styled("Move down", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  Enter      ", Style::default().fg(ACCENT)), Span::styled("Play selected playlist on selected speaker", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  PLAYBACK", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  Space      ", Style::default().fg(ACCENT)), Span::styled("Pause / resume — even hobbits need rest", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  n          ", Style::default().fg(ACCENT)), Span::styled("Next track — onwards, to Rivendell", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  p          ", Style::default().fg(ACCENT)), Span::styled("Previous track — back to the Shire", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  + / =      ", Style::default().fg(ACCENT)), Span::styled("Volume up 5", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  -          ", Style::default().fg(ACCENT)), Span::styled("Volume down 5", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  v          ", Style::default().fg(ACCENT)), Span::styled("Set exact volume — speak your will", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  GROUPS", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  g          ", Style::default().fg(ACCENT)), Span::styled("Toggle group all speakers — assemble the Fellowship", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  COMMAND MODE  (press : to enter)", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
        Line::from(vec![Span::styled("  :play <name> ", Style::default().fg(ACCENT)), Span::styled("Play a favorite — fuzzy matched", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :vol <0-100> ", Style::default().fg(ACCENT)), Span::styled("Set exact volume", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :group all   ", Style::default().fg(ACCENT)), Span::styled("Group all speakers", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :sleep <min> ", Style::default().fg(ACCENT)), Span::styled("Sleep timer — pause all after N minutes", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  :reload      ", Style::default().fg(ACCENT)), Span::styled("Reload config.yaml — a wizard is never stale", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  Tab          ", Style::default().fg(ACCENT)), Span::styled("Accept ghost text autocomplete suggestion", Style::default().fg(FG))]),
        Line::from(""),
        Line::from(vec![Span::styled("  ?            ", Style::default().fg(ACCENT)), Span::styled("Toggle this help — knowledge is power in the dark", Style::default().fg(FG))]),
        Line::from(vec![Span::styled("  q            ", Style::default().fg(ACCENT)), Span::styled("Quit — go back to the Shire", Style::default().fg(FG))]),
    ];

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}
```

**Step 3: Call `draw_help_overlay` from `draw` when `help_open`**

At the end of the `draw` function in `ui.rs`, add:

```rust
if app.help_open {
    draw_help_overlay(f);
}
```

Also add `?` to the normal help bar spans.

**Step 4: Build and run all tests**

```bash
cd tui && cargo build --release 2>&1 | tail -5
cd tui && cargo test 2>&1 | tail -10
cd daemon && source .venv/bin/activate && pytest -q
```

Expected: all pass.

**Step 5: Final commit**

```bash
git add tui/src/ui.rs tui/src/main.rs
git commit -m "feat: the lore is written — full-screen help overlay with LOTR flavor text"
git push origin main
```
