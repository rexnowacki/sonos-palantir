# Todo Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the four items from todo.md: grouped speaker display, follower Now Playing, favorites merge, and volume input mode.

**Architecture:** Three layers — Python daemon (`daemon/sonosd/`), Rust API client (`tui/src/api.rs`), Rust TUI (`tui/src/app.rs`, `ui.rs`, `main.rs`). Each feature touches one or both layers. All daemon changes have pytest tests; Rust logic changes have `#[cfg(test)]` tests.

**Tech Stack:** Python/soco/FastAPI (daemon), Rust/Ratatui/tokio/reqwest (TUI), pytest, cargo test.

---

## Key facts before you start

- `group_coordinator` in the API response is the **coordinator's player_name** (real Sonos device name).
- A speaker is a **coordinator** when `group_coordinator == Some(speaker.name)`.
- A speaker is a **follower** when `group_coordinator == Some(other_name)`.
- A speaker is **ungrouped/solo** when `group_coordinator == None`.
- Run daemon tests: `cd daemon && source .venv/bin/activate && pytest -q`
- Run TUI tests: `cd tui && cargo test`
- Build TUI: `cd tui && cargo build --release`

---

### Task 1: Grouped speaker visual indicators

Show `◈` after coordinator names and `↳` after follower names in the Speakers panel.

**Files:**
- Modify: `tui/src/ui.rs:55-86` (`draw_speakers` function)

**Step 1: Add group indicator logic to `draw_speakers`**

In `ui.rs`, replace the existing `draw_speakers` items map body with this (inside the `.map(|(i, sp)| { ... })` closure, after `state_icon` is defined and before `let line = ...`):

```rust
let group_tag = match &sp.group_coordinator {
    None => Span::raw("   "),
    Some(coord) if coord == &sp.name => {
        Span::styled(" ◈", Style::default().fg(ACCENT))
    }
    Some(_) => Span::styled(" ↳", Style::default().fg(DIM)),
};
```

Then update the `Line::from(vec![...])` to include `group_tag` after `display_name`:

```rust
let line = Line::from(vec![
    Span::raw(if i == app.speaker_index { " ► " } else { "   " }),
    Span::styled(format!("{:<14}", display_name), name_style),
    group_tag,
    Span::styled(format!(" {:>3}", sp.volume), Style::default().fg(DIM)),
    Span::raw("  "),
    state_icon,
]);
```

(Note: shrink name field from 16 to 14 to make room for the 2-char tag.)

**Step 2: Build and visually verify**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

Expected: `Finished release profile` with no errors.

Run the TUI and confirm grouped speakers show `◈`/`↳`.

**Step 3: Commit**

```bash
cd /Users/christophernowacki/sonos
git add tui/src/ui.rs
git commit -m "feat: show group coordinator/follower indicators in speakers panel"
```

---

### Task 2: Follower speakers show coordinator's track info

When a grouped follower speaker is selected, Now Playing should show the track — currently shows "Nothing playing" because the follower's `get_current_track_info()` may return an empty title.

**Files:**
- Modify: `daemon/sonosd/sonos.py:39-66` (`get_speaker_info`)
- Test: `daemon/tests/test_sonos.py`

**Step 1: Write the failing test**

Add to `daemon/tests/test_sonos.py`:

```python
def test_get_speaker_info_follower_uses_coordinator_track():
    """If a follower has no track info, it should fall back to the coordinator's."""
    manager, mock_follower = _make_manager()

    mock_coordinator = MagicMock()
    mock_coordinator.player_name = "Family Room"
    mock_follower.group = MagicMock()
    mock_follower.group.coordinator = mock_coordinator

    mock_follower.get_current_transport_info.return_value = {
        "current_transport_state": "PLAYING"
    }
    mock_follower.get_current_track_info.return_value = {
        "title": "",
        "artist": "",
        "album": "",
        "duration": "0:00:00",
        "position": "0:00:00",
        "album_art": "",
    }
    mock_coordinator.get_current_track_info.return_value = {
        "title": "Alt Wave Track",
        "artist": "Some Artist",
        "album": "Some Album",
        "duration": "0:03:00",
        "position": "0:01:00",
        "album_art": "",
    }

    info = manager.get_speaker_info(mock_follower)

    assert info["track"] is not None
    assert info["track"]["title"] == "Alt Wave Track"
    assert info["track"]["artist"] == "Some Artist"
```

**Step 2: Run the test to confirm it fails**

