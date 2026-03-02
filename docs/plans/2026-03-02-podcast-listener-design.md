# Podcast Listener Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement the corresponding implementation plan task-by-task.

**Goal:** Add podcast subscription and playback to sonos-palantir, with episode management, resume support, and skip controls — all routing audio through Sonos speakers.

**Approach:** Daemon-side podcast engine. The daemon handles RSS fetching, episode caching in SQLite, and progress tracking. TUI stays thin — just displays state and sends commands.

---

## 1. Config & Data Model

### config.yaml additions

```yaml
podcasts:
  tpm: "https://feeds.example.com/thepublicmood.xml"
  ycombinator: "https://feeds.example.com/yc.xml"

podcast_skip_forward: 30    # seconds, default 30
podcast_skip_back: 10       # seconds, default 10
podcast_refresh_minutes: 30 # feed refresh interval, default 30
```

Aliases follow the same pattern as playlists — short name maps to RSS feed URL.

### SQLite schema (`~/.config/sonos-palantir/podcasts.db`)

```sql
CREATE TABLE episodes (
    id TEXT PRIMARY KEY,          -- guid from RSS
    podcast_alias TEXT NOT NULL,
    title TEXT NOT NULL,
    url TEXT NOT NULL,            -- audio enclosure URL
    published TEXT,               -- ISO timestamp
    duration INTEGER,             -- seconds (from RSS or 0)
    position INTEGER DEFAULT 0,  -- resume position in seconds
    played INTEGER DEFAULT 0,    -- 0 = unplayed, 1 = played
    fetched_at TEXT NOT NULL
);
```

- Episodes fetched on daemon startup and refreshed every `podcast_refresh_minutes`
- Latest 20 episodes per podcast retained
- `feedparser` library for RSS parsing

---

## 2. Daemon API Endpoints

| Route | Method | Purpose |
|-------|--------|---------|
| `/podcasts` | GET | List subscribed podcasts with unplayed counts |
| `/podcasts/{alias}/episodes` | GET | List episodes (latest 20, with played/position state) |
| `/play_uri` | POST | Play audio URL on speaker `{speaker, uri, title?}` |
| `/seek` | POST | Absolute seek `{speaker, position}` — for resume |
| `/skip` | POST | Relative seek `{speaker, seconds}` — +30 or -10 |
| `/podcasts/episode/progress` | POST | Save resume position `{episode_id, position, played?}` |
| `/podcasts/refresh` | POST | Force re-fetch all RSS feeds |

### Key behaviors

- `GET /podcasts` returns `{podcasts: [{alias, name, unplayed, image_url}]}`
- `GET /podcasts/{alias}/episodes` returns episodes sorted newest-first: `{id, title, url, duration, position, played, published}`
- `/play_uri` uses `soco`'s `play_uri()` — same mechanism existing `play_favorite` uses as fallback
- `/skip` reads current position from `soco`, computes target, calls `seek()`, clamps to 0..duration
- Progress saved explicitly by TUI on pause; not auto-polled

---

## 3. TUI Source Toggle & Navigation

### Source toggle

- `s` key or `:source` command toggles the left-bottom panel between "Playlists" and "Podcasts"
- `App` gains `source_mode: SourceMode` enum (`Playlists` | `Podcasts`)
- Panel title changes to match the active mode
- Help bar shows `s source` alongside existing keybindings

### Two-level podcast navigation

**Level 1 — Podcast list:**
```
╭─ Podcasts ─────────╮
│ ▸ tpm           ●3 │
│   ycombinator   ●1 │
╰────────────────────╯
```
- Shows subscribed podcasts with unplayed count badge
- `j/k/↑/↓` to navigate, `Enter` to drill into episodes, `Esc` returns to podcast list

**Level 2 — Episode list:**
```
╭─ tpm ──────────────╮
│ ▸ Ep 42    58:12   │
│   Ep 41  ✓ 45:30   │
│   Ep 40  ✓ 52:11   │
╰────────────────────╯
```
- Title, duration, `✓` for played, partial progress for in-progress episodes
- `Enter` plays episode on selected speaker
- `Esc` returns to podcast list

### New App state

```
source_mode: SourceMode
podcasts: Vec<Podcast>
podcast_index: usize
episodes: Vec<Episode>
episode_index: usize
podcast_drill: bool        // false = podcast list, true = episode list
```

Tab cycling unchanged: Rooms → left-bottom panel → Now Playing.

---

## 4. Playback Controls & Skip

### Keybindings (active only when source is Podcast)

| Key | Action |
|-----|--------|
| `f` or `→` | Skip forward (default 30s) |
| `b` or `←` | Skip back (default 10s) |
| `n` | Next episode |
| `p` | Previous episode |

### Skip flow

1. TUI sends `POST /skip {speaker, seconds: 30}` (or `-10`)
2. Daemon reads current position from `soco`, computes new absolute position, calls `seek()`
3. Position clamped to `0..duration`

### Progress saving

- On pause: TUI sends current position to `/podcasts/episode/progress`
- On play: if episode has saved position > 0, TUI sends `/seek` after `/play_uri` to resume
- Auto-marked played when position reaches within 60 seconds of duration
- Manual toggle via `:mark` command

### Source detection

- `_detect_source()` gains `"Podcast"` return for URIs played via the podcast system
- TUI uses source field to show `f/b` skip hints in help bar when source is Podcast

---

## 5. Now Playing (Podcast Mode)

Same panel, podcast-specific content:
- Speaker name
- Episode title (instead of track name)
- Podcast name (instead of artist — album)
- Source: "Podcast"
- Segmented progress bar (identical to music — already has position/duration)
- Help bar adds `f +30s  b -10s` (or configured values) when source is Podcast

---

## 6. Episode Lifecycle

- **Unplayed**: No indicator
- **In progress**: Position saved, shows `23:41/58:12` style progress
- **Played**: `✓` marker — automatic when within 60s of end, or manual via `:mark`

### Feed refresh

- Background thread fetches all RSS feeds every `podcast_refresh_minutes` (default 30)
- Feeds fetched on startup
- `:podcast refresh` forces immediate re-fetch
- New episodes appear on next TUI poll

---

## 7. Not Building (YAGNI)

- No download/offline — Sonos streams directly from the audio URL
- No podcast search/discovery — manage RSS URLs in config.yaml
- No chapter support — skip forward/back covers navigation
- No playback speed — Sonos doesn't support this natively
- No episode queue — play one episode at a time
