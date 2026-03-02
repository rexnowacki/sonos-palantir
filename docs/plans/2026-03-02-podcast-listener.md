# Podcast Listener Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add podcast subscription and playback to sonos-palantir — RSS feeds in config.yaml, SQLite episode tracking, skip controls, and source-toggle TUI panel.

**Architecture:** Daemon-side podcast engine. New `podcast.py` module handles RSS fetching via `feedparser`, episode caching in SQLite via `aiosqlite`, and progress persistence. New FastAPI endpoints expose podcasts/episodes to the TUI. TUI gains a source toggle (`s` key) that swaps the left-bottom panel between Playlists and Podcasts, with two-level drill-down into episodes.

**Tech Stack:** Python (`feedparser`, `aiosqlite`, SQLite3), Rust (existing `ratatui`/`reqwest`/`serde` stack — no new crates)

---

### Task 1: Daemon — Podcast Manager Module (SQLite + RSS)

**Files:**
- Create: `daemon/sonosd/podcast.py`
- Create: `daemon/tests/test_podcast.py`
- Modify: `daemon/pyproject.toml` (add feedparser + aiosqlite deps)

**Step 1: Add dependencies to pyproject.toml**

In `daemon/pyproject.toml`, add `feedparser` and `aiosqlite` to the `dependencies` list:

```toml
dependencies = [
    "soco>=0.30",
    "fastapi>=0.110",
    "uvicorn>=0.29",
    "pyyaml>=6.0",
    "feedparser>=6.0",
    "aiosqlite>=0.20",
]
```

**Step 2: Install updated dependencies**

Run: `cd daemon && pip install -e .`
Expected: Successfully installed feedparser and aiosqlite

**Step 3: Write the test file**

Create `daemon/tests/test_podcast.py`:

```python
import pytest
import asyncio
import aiosqlite
from pathlib import Path
from sonosd.podcast import PodcastManager

TEST_DB = "/tmp/test_podcasts.db"


@pytest.fixture
def pm():
    """Create a PodcastManager with a temp DB, clean up after."""
    p = Path(TEST_DB)
    p.unlink(missing_ok=True)
    manager = PodcastManager(
        podcasts={"testpod": "https://example.com/feed.xml"},
        db_path=str(p),
        skip_forward=30,
        skip_back=10,
    )
    asyncio.get_event_loop().run_until_complete(manager.init_db())
    yield manager
    p.unlink(missing_ok=True)


def test_init_creates_table(pm):
    async def check():
        async with aiosqlite.connect(TEST_DB) as db:
            cursor = await db.execute(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='episodes'"
            )
            row = await cursor.fetchone()
            assert row is not None
    asyncio.get_event_loop().run_until_complete(check())


def test_upsert_and_list_episodes(pm):
    async def check():
        await pm.upsert_episodes("testpod", [
            {
                "id": "guid-1",
                "title": "Episode 1",
                "url": "https://example.com/ep1.mp3",
                "published": "2026-03-01T00:00:00",
                "duration": 3600,
            },
            {
                "id": "guid-2",
                "title": "Episode 2",
                "url": "https://example.com/ep2.mp3",
                "published": "2026-03-02T00:00:00",
                "duration": 1800,
            },
        ])
        episodes = await pm.list_episodes("testpod")
        assert len(episodes) == 2
        assert episodes[0]["title"] == "Episode 2"  # newest first
        assert episodes[0]["position"] == 0
        assert episodes[0]["played"] == 0
    asyncio.get_event_loop().run_until_complete(check())


def test_save_and_load_progress(pm):
    async def check():
        await pm.upsert_episodes("testpod", [
            {
                "id": "guid-1",
                "title": "Episode 1",
                "url": "https://example.com/ep1.mp3",
                "published": "2026-03-01T00:00:00",
                "duration": 3600,
            },
        ])
        await pm.save_progress("guid-1", position=900, played=False)
        episodes = await pm.list_episodes("testpod")
        assert episodes[0]["position"] == 900
        assert episodes[0]["played"] == 0

        await pm.save_progress("guid-1", position=3550, played=True)
        episodes = await pm.list_episodes("testpod")
        assert episodes[0]["played"] == 1
    asyncio.get_event_loop().run_until_complete(check())


def test_list_podcasts_with_counts(pm):
    async def check():
        await pm.upsert_episodes("testpod", [
            {"id": "g1", "title": "Ep 1", "url": "u1", "published": "2026-03-01", "duration": 100},
            {"id": "g2", "title": "Ep 2", "url": "u2", "published": "2026-03-02", "duration": 200},
        ])
        await pm.save_progress("g1", position=90, played=True)
        podcasts = await pm.list_podcasts()
        assert len(podcasts) == 1
        assert podcasts[0]["alias"] == "testpod"
        assert podcasts[0]["unplayed"] == 1
    asyncio.get_event_loop().run_until_complete(check())


def test_upsert_preserves_progress(pm):
    async def check():
        await pm.upsert_episodes("testpod", [
            {"id": "g1", "title": "Ep 1", "url": "u1", "published": "2026-03-01", "duration": 100},
        ])
        await pm.save_progress("g1", position=50, played=False)
        # Re-upsert same episode (simulating feed refresh)
        await pm.upsert_episodes("testpod", [
            {"id": "g1", "title": "Ep 1 (updated)", "url": "u1", "published": "2026-03-01", "duration": 100},
        ])
        episodes = await pm.list_episodes("testpod")
        assert episodes[0]["position"] == 50  # progress preserved
        assert episodes[0]["title"] == "Ep 1 (updated)"  # title updated
    asyncio.get_event_loop().run_until_complete(check())
```

**Step 4: Run tests to verify they fail**

Run: `cd daemon && source .venv/bin/activate && pytest tests/test_podcast.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'sonosd.podcast'`

**Step 5: Write the PodcastManager implementation**

Create `daemon/sonosd/podcast.py`:

