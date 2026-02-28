# Group Toggle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Family Room to config and expose a `g` keybinding that toggles all speakers between grouped and ungrouped.

**Architecture:** Config gets a new alias. `ApiClient` gets two new methods. `App` gets a pure `is_grouped()` helper that inspects existing speaker state. `main.rs` gets a `g` keybinding that calls the right method. Help bar gets updated.

**Tech Stack:** Python/YAML (config), Rust/ratatui (TUI)

---

### Task 1: Add Family Room to config

**Files:**
- Modify: `daemon/config.yaml`

**Step 1: Edit `daemon/config.yaml`**

```yaml
# Playlist aliases — map short names to Sonos Favorites
playlists:
  altwave: "Alt Wave"

# Speaker aliases — map short names to device names
speakers:
  cthulhu: "cthulhu"
  family: "Family Room"

# Defaults
default_speaker: cthulhu
default_volume: 25

# Server
host: "127.0.0.1"
port: 9271
```

**Step 2: Verify daemon reflects new alias**

Restart the daemon if running, then:

```bash
curl -s http://localhost:9271/speakers | python -m json.tool
```

Expected: Family Room entry now has `"alias": "family"` instead of `null`.

**Step 3: Commit**

```bash
git add daemon/config.yaml
git commit -m "feat: add Family Room speaker alias to config"
```

---

### Task 2: Add `group_all` and `ungroup_all` to `ApiClient`

**Files:**
- Modify: `tui/src/api.rs`

**Step 1: Add two methods to `ApiClient` in `tui/src/api.rs`**

After the `previous` method (around line 120), add:

```rust
    pub async fn group_all(&self) -> anyhow::Result<()> {
        self.client.post(format!("{}/group", self.base_url))
            .json(&serde_json::json!({"speakers": ["all"]}))
            .send().await?;
        Ok(())
    }

    pub async fn ungroup_all(&self) -> anyhow::Result<()> {
        self.client.post(format!("{}/ungroup", self.base_url))
            .json(&SpeakerRequest { speaker: "all".to_string() })
            .send().await?;
        Ok(())
    }
```

**Step 2: Verify it compiles**

```bash
cd tui && cargo build 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

**Step 3: Commit**

```bash
cd ..
git add tui/src/api.rs
git commit -m "feat: add group_all and ungroup_all to ApiClient"
```

---

### Task 3: Add `is_grouped()` to `App` with tests

**Files:**
- Modify: `tui/src/app.rs`

**Step 1: Write a test**

Sonos reports grouping via `group_coordinator`. When speakers are grouped, at least one speaker will have a `group_coordinator` that differs from its own name (it's a follower). When ungrouped, every speaker is its own coordinator (or coordinator is `None`).

Add a `#[cfg(test)]` block at the bottom of `tui/src/app.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Speaker, Track};

    fn make_speaker(name: &str, coordinator: Option<&str>) -> Speaker {
        Speaker {
            name: name.to_string(),
            alias: None,
            ip: "0.0.0.0".to_string(),
            volume: 25,
            muted: false,
            state: "PLAYING".to_string(),
            group_coordinator: coordinator.map(|s| s.to_string()),
            track: None,
        }
    }

    #[test]
    fn test_is_grouped_when_follower_present() {
        let mut app = App::new();
        // Family Room is coordinator, cthulhu is follower
        app.speakers = vec![
            make_speaker("Family Room", Some("Family Room")),
            make_speaker("cthulhu", Some("Family Room")),
        ];
        assert!(app.is_grouped());
    }

    #[test]
    fn test_is_not_grouped_when_all_self_coordinating() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("Family Room", Some("Family Room")),
            make_speaker("cthulhu", Some("cthulhu")),
        ];
        assert!(!app.is_grouped());
    }

    #[test]
    fn test_is_not_grouped_when_coordinators_null() {
        let mut app = App::new();
        app.speakers = vec![
            make_speaker("Family Room", None),
            make_speaker("cthulhu", None),
        ];
        assert!(!app.is_grouped());
    }
}
```

**Step 2: Run to confirm failure**

```bash
cd tui && cargo test 2>&1 | grep -E "^error|FAILED|test result"
```

Expected: compile error — `is_grouped` not found.

**Step 3: Implement `is_grouped()` in `App`**

Add after `cycle_panel` in `tui/src/app.rs`:

```rust
    pub fn is_grouped(&self) -> bool {
        // A speaker is a group follower when its coordinator differs from its own name.
        // If any follower exists, speakers are grouped.
        self.speakers.iter().any(|s| {
            s.group_coordinator
                .as_deref()
                .map(|coord| coord != s.name)
                .unwrap_or(false)
        })
    }
```

**Step 4: Run tests**

```bash
cargo test 2>&1 | grep -E "^error|FAILED|test result"
```

Expected: `test result: ok. 3 passed`

**Step 5: Commit**

```bash
cd ..
git add tui/src/app.rs
git commit -m "feat: add is_grouped() to App with tests"
```

---

### Task 4: Wire `g` keybinding and update help bar

**Files:**
- Modify: `tui/src/main.rs`
- Modify: `tui/src/ui.rs`

**Step 1: Add `g` to `handle_key` in `tui/src/main.rs`**

After the `KeyCode::Char('p')` arm (around line 123), add:

```rust
        KeyCode::Char('g') => {
            if app.is_grouped() {
                let _ = client.ungroup_all().await;
            } else {
                let _ = client.group_all().await;
            }
        }
```

**Step 2: Add `g` to the help bar in `tui/src/ui.rs`**

In `draw_help_bar`, the current help `Line::from` ends with:
```rust
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(DIM)),
```

Replace that closing pair with:
```rust
        Span::styled("g", Style::default().fg(ACCENT)),
        Span::styled(" group  ", Style::default().fg(DIM)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(DIM)),
```

**Step 3: Build release binary**

```bash
cd tui && cargo build --release 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished`

**Step 4: Run all tests**

```bash
cargo test 2>&1 | grep -E "test result"
```

Expected: `test result: ok. 3 passed`

**Step 5: Commit**

```bash
cd ..
git add tui/src/main.rs tui/src/ui.rs
git commit -m "feat: add g keybinding to toggle speaker grouping"
```

---

## Manual verification

With daemon running:

```bash
./tui/target/release/sonos-tui
```

- Press `g` — both speakers should group (cthulhu joins Family Room). Check the `group_coordinator` field updates in the Speakers panel on next refresh.
- Press `g` again — speakers should ungroup.
- Help bar should show `g group` alongside other bindings.