```bash
cd daemon && source .venv/bin/activate && pytest tests/test_sonos.py::test_get_speaker_info_follower_uses_coordinator_track -v
```

Expected: FAIL — `AssertionError: assert None is not None`

**Step 3: Implement the fix in `get_speaker_info`**

In `daemon/sonosd/sonos.py`, replace the `get_speaker_info` method with:

```python
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

    coordinator_sp = speaker.group.coordinator if speaker.group else None
    coordinator_name = coordinator_sp.player_name if coordinator_sp else None

    # Follower has no track info — fetch from coordinator
    if track is None and coordinator_sp and coordinator_sp != speaker:
        coord_track = coordinator_sp.get_current_track_info()
        if coord_track.get("title"):
            track = {
                "title": coord_track.get("title", ""),
                "artist": coord_track.get("artist", ""),
                "album": coord_track.get("album", ""),
                "duration": _parse_duration(coord_track.get("duration", "0:00:00")),
                "position": _parse_duration(coord_track.get("position", "0:00:00")),
                "art_uri": coord_track.get("album_art", ""),
            }

    return {
        "name": speaker.player_name,
        "alias": self._reverse_alias.get(speaker.player_name),
        "ip": speaker.ip_address,
        "volume": speaker.volume,
        "muted": speaker.mute,
        "state": info.get("current_transport_state", "UNKNOWN"),
        "group_coordinator": coordinator_name,
        "track": track,
    }
```

**Step 4: Run all daemon tests**

```bash
cd daemon && source .venv/bin/activate && pytest -q
```

Expected: all tests pass (was 19, now 20).

**Step 5: Commit**

```bash
cd /Users/christophernowacki/sonos
git add daemon/sonosd/sonos.py daemon/tests/test_sonos.py
git commit -m "fix: follower speakers fall back to coordinator track info in Now Playing"
```

---

### Task 3: Favorites merge on startup

On startup, call `GET /favorites` and add any Sonos Favorites not already aliased in `config.yaml` to the playlists panel.

**Files:**
- Modify: `tui/src/api.rs` (add `get_favorites`)
- Modify: `tui/src/main.rs` (call on startup, merge)
- Test: `daemon/tests/test_server.py` (add `test_get_favorites`)

**Step 1: Write the failing server test**

Add to `daemon/tests/test_server.py`:

```python
def test_get_favorites():
    client, mock_manager, mock_speaker = _make_client()
    mock_fav = MagicMock()
    mock_fav.title = "Jazz Classics"
    mock_speaker.music_library.get_sonos_favorites.return_value = [mock_fav]
    resp = client.get("/favorites")
    assert resp.status_code == 200
    data = resp.json()
    assert any(f["title"] == "Jazz Classics" for f in data["favorites"])
```

**Step 2: Run to confirm it fails**

```bash
cd daemon && source .venv/bin/activate && pytest tests/test_server.py::test_get_favorites -v
```

Expected: FAIL — the endpoint exists but the mock speaker isn't set up the same way `get_speakers` returns it. Check if it fails for the right reason (it may actually pass already if the endpoint works). If it passes, move on.

**Step 3: Add `get_favorites` to `ApiClient`**

In `tui/src/api.rs`, add after `get_playlists`:

```rust
pub async fn get_favorites(&self) -> anyhow::Result<Vec<String>> {
    let resp: serde_json::Value = self.client
        .get(format!("{}/favorites", self.base_url))
        .send().await?
        .json().await?;
    let favs = resp["favorites"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(favs.iter()
        .filter_map(|f| f["title"].as_str().map(|s| s.to_string()))
        .collect())
}
```

**Step 4: Merge favorites in `main.rs`**

In `tui/src/main.rs`, after the `get_playlists` block (around line 43), add:

```rust
if let Ok(playlists) = client.get_playlists().await {
    app.playlists = playlists;
}
// Merge any Sonos Favorites not already covered by a config alias
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
```

Replace the existing `get_playlists` block (keep just one call); the full startup section should look like:

```rust
if let Ok(speakers) = client.get_speakers().await {
    app.speakers = speakers;
}
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
```

**Step 5: Build the TUI**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

Expected: `Finished release profile` with no errors.

**Step 6: Run all daemon tests**

```bash
cd daemon && source .venv/bin/activate && pytest -q
```

Expected: all tests pass.

**Step 7: Commit**

```bash
cd /Users/christophernowacki/sonos
git add tui/src/api.rs tui/src/main.rs daemon/tests/test_server.py
git commit -m "feat: merge Sonos Favorites into playlists panel on startup"
```