```python
import aiosqlite
import feedparser
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional


class PodcastManager:
    """Manages podcast RSS feeds and episode state in SQLite."""

    def __init__(
        self,
        podcasts: dict[str, str],
        db_path: Optional[str] = None,
        skip_forward: int = 30,
        skip_back: int = 10,
        refresh_minutes: int = 30,
    ):
        self.podcasts = podcasts  # {alias: rss_url}
        self.skip_forward = skip_forward
        self.skip_back = skip_back
        self.refresh_minutes = refresh_minutes
        if db_path is None:
            home = Path.home()
            config_dir = home / ".config" / "sonos-palantir"
            config_dir.mkdir(parents=True, exist_ok=True)
            db_path = str(config_dir / "podcasts.db")
        self.db_path = db_path
        self._feed_cache: dict[str, str] = {}  # alias -> feed title

    async def init_db(self) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("""
                CREATE TABLE IF NOT EXISTS episodes (
                    id TEXT PRIMARY KEY,
                    podcast_alias TEXT NOT NULL,
                    title TEXT NOT NULL,
                    url TEXT NOT NULL,
                    published TEXT,
                    duration INTEGER DEFAULT 0,
                    position INTEGER DEFAULT 0,
                    played INTEGER DEFAULT 0,
                    fetched_at TEXT NOT NULL
                )
            """)
            await db.commit()

    async def upsert_episodes(self, alias: str, episodes: list[dict]) -> None:
        now = datetime.now(timezone.utc).isoformat()
        async with aiosqlite.connect(self.db_path) as db:
            for ep in episodes:
                await db.execute("""
                    INSERT INTO episodes (id, podcast_alias, title, url, published, duration, position, played, fetched_at)
                    VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?)
                    ON CONFLICT(id) DO UPDATE SET
                        title = excluded.title,
                        url = excluded.url,
                        published = excluded.published,
                        duration = excluded.duration,
                        fetched_at = excluded.fetched_at
                """, (
                    ep["id"], alias, ep["title"], ep["url"],
                    ep.get("published", ""), ep.get("duration", 0), now,
                ))
            await db.commit()

    async def list_episodes(self, alias: str) -> list[dict]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute(
                "SELECT * FROM episodes WHERE podcast_alias = ? ORDER BY published DESC LIMIT 20",
                (alias,),
            )
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    async def list_podcasts(self) -> list[dict]:
        result = []
        async with aiosqlite.connect(self.db_path) as db:
            for alias, url in self.podcasts.items():
                cursor = await db.execute(
                    "SELECT COUNT(*) FROM episodes WHERE podcast_alias = ? AND played = 0",
                    (alias,),
                )
                row = await cursor.fetchone()
                unplayed = row[0] if row else 0
                result.append({
                    "alias": alias,
                    "name": self._feed_cache.get(alias, alias),
                    "url": url,
                    "unplayed": unplayed,
                })
        return result

    async def save_progress(self, episode_id: str, position: int, played: bool) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute(
                "UPDATE episodes SET position = ?, played = ? WHERE id = ?",
                (position, 1 if played else 0, episode_id),
            )
            await db.commit()

    async def get_episode(self, episode_id: str) -> Optional[dict]:
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute("SELECT * FROM episodes WHERE id = ?", (episode_id,))
            row = await cursor.fetchone()
            return dict(row) if row else None

    def fetch_feed(self, alias: str) -> list[dict]:
        """Fetch and parse RSS feed, returning episode dicts. Synchronous (run in thread)."""
        url = self.podcasts.get(alias)
        if not url:
            return []
        feed = feedparser.parse(url)
        self._feed_cache[alias] = feed.feed.get("title", alias)
        episodes = []
        for entry in feed.entries[:20]:
            audio_url = ""
            for link in entry.get("links", []):
                if link.get("type", "").startswith("audio/") or link.get("rel") == "enclosure":
                    audio_url = link.get("href", "")
                    break
            for enc in entry.get("enclosures", []):
                if enc.get("type", "").startswith("audio/"):
                    audio_url = enc.get("href", "")
                    break
            if not audio_url:
                continue
            # Parse duration from itunes:duration (H:MM:SS or MM:SS or seconds)
            duration = _parse_podcast_duration(entry.get("itunes_duration", "0"))
            episodes.append({
                "id": entry.get("id", entry.get("link", audio_url)),
                "title": entry.get("title", "Untitled"),
                "url": audio_url,
                "published": entry.get("published", ""),
                "duration": duration,
            })
        return episodes

    async def refresh_all_feeds(self) -> None:
        """Fetch all RSS feeds and upsert episodes. Runs feed parsing in threads."""
        import asyncio
        loop = asyncio.get_event_loop()
        for alias in self.podcasts:
            episodes = await loop.run_in_executor(None, self.fetch_feed, alias)
            if episodes:
                await self.upsert_episodes(alias, episodes)

    def start_background_refresh(self) -> None:
        """Start a daemon thread that refreshes feeds periodically."""
        import asyncio
        def _refresh_loop():
            loop = asyncio.new_event_loop()
            while True:
                try:
                    loop.run_until_complete(self.refresh_all_feeds())
                except Exception:
                    pass
                time.sleep(self.refresh_minutes * 60)
        t = threading.Thread(target=_refresh_loop, daemon=True)
        t.start()


def _parse_podcast_duration(value: str) -> int:
    """Parse itunes:duration — can be seconds, MM:SS, or H:MM:SS."""
    if not value:
        return 0
    try:
        return int(value)
    except ValueError:
        pass
    parts = value.split(":")
    try:
        if len(parts) == 2:
            return int(parts[0]) * 60 + int(parts[1])
        if len(parts) == 3:
            return int(parts[0]) * 3600 + int(parts[1]) * 60 + int(parts[2])
    except ValueError:
        pass
    return 0
```

**Step 6: Run tests to verify they pass**

Run: `cd daemon && pytest tests/test_podcast.py -v`
Expected: 5 passed

**Step 7: Run all daemon tests**

Run: `cd daemon && pytest -q`
Expected: All tests pass (existing + new)

**Step 8: Commit**

```bash
git add daemon/sonosd/podcast.py daemon/tests/test_podcast.py daemon/pyproject.toml
git commit -m "feat: the palantir learns to listen — podcast manager with SQLite episodes"
```

---

### Task 2: Daemon — Podcast API Endpoints

**Files:**
- Modify: `daemon/sonosd/server.py` (add 7 new endpoints)
- Modify: `daemon/tests/test_server.py` (add endpoint tests)

**Step 1: Write the tests**

Add to `daemon/tests/test_server.py` — new helper and tests. First, update `_make_client()` to also create a mock `PodcastManager`, and add it to `server_module`. Then add tests for each endpoint.

Add these imports at the top of `test_server.py`:

```python
import asyncio
```

Add these test functions at the bottom of `test_server.py`:

```python
def _make_podcast_client():
    """Build a TestClient with mocked SonosManager + real PodcastManager (temp DB)."""
    import tempfile, os
    from sonosd.podcast import PodcastManager

    client, mock_manager, mock_speaker = _make_client()

    db_path = os.path.join(tempfile.mkdtemp(), "test.db")
    pm = PodcastManager(
        podcasts={"testpod": "https://example.com/feed.xml"},
        db_path=db_path,
        skip_forward=30,
        skip_back=10,
    )
    asyncio.get_event_loop().run_until_complete(pm.init_db())
    asyncio.get_event_loop().run_until_complete(pm.upsert_episodes("testpod", [
        {"id": "g1", "title": "Episode 1", "url": "https://example.com/ep1.mp3",
         "published": "2026-03-01T00:00:00", "duration": 3600},
        {"id": "g2", "title": "Episode 2", "url": "https://example.com/ep2.mp3",
         "published": "2026-03-02T00:00:00", "duration": 1800},
    ]))

    import sonosd.server as server_module
    server_module.podcast_manager = pm

    return client, mock_manager, mock_speaker, pm


def test_get_podcasts():
    client, _, _, _ = _make_podcast_client()
    resp = client.get("/podcasts")
    assert resp.status_code == 200
    data = resp.json()
    assert len(data["podcasts"]) == 1
    assert data["podcasts"][0]["alias"] == "testpod"
    assert data["podcasts"][0]["unplayed"] == 2


def test_get_podcast_episodes():
    client, _, _, _ = _make_podcast_client()
    resp = client.get("/podcasts/testpod/episodes")
    assert resp.status_code == 200
    data = resp.json()
    assert len(data["episodes"]) == 2
    assert data["episodes"][0]["title"] == "Episode 2"  # newest first


def test_get_podcast_episodes_unknown_returns_empty():
    client, _, _, _ = _make_podcast_client()
    resp = client.get("/podcasts/unknown/episodes")
    assert resp.status_code == 200
    assert len(resp.json()["episodes"]) == 0


def test_play_uri():
    client, _, mock_speaker, _ = _make_podcast_client()
    resp = client.post("/play_uri", json={
        "speaker": "cthulhu",
        "uri": "https://example.com/ep1.mp3",
        "title": "Episode 1",
    })
    assert resp.status_code == 200
    mock_speaker.play_uri.assert_called_once()


def test_skip_forward():
    client, _, mock_speaker, _ = _make_podcast_client()
    mock_speaker.get_current_track_info.return_value = {
        "position": "0:05:00",
        "duration": "1:00:00",
    }
    resp = client.post("/skip", json={"speaker": "cthulhu", "seconds": 30})
    assert resp.status_code == 200
    mock_speaker.seek.assert_called_once()


def test_skip_backward_clamps_to_zero():
    client, _, mock_speaker, _ = _make_podcast_client()
    mock_speaker.get_current_track_info.return_value = {
        "position": "0:00:05",
        "duration": "1:00:00",
    }
    resp = client.post("/skip", json={"speaker": "cthulhu", "seconds": -30})
    assert resp.status_code == 200
    # Should seek to 0:00:00, not negative
    mock_speaker.seek.assert_called_once_with("0:00:00")


def test_seek_absolute():
    client, _, mock_speaker, _ = _make_podcast_client()
    resp = client.post("/seek", json={"speaker": "cthulhu", "position": 300})
    assert resp.status_code == 200
    mock_speaker.seek.assert_called_once_with("0:05:00")


def test_save_episode_progress():
    client, _, _, pm = _make_podcast_client()
    resp = client.post("/podcasts/episode/progress", json={
        "episode_id": "g1",
        "position": 1200,
        "played": False,
    })
    assert resp.status_code == 200
    ep = asyncio.get_event_loop().run_until_complete(pm.get_episode("g1"))
    assert ep["position"] == 1200


def test_podcast_refresh():
    client, _, _, pm = _make_podcast_client()
    # Just verify endpoint exists and returns 200 (actual RSS fetch is mocked/skipped)
    resp = client.post("/podcasts/refresh")
    assert resp.status_code == 200
```

**Step 2: Run tests to verify they fail**

Run: `cd daemon && pytest tests/test_server.py -v -k "podcast or play_uri or skip or seek"`
Expected: FAIL — endpoints don't exist yet

**Step 3: Add request models and endpoints to server.py**

Add these request models after the existing models in `daemon/sonosd/server.py`:

```python
class PlayUriRequest(BaseModel):
    speaker: str
    uri: str
    title: str = ""


class SkipRequest(BaseModel):
    speaker: str
    seconds: int


class SeekRequest(BaseModel):
    speaker: str
    position: int


class EpisodeProgressRequest(BaseModel):
    episode_id: str
    position: int
    played: bool = False
```

Add this global after `manager: SonosManager = None`:

```python
podcast_manager = None  # PodcastManager, set on startup
```

Update the `startup()` function to also initialize the PodcastManager:

```python
@app.on_event("startup")
async def startup():
    global manager, podcast_manager
    config_path = Path(__file__).parent.parent / "config.yaml"
    with open(config_path) as f:
        config = yaml.safe_load(f)
    manager = SonosManager(config)

    from .podcast import PodcastManager
    podcasts = config.get("podcasts", {})
    podcast_manager = PodcastManager(
        podcasts=podcasts,
        skip_forward=config.get("podcast_skip_forward", 30),
        skip_back=config.get("podcast_skip_back", 10),
        refresh_minutes=config.get("podcast_refresh_minutes", 30),
    )
    await podcast_manager.init_db()
    if podcasts:
        await podcast_manager.refresh_all_feeds()
        podcast_manager.start_background_refresh()
```