---

### Task 4: Volume input mode

Press `v` to enter a volume input mode. Type 0–3 digits. Press `Enter` to apply, `Esc` to cancel. The help bar shows the prompt while in this mode.

**Files:**
- Modify: `tui/src/app.rs` (add `volume_input` field + tests)
- Modify: `tui/src/main.rs` (key handling for volume input mode)
- Modify: `tui/src/ui.rs` (`draw_help_bar` takes `app`, shows prompt)

**Step 1: Add `volume_input` field to `App` and write tests**

In `tui/src/app.rs`, add to the `App` struct (after `status_message`):

```rust
pub volume_input: Option<String>,
```

Add to `App::new()`:

```rust
volume_input: None,
```

Add these tests to the `#[cfg(test)]` block:

```rust
#[test]
fn test_volume_input_starts_none() {
    let app = App::new();
    assert!(app.volume_input.is_none());
}

#[test]
fn test_volume_input_can_be_set() {
    let mut app = App::new();
    app.volume_input = Some(String::from("42"));
    assert_eq!(app.volume_input.as_deref(), Some("42"));
}
```

**Step 2: Run tests to verify they pass**

```bash
cd tui && cargo test 2>&1 | tail -10
```

Expected: all tests pass (was 3, now 5).

**Step 3: Update key handling in `main.rs`**

In `handle_key`, add volume input mode handling at the **top** of the function, before the main `match`:

```rust
async fn handle_key(app: &mut App, client: &ApiClient, key: KeyEvent) -> Result<()> {
    // Volume input mode intercepts all keys
    if app.volume_input.is_some() {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let input = app.volume_input.as_mut().unwrap();
                if input.len() < 3 {
                    input.push(c);
                }
            }
            KeyCode::Backspace => {
                app.volume_input.as_mut().unwrap().pop();
            }
            KeyCode::Enter => {
                if let Some(input) = app.volume_input.take() {
                    if let Ok(vol) = input.parse::<u8>() {
                        let vol = vol.min(100);
                        if let Some(id) = app.speaker_id() {
                            let _ = client.set_volume(&id, vol).await;
                        }
                    }
                }
            }
            KeyCode::Esc => {
                app.volume_input = None;
            }
            _ => {}
        }
        return Ok(());
    }

    match key.code {
        // ... existing match arms unchanged ...
```

Also add a `v` arm inside the existing `match key.code` block:

```rust
KeyCode::Char('v') => {
    app.volume_input = Some(String::new());
}
```

**Step 4: Update `draw_help_bar` in `ui.rs` to accept `app`**

Change the signature of `draw_help_bar` from:

```rust
fn draw_help_bar(f: &mut Frame, area: Rect) {
```

to:

```rust
fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
```

Update the call site in `draw` (line 39):

```rust
draw_help_bar(f, app, outer[1]);
```

At the top of `draw_help_bar`, add volume input rendering before the existing help text:

```rust
fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    if let Some(input) = &app.volume_input {
        let prompt = Line::from(vec![
            Span::styled("  Vol: ", Style::default().fg(ACCENT)),
            Span::styled(
                format!("[{}▌]", input),
                Style::default().fg(FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Enter confirm   Esc cancel", Style::default().fg(DIM)),
        ]);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT))
            .style(Style::default().bg(BG));
        f.render_widget(Paragraph::new(prompt).block(block), area);
        return;
    }

    // existing help bar below unchanged...
```

Also add `v` to the existing help bar `Line::from(vec![...])`:

```rust
Span::styled("v", Style::default().fg(ACCENT)),
Span::styled(" vol#  ", Style::default().fg(DIM)),
```

(Add this after the `+/-` vol entry.)

**Step 5: Build the TUI**

```bash
cd tui && cargo build --release 2>&1 | tail -5
```

Expected: `Finished release profile` with no errors.

**Step 6: Run all tests**

```bash
cd tui && cargo test 2>&1 | tail -10
cd /Users/christophernowacki/sonos/daemon && source .venv/bin/activate && pytest -q
```

Expected: all pass.

**Step 7: Update todo.md to mark all items complete, then commit**

Mark all four items in `todo.md` as done (add `[x]` or `~~strikethrough~~` to each heading).

```bash
cd /Users/christophernowacki/sonos
git add tui/src/app.rs tui/src/main.rs tui/src/ui.rs todo.md
git commit -m "feat: volume input mode — press v then digits then Enter to set exact volume"
```