Note: the startup function changes from `def startup()` to `async def startup()` to support `await`.

Add the new endpoints after the existing ones:

```python
@app.get("/podcasts")
async def get_podcasts():
    if podcast_manager is None:
        return {"podcasts": []}
    podcasts = await podcast_manager.list_podcasts()
    return {"podcasts": podcasts}


@app.get("/podcasts/{alias}/episodes")
async def get_podcast_episodes(alias: str):
    if podcast_manager is None:
        return {"episodes": []}
    episodes = await podcast_manager.list_episodes(alias)
    return {"episodes": episodes}


@app.post("/play_uri")
def play_uri(req: PlayUriRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        speaker.play_uri(req.uri, title=req.title)
        return {"status": "playing", "uri": req.uri}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/skip")
def skip(req: SkipRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        track_info = speaker.get_current_track_info()
        from .sonos import _parse_duration
        current = _parse_duration(track_info.get("position", "0:00:00"))
        duration = _parse_duration(track_info.get("duration", "0:00:00"))
        target = max(0, min(current + req.seconds, duration))
        h = target // 3600
        m = (target % 3600) // 60
        s = target % 60
        speaker.seek(f"{h}:{m:02}:{s:02}")
        return {"status": "ok", "position": target}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/seek")
def seek(req: SeekRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        pos = max(0, req.position)
        h = pos // 3600
        m = (pos % 3600) // 60
        s = pos % 60
        speaker.seek(f"{h}:{m:02}:{s:02}")
        return {"status": "ok", "position": pos}
    except KeyError as e:
        raise HTTPException(404, str(e))


@app.post("/podcasts/episode/progress")
async def save_episode_progress(req: EpisodeProgressRequest):
    if podcast_manager is None:
        raise HTTPException(503, "Podcast manager not initialized")
    await podcast_manager.save_progress(req.episode_id, req.position, req.played)
    return {"status": "saved"}


@app.post("/podcasts/refresh")
async def refresh_podcasts():
    if podcast_manager is None:
        raise HTTPException(503, "Podcast manager not initialized")
    await podcast_manager.refresh_all_feeds()
    return {"status": "refreshed"}
```

Also update the `reload_config` endpoint to refresh podcast config:

```python
@app.post("/reload")
async def reload_config():
    manager.reload_config()
    if podcast_manager is not None:
        podcast_manager.podcasts = manager.config.get("podcasts", {})
        podcast_manager.skip_forward = manager.config.get("podcast_skip_forward", 30)
        podcast_manager.skip_back = manager.config.get("podcast_skip_back", 10)
    return {"status": "reloaded"}
```

**Step 4: Run the new tests**

Run: `cd daemon && pytest tests/test_server.py -v`
Expected: All tests pass (existing + new)

**Step 5: Run all daemon tests**

Run: `cd daemon && pytest -q`
Expected: All tests pass

**Step 6: Commit**

```bash
git add daemon/sonosd/server.py daemon/tests/test_server.py
git commit -m "feat: voices carried on the wind — podcast API endpoints for play_uri, skip, seek, episodes"
```

---

### Task 3: Daemon — Source Detection for Podcasts

**Files:**
- Modify: `daemon/sonosd/sonos.py:205-228` (update `_detect_source`)
- Modify: `daemon/tests/test_detect_source.py` (add podcast test)
- Modify: `daemon/sonosd/server.py` (track podcast URIs)

**Step 1: Write the test**

Add to `daemon/tests/test_detect_source.py`:

```python
def test_podcast():
    assert _detect_source("x-sonos-podcast:https://example.com/ep1.mp3") == "Podcast"
```

**Step 2: Run test to verify it fails**

Run: `cd daemon && pytest tests/test_detect_source.py::test_podcast -v`
Expected: FAIL — returns "" not "Podcast"

**Step 3: Update _detect_source and play_uri endpoint**

The podcast source detection needs a way to know the URI came from the podcast system. The simplest approach: when `play_uri` is called, the daemon stores the URI in a set. `_detect_source` checks that set.

In `daemon/sonosd/sonos.py`, add a module-level set and update `_detect_source`:

Add before `_detect_source`:
```python
_podcast_uris: set[str] = set()
```

Add as the first check inside `_detect_source` (after the empty check):
```python
if uri in _podcast_uris or "x-sonos-podcast" in uri_lower:
    return "Podcast"
```

In `daemon/sonosd/server.py`, update the `play_uri` endpoint to register the URI:

```python
@app.post("/play_uri")
def play_uri(req: PlayUriRequest):
    try:
        speaker = manager.get_coordinator(req.speaker)
        from .sonos import _podcast_uris
        _podcast_uris.add(req.uri)
        speaker.play_uri(req.uri, title=req.title)
        return {"status": "playing", "uri": req.uri}
    except KeyError as e:
        raise HTTPException(404, str(e))
```

**Step 4: Run the test**

Run: `cd daemon && pytest tests/test_detect_source.py -v`
Expected: All pass including `test_podcast`

**Step 5: Run all daemon tests**

Run: `cd daemon && pytest -q`
Expected: All pass

**Step 6: Commit**

```bash
git add daemon/sonosd/sonos.py daemon/sonosd/server.py daemon/tests/test_detect_source.py
git commit -m "feat: the palantir discerns the spoken word — podcast source detection"
```

---

### Task 4: Daemon — Config + Skip Interval Support

**Files:**
- Modify: `daemon/sonosd/server.py` (expose skip config)
- Modify: `daemon/tests/test_server.py` (test config endpoint)

**Step 1: Write the test**

Add to `daemon/tests/test_server.py`:

```python
def test_get_config_returns_podcast_skip():
    client, _, _, _ = _make_podcast_client()
    resp = client.get("/config")
    assert resp.status_code == 200
    data = resp.json()
    assert data["podcast_skip_forward"] == 30
    assert data["podcast_skip_back"] == 10
```

**Step 2: Run test to verify it fails**

Run: `cd daemon && pytest tests/test_server.py::test_get_config_returns_podcast_skip -v`
Expected: FAIL — keys not in response

**Step 3: Update the /config endpoint**

In `daemon/sonosd/server.py`, update `get_config()`:

```python
@app.get("/config")
def get_config():
    raw = manager.config.get("playlist_sort", "alphabetical")
    sort = raw if raw in ("alphabetical", "popularity") else "alphabetical"
    skip_fwd = 30
    skip_back = 10
    if podcast_manager is not None:
        skip_fwd = podcast_manager.skip_forward
        skip_back = podcast_manager.skip_back
    return {
        "playlist_sort": sort,
        "podcast_skip_forward": skip_fwd,
        "podcast_skip_back": skip_back,
    }
```

**Step 4: Run tests**

Run: `cd daemon && pytest tests/test_server.py -v`
Expected: All pass

**Step 5: Commit**

```bash
git add daemon/sonosd/server.py daemon/tests/test_server.py
git commit -m "feat: the wizard's stride measured — skip intervals in config"
```

---

### Task 5: TUI — API Client + Data Structs for Podcasts

**Files:**
- Modify: `tui/src/api.rs` (add Podcast/Episode structs, new API methods)

**Step 1: Add Podcast and Episode structs**

Add after the existing `Playlist` struct in `tui/src/api.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Podcast {
    pub alias: String,
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub unplayed: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Episode {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub position: u64,
    #[serde(default)]
    pub played: u8,
}
```

Add new request structs:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct PlayUriRequest {
    pub speaker: String,
    pub uri: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkipRequest {
    pub speaker: String,
    pub seconds: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeekRequest {
    pub speaker: String,
    pub position: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeProgressRequest {
    pub episode_id: String,
    pub position: u64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub played: bool,
}
```

Add new methods to `ApiClient`:

```rust
pub async fn get_podcasts(&self) -> anyhow::Result<Vec<Podcast>> {
    let resp: serde_json::Value = self.client
        .get(format!("{}/podcasts", self.base_url))
        .send().await?
        .json().await?;
    let podcasts: Vec<Podcast> = serde_json::from_value(resp["podcasts"].clone())?;
    Ok(podcasts)
}

pub async fn get_episodes(&self, alias: &str) -> anyhow::Result<Vec<Episode>> {
    let resp: serde_json::Value = self.client
        .get(format!("{}/podcasts/{}/episodes", self.base_url, alias))
        .send().await?
        .json().await?;
    let episodes: Vec<Episode> = serde_json::from_value(resp["episodes"].clone())?;
    Ok(episodes)
}

pub async fn play_uri(&self, speaker: &str, uri: &str, title: &str) -> anyhow::Result<()> {
    self.client.post(format!("{}/play_uri", self.base_url))
        .json(&PlayUriRequest {
            speaker: speaker.to_string(),
            uri: uri.to_string(),
            title: title.to_string(),
        })
        .send().await?;
    Ok(())
}

pub async fn skip(&self, speaker: &str, seconds: i32) -> anyhow::Result<()> {
    self.client.post(format!("{}/skip", self.base_url))
        .json(&SkipRequest {
            speaker: speaker.to_string(),
            seconds,
        })
        .send().await?;
    Ok(())
}

pub async fn seek(&self, speaker: &str, position: u64) -> anyhow::Result<()> {
    self.client.post(format!("{}/seek", self.base_url))
        .json(&SeekRequest {
            speaker: speaker.to_string(),
            position,
        })
        .send().await?;
    Ok(())
}

pub async fn save_episode_progress(
    &self,
    episode_id: &str,
    position: u64,
    played: bool,
) -> anyhow::Result<()> {
    self.client.post(format!("{}/podcasts/episode/progress", self.base_url))
        .json(&EpisodeProgressRequest {
            episode_id: episode_id.to_string(),
            position,
            played,
        })
        .send().await?;
    Ok(())
}

pub async fn refresh_podcasts(&self) -> anyhow::Result<()> {
    self.client.post(format!("{}/podcasts/refresh", self.base_url))
        .send().await?;
    Ok(())
}

pub async fn get_skip_config(&self) -> anyhow::Result<(i32, i32)> {
    let resp: serde_json::Value = self.client
        .get(format!("{}/config", self.base_url))
        .send().await?
        .json().await?;
    let fwd = resp["podcast_skip_forward"].as_i64().unwrap_or(30) as i32;
    let back = resp["podcast_skip_back"].as_i64().unwrap_or(10) as i32;
    Ok((fwd, back))
}
```

**Step 2: Verify it compiles**

Run: `cd tui && cargo build 2>&1 | head -20`
Expected: Compiles (warnings about unused structs are fine)

**Step 3: Commit**

```bash
git add tui/src/api.rs
git commit -m "feat: the palantir extends its sight — podcast API structs and client methods"
```

---

### Task 6: TUI — App State for Source Toggle + Podcast Navigation

**Files:**
- Modify: `tui/src/app.rs` (add SourceMode, podcast state, navigation methods)

**Step 1: Add SourceMode enum and podcast fields to App**

Add `SourceMode` enum after the `Panel` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SourceMode {
    Playlists,
    Podcasts,
}
```

Add these fields to the `App` struct (after `help_open`):

```rust
pub source_mode: SourceMode,
pub podcasts: Vec<crate::api::Podcast>,
pub podcast_index: usize,
pub episodes: Vec<crate::api::Episode>,
pub episode_index: usize,
pub podcast_drill: bool,  // false = podcast list, true = episode list
pub skip_forward: i32,
pub skip_back: i32,
pub current_episode_id: Option<String>,  // currently playing episode for progress tracking
```

Update `App::new()` to initialize the new fields:

```rust
source_mode: SourceMode::Playlists,
podcasts: vec![],
podcast_index: 0,
episodes: vec![],
episode_index: 0,
podcast_drill: false,
skip_forward: 30,
skip_back: 10,
current_episode_id: None,
```

**Step 2: Update navigation methods**

Update `next_in_list` to handle podcast navigation:

```rust
pub fn next_in_list(&mut self) {
    match self.active_panel {
        Panel::Speakers => {
            if !self.speakers.is_empty() {
                self.speaker_index = (self.speaker_index + 1) % self.speakers.len();
            }
        }
        Panel::Playlists => {
            if self.source_mode == SourceMode::Podcasts {
                if self.podcast_drill {
                    if !self.episodes.is_empty() {
                        self.episode_index = (self.episode_index + 1) % self.episodes.len();
                    }
                } else {
                    if !self.podcasts.is_empty() {
                        self.podcast_index = (self.podcast_index + 1) % self.podcasts.len();
                    }
                }
            } else {
                if !self.playlists.is_empty() {
                    self.playlist_index = (self.playlist_index + 1) % self.playlists.len();
                }
            }
        }
        _ => {}
    }
}
```

Update `prev_in_list` similarly:

```rust
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
            if self.source_mode == SourceMode::Podcasts {
                if self.podcast_drill {
                    if !self.episodes.is_empty() {
                        self.episode_index = self.episode_index
                            .checked_sub(1)
                            .unwrap_or(self.episodes.len() - 1);
                    }
                } else {
                    if !self.podcasts.is_empty() {
                        self.podcast_index = self.podcast_index
                            .checked_sub(1)
                            .unwrap_or(self.podcasts.len() - 1);
                    }
                }
            } else {
                if !self.playlists.is_empty() {
                    self.playlist_index = self.playlist_index
                        .checked_sub(1)
                        .unwrap_or(self.playlists.len() - 1);
                }
            }
        }
        _ => {}
    }
}
```

Add toggle and helper methods:

```rust
pub fn toggle_source(&mut self) {
    self.source_mode = match self.source_mode {
        SourceMode::Playlists => SourceMode::Podcasts,
        SourceMode::Podcasts => SourceMode::Playlists,
    };
    self.podcast_drill = false;
}

pub fn selected_podcast(&self) -> Option<&crate::api::Podcast> {
    self.podcasts.get(self.podcast_index)
}

pub fn selected_episode(&self) -> Option<&crate::api::Episode> {
    self.episodes.get(self.episode_index)
}

pub fn is_podcast_playing(&self) -> bool {
    self.selected_speaker()
        .and_then(|s| s.track.as_ref())
        .map(|t| t.source == "Podcast")
        .unwrap_or(false)
}
```

**Step 2: Add tests**

Add tests to the existing `mod tests` block in `app.rs`:

```rust
#[test]
fn test_toggle_source() {
    let mut app = App::new();
    assert_eq!(app.source_mode, SourceMode::Playlists);
    app.toggle_source();
    assert_eq!(app.source_mode, SourceMode::Podcasts);
    app.toggle_source();
    assert_eq!(app.source_mode, SourceMode::Playlists);
}

#[test]
fn test_toggle_source_resets_drill() {
    let mut app = App::new();
    app.podcast_drill = true;
    app.toggle_source();
    assert!(!app.podcast_drill);
}

#[test]
fn test_podcast_navigation() {
    let mut app = App::new();
    app.source_mode = SourceMode::Podcasts;
    app.active_panel = Panel::Playlists;
    app.podcasts = vec![
        crate::api::Podcast { alias: "a".into(), name: "A".into(), url: "".into(), unplayed: 0 },
        crate::api::Podcast { alias: "b".into(), name: "B".into(), url: "".into(), unplayed: 0 },
    ];
    app.next_in_list();
    assert_eq!(app.podcast_index, 1);
    app.next_in_list();
    assert_eq!(app.podcast_index, 0); // wraps
}
```

**Step 3: Run tests**

Run: `cd tui && cargo test`
Expected: All pass

**Step 4: Commit**

```bash
git add tui/src/app.rs
git commit -m "feat: the palantir pivots — source toggle and podcast navigation in App state"
```

---

### Task 7: TUI — Key Bindings (s, f/b, arrows, Esc, Enter for podcasts)

**Files:**
- Modify: `tui/src/main.rs` (add keybindings + podcast startup fetch)
- Modify: `tui/src/command.rs` (add Source and PodcastRefresh and Mark commands)

**Step 1: Update command.rs**

Add new variants to the `Command` enum:

```rust
Source,
PodcastRefresh,
Mark,
```

Add parsing in the `match cmd` block:

```rust
"source" => Some(Command::Source),
"podcast" => {
    if rest == "refresh" {
        Some(Command::PodcastRefresh)
    } else {
        Some(Command::Unknown(input.to_string()))
    }
}
"mark" => Some(Command::Mark),
```

Add "source" and "podcast refresh" and "mark" to the autocomplete command list:

```rust
let commands = [
    "play", "vol", "group all", "ungroup", "next", "prev",
    "sleep", "reload", "source", "podcast refresh", "mark",
];
```

**Step 2: Add tests for new commands**

```rust
#[test]
fn test_parse_source() {
    assert_eq!(parse("source"), Some(Command::Source));
}

#[test]
fn test_parse_podcast_refresh() {
    assert_eq!(parse("podcast refresh"), Some(Command::PodcastRefresh));
}

#[test]
fn test_parse_mark() {
    assert_eq!(parse("mark"), Some(Command::Mark));
}

#[test]
fn test_autocomplete_source() {
    assert_eq!(autocomplete("so", &[], &[]), Some("urce".to_string()));
}
```

**Step 3: Update main.rs — startup podcast fetch**

In the `run()` function, after the existing playlist loading code (after the `get_playlist_sort` block around line 71), add:

```rust
// Load podcasts
if let Ok(podcasts) = client.get_podcasts().await {
    app.podcasts = podcasts;
}
// Load skip config
if let Ok((fwd, back)) = client.get_skip_config().await {
    app.skip_forward = fwd;
    app.skip_back = back;
}
```

**Step 4: Update handle_key — add `s` key for source toggle**

In the `match key.code` block in `handle_key()`, add before the `KeyCode::Char(':')` arm:

```rust
KeyCode::Char('s') => {
    app.toggle_source();
}
```

**Step 5: Update handle_key — add `f`/`b`/arrows for podcast skip**

Add these in the `match key.code` block:

```rust
KeyCode::Char('f') | KeyCode::Right => {
    if app.is_podcast_playing() {
        if let Some(id) = app.speaker_id() {
            let _ = client.skip(&id, app.skip_forward).await;
        }
    }
}
KeyCode::Char('b') | KeyCode::Left => {
    if app.is_podcast_playing() {
        if let Some(id) = app.speaker_id() {
            let _ = client.skip(&id, -app.skip_back).await;
        }
    }
}
```

**Step 6: Update handle_key — Enter key for podcast drill-down and episode play**

Update the `KeyCode::Enter` arm to handle podcast mode:

```rust
KeyCode::Enter => {
    if app.source_mode == crate::app::SourceMode::Podcasts && app.active_panel == crate::app::Panel::Playlists {
        if app.podcast_drill {
            // Play the selected episode
            if let (Some(speaker_id), Some(episode)) = (app.speaker_id(), app.selected_episode()) {
                let title = episode.title.clone();
                let url = episode.url.clone();
                let ep_id = episode.id.clone();
                let position = episode.position;
                let _ = client.play_uri(&speaker_id, &url, &title).await;
                if position > 0 {
                    let _ = client.seek(&speaker_id, position).await;
                }
                app.current_episode_id = Some(ep_id);
                app.set_status(format!("Playing: {}", title), 3);
            }
        } else {
            // Drill into episode list
            if let Some(podcast) = app.selected_podcast() {
                let alias = podcast.alias.clone();
                if let Ok(episodes) = client.get_episodes(&alias).await {
                    app.episodes = episodes;
                    app.episode_index = 0;
                    app.podcast_drill = true;
                }
            }
        }
    } else if let (Some(speaker_id), Some(playlist)) =
        (app.speaker_id(), app.selected_playlist())
    {
        let _ = client.play(&speaker_id, &playlist.alias).await;
        history::record_play(&playlist.alias);
        app.set_status(format!("Playing {} on {}", playlist.alias, speaker_id), 3);
    }
}
```

**Step 7: Update handle_key — Esc for podcast drill-back**

Update the `KeyCode::Esc` arm:

```rust
KeyCode::Esc => {
    if app.help_open {
        app.help_open = false;
    } else if app.podcast_drill {
        app.podcast_drill = false;
    }
}
```

**Step 8: Update handle_key — Space (pause) saves podcast progress**

Update the `KeyCode::Char(' ')` arm to save progress:

```rust
KeyCode::Char(' ') => {
    if let Some(sp) = app.selected_speaker() {
        let id = sp.alias.as_deref().unwrap_or(&sp.name).to_string();
        let is_playing = sp.state == "PLAYING";
        let position = sp.track.as_ref().map(|t| t.position).unwrap_or(0);
        match sp.state.as_str() {
            "PLAYING" => { let _ = client.pause(&id).await; }
            _ => { let _ = client.resume(&id).await; }
        }
        // Save podcast progress on pause
        if is_playing {
            if let Some(ep_id) = &app.current_episode_id {
                let _ = client.save_episode_progress(ep_id, position, false).await;
            }
        }
    }
}
```

**Step 9: Add command handling for Source, PodcastRefresh, and Mark**

In `execute_command()`, add these arms before the `Unknown` arm:

```rust
Some(Command::Source) => {
    app.toggle_source();
}
Some(Command::PodcastRefresh) => {
    let _ = client.refresh_podcasts().await;
    if let Ok(podcasts) = client.get_podcasts().await {
        app.podcasts = podcasts;
    }
    app.set_status("The distant voices are refreshed — feeds updated.", 3);
}
Some(Command::Mark) => {
    if let Some(ep) = app.selected_episode() {
        let new_played = ep.played == 0;
        let _ = client.save_episode_progress(&ep.id, ep.position, new_played).await;
        // Refresh episode list
        if let Some(podcast) = app.selected_podcast() {
            if let Ok(episodes) = client.get_episodes(&podcast.alias).await {
                app.episodes = episodes;
            }
        }
        app.set_status(
            if new_played { "Marked as heard." } else { "Marked as unheard." },
            2,
        );
    }
}
```

**Step 10: Run tests**

Run: `cd tui && cargo test`
Expected: All pass

**Step 11: Verify build**

Run: `cd tui && cargo build --release`
Expected: Clean build

**Step 12: Commit**

```bash
git add tui/src/main.rs tui/src/command.rs
git commit -m "feat: the voice of Isengard carries far — podcast keybindings, skip, play, source toggle"
```

---

### Task 8: TUI — Podcast Panel Rendering

**Files:**
- Modify: `tui/src/ui.rs` (podcast list rendering, episode rendering, help bar updates)

**Step 1: Update draw_playlists to handle source mode**

Rename `draw_playlists` to handle both modes. The function signature stays the same; it checks `app.source_mode` internally.

Replace the existing `draw_playlists` function body with:

```rust
fn draw_playlists(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Playlists;

    if app.source_mode == crate::app::SourceMode::Podcasts {
        draw_podcasts_panel(f, app, area, active);
        return;
    }

    // existing playlist rendering code (unchanged)
    let block = panel_block("Playlists", active);
    let inner_width = area.width.saturating_sub(2) as usize;

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

**Step 2: Add draw_podcasts_panel function**

Add this new function:

```rust
fn draw_podcasts_panel(f: &mut Frame, app: &App, area: Rect, active: bool) {
    if app.podcast_drill {
        // Episode list view
        let podcast_name = app.selected_podcast()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Episodes".to_string());
        let block = panel_block(&podcast_name, active);
        let inner_width = area.width.saturating_sub(2) as usize;

        let items: Vec<ListItem> = app.episodes.iter().enumerate().map(|(i, ep)| {
            let selected = i == app.episode_index;
            let style = if selected && active {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else if ep.played == 1 {
                Style::default().fg(DIM)
            } else {
                Style::default().fg(FG)
            };

            let marker = if selected { "▸" } else { " " };
            let played_marker = if ep.played == 1 { "✓" } else { " " };
            let duration_str = format_time(ep.duration);
            let title_max = inner_width.saturating_sub(12);
            let title = truncate(&ep.title, title_max);

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", marker),
                    if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) },
                ),
                Span::styled(title, style),
                Span::styled(format!(" {} ", played_marker), Style::default().fg(PLAYING)),
                Span::styled(duration_str, Style::default().fg(DIM)),
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
        if !app.episodes.is_empty() {
            state.select(Some(app.episode_index));
        }
        f.render_stateful_widget(list, area, &mut state);
    } else {
        // Podcast list view
        let block = panel_block("Podcasts", active);
        let inner_width = area.width.saturating_sub(2) as usize;

        let items: Vec<ListItem> = app.podcasts.iter().enumerate().map(|(i, pod)| {
            let selected = i == app.podcast_index;
            let style = if selected && active {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(FG)
            };

            let marker = if selected { "▸" } else { " " };
            let badge = if pod.unplayed > 0 {
                format!("●{}", pod.unplayed)
            } else {
                String::new()
            };
            let name_max = inner_width.saturating_sub(badge.len() + 6);
            let name = truncate(&pod.name, name_max);

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", marker),
                    if selected { Style::default().fg(ACCENT) } else { Style::default().fg(DIM) },
                ),
                Span::styled(name, style),
                Span::styled(format!(" {}", badge), Style::default().fg(PLAYING)),
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
        if !app.podcasts.is_empty() {
            state.select(Some(app.podcast_index));
        }
        f.render_stateful_widget(list, area, &mut state);
    }
}
```

**Step 3: Update the help bar**

In `draw_help_bar`, update the normal help line to include `s source` and conditionally show skip hints. Replace the help `Line::from(vec![...])` block:

```rust
let mut help_spans = vec![
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
    Span::styled("s", Style::default().fg(ACCENT)),
    Span::styled(" source  ", Style::default().fg(DIM)),
];

if app.is_podcast_playing() {
    help_spans.push(Span::styled("f/→", Style::default().fg(ACCENT)));
    help_spans.push(Span::styled(format!(" +{}s  ", app.skip_forward), Style::default().fg(DIM)));
    help_spans.push(Span::styled("b/←", Style::default().fg(ACCENT)));
    help_spans.push(Span::styled(format!(" -{}s  ", app.skip_back), Style::default().fg(DIM)));
} else {
    help_spans.push(Span::styled("n/p", Style::default().fg(ACCENT)));
    help_spans.push(Span::styled(" track  ", Style::default().fg(DIM)));
}

help_spans.extend([
    Span::styled(":", Style::default().fg(ACCENT)),
    Span::styled(" cmd  ", Style::default().fg(DIM)),
    Span::styled("?", Style::default().fg(ACCENT)),
    Span::styled(" help  ", Style::default().fg(DIM)),
    Span::styled("q", Style::default().fg(ACCENT)),
    Span::styled(" quit", Style::default().fg(DIM)),
]);

let help = Line::from(help_spans);
```

**Step 4: Update the help overlay**

In `draw_help_overlay`, add the podcast section after the GROUPS section:

```rust
Line::from(""),
Line::from(vec![Span::styled("  PODCASTS", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))]),
Line::from(vec![Span::styled("  s          ", Style::default().fg(ACCENT)), Span::styled("Toggle source — Playlists / Podcasts", Style::default().fg(FG))]),
Line::from(vec![Span::styled("  f / →      ", Style::default().fg(ACCENT)), Span::styled("Skip forward (when podcast playing)", Style::default().fg(FG))]),
Line::from(vec![Span::styled("  b / ←      ", Style::default().fg(ACCENT)), Span::styled("Skip back (when podcast playing)", Style::default().fg(FG))]),
Line::from(vec![Span::styled("  :mark      ", Style::default().fg(ACCENT)), Span::styled("Toggle played/unplayed on selected episode", Style::default().fg(FG))]),
```

**Step 5: Run tests**

Run: `cd tui && cargo test`
Expected: All pass

**Step 6: Verify build**

Run: `cd tui && cargo build --release`
Expected: Clean build

**Step 7: Commit**

```bash
git add tui/src/ui.rs
git commit -m "feat: the palantir reveals distant voices — podcast panel, episode list, skip hints"
```

---

### Task 9: Final Verification & README Update

**Files:**
- Modify: `README.md`

**Step 1: Run all daemon tests**

Run: `cd daemon && source .venv/bin/activate && pytest -q`
Expected: All pass

**Step 2: Run all TUI tests**

Run: `cd tui && cargo test`
Expected: All pass

**Step 3: Build release binary**

Run: `cd tui && cargo build --release`
Expected: Clean build, no warnings

**Step 4: Update README.md**

Add podcast keybindings to the keybindings table:

```markdown
| `s` | Toggle source (Playlists / Podcasts) |
| `f` / `→` | Skip forward (podcast, default 30s) |
| `b` / `←` | Skip back (podcast, default 10s) |
```

Add podcast commands to the command mode table:

```markdown
| `:source` | Toggle Playlists / Podcasts panel |
| `:podcast refresh` | Force re-fetch all podcast RSS feeds |
| `:mark` | Toggle played/unplayed on selected episode |
```

Add to the config.yaml example:

```yaml
podcasts:
  tpm: "https://feeds.example.com/thepublicmood.xml"

podcast_skip_forward: 30    # seconds, default 30
podcast_skip_back: 10       # seconds, default 10
```

Add podcast to the Features list:

```markdown
- **Podcast listener** — subscribe to RSS feeds in config.yaml, browse episodes, skip forward/back, auto-resume, progress tracking via SQLite
```

Update the Requirements section:

```markdown
- Python 3.11+ (feedparser, aiosqlite)
```

**Step 5: Commit**

```bash
git add README.md
git commit -m "docs: and thus the tale grows — README updated with podcast listener documentation"
```

**Step 6: Push**

```bash
git push origin HEAD
```
