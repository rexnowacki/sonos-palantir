"""Podcast feed manager with SQLite episode tracking."""

from __future__ import annotations

import asyncio
import logging
import re
import threading
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from hashlib import sha256
from pathlib import Path
from typing import Optional

import aiosqlite
import feedparser

log = logging.getLogger(__name__)

_CREATE_TABLE = """
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
);
"""

_UPSERT_EPISODE = """
INSERT INTO episodes (id, podcast_alias, title, url, published, duration, position, played, fetched_at)
VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?)
ON CONFLICT(id) DO UPDATE SET
    title = excluded.title,
    url = excluded.url,
    published = excluded.published,
    duration = excluded.duration,
    fetched_at = excluded.fetched_at;
"""


def _parse_podcast_duration(value) -> int:
    """Parse duration from seconds (int/str), MM:SS, or H:MM:SS to total seconds."""
    if value is None:
        return 0
    if isinstance(value, int):
        return value
    s = str(value).strip()
    if not s:
        return 0
    # Pure numeric (seconds)
    if re.fullmatch(r"\d+", s):
        return int(s)
    # MM:SS
    m = re.fullmatch(r"(\d+):(\d{1,2})", s)
    if m:
        return int(m.group(1)) * 60 + int(m.group(2))
    # H:MM:SS
    m = re.fullmatch(r"(\d+):(\d{1,2}):(\d{1,2})", s)
    if m:
        return int(m.group(1)) * 3600 + int(m.group(2)) * 60 + int(m.group(3))
    return 0


class PodcastManager:
    """Manages podcast feeds and episode state via SQLite."""

    def __init__(
        self,
        podcasts: dict[str, str],
        db_path: Optional[str] = None,
        skip_forward: int = 30,
        skip_back: int = 10,
        refresh_minutes: int = 30,
    ):
        self.podcasts = podcasts
        default_dir = Path.home() / ".config" / "sonos-palantir"
        self.db_path = db_path or str(default_dir / "podcasts.db")
        self.skip_forward = skip_forward
        self.skip_back = skip_back
        self.refresh_minutes = refresh_minutes
        self._feed_titles: dict[str, str] = {}
        self._refresh_thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()

    async def init_db(self) -> None:
        """Create the episodes table if it does not exist."""
        Path(self.db_path).parent.mkdir(parents=True, exist_ok=True)
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute(_CREATE_TABLE)
            await db.commit()

    async def upsert_episodes(self, alias: str, episodes: list[dict]) -> None:
        """Insert or update episodes, preserving position and played on conflict."""
        now = datetime.now(timezone.utc).isoformat()
        async with aiosqlite.connect(self.db_path) as db:
            for ep in episodes:
                ep_id = ep.get("id") or sha256(ep["url"].encode()).hexdigest()[:16]
                await db.execute(
                    _UPSERT_EPISODE,
                    (
                        ep_id,
                        alias,
                        ep["title"],
                        ep["url"],
                        ep.get("published", ""),
                        ep.get("duration", 0),
                        now,
                    ),
                )
            # Trim to 10 most recent episodes per podcast
            await db.execute(
                "DELETE FROM episodes WHERE podcast_alias = ? AND id NOT IN "
                "(SELECT id FROM episodes WHERE podcast_alias = ? ORDER BY published DESC LIMIT 10)",
                (alias, alias),
            )
            await db.commit()

    async def list_episodes(self, alias: str) -> list[dict]:
        """Return up to 10 episodes for a podcast, newest first."""
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute(
                "SELECT * FROM episodes WHERE podcast_alias = ? ORDER BY published DESC LIMIT 10",
                (alias,),
            )
            rows = await cursor.fetchall()
            return [dict(r) for r in rows]

    async def list_podcasts(self) -> list[dict]:
        """Return podcast list with unplayed episode counts."""
        result = []
        async with aiosqlite.connect(self.db_path) as db:
            for alias, url in self.podcasts.items():
                cursor = await db.execute(
                    "SELECT COUNT(*) FROM episodes WHERE podcast_alias = ? AND played = 0",
                    (alias,),
                )
                row = await cursor.fetchone()
                unplayed = row[0] if row else 0
                name = self._feed_titles.get(alias, alias)
                result.append({"alias": alias, "name": name, "url": url, "unplayed": unplayed})
        return result

    async def save_progress(self, episode_id: str, position: int, played: bool) -> None:
        """Update position and played flag for an episode."""
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute(
                "UPDATE episodes SET position = ?, played = ? WHERE id = ?",
                (position, int(played), episode_id),
            )
            await db.commit()

    async def get_episode(self, episode_id: str) -> Optional[dict]:
        """Return a single episode by id."""
        async with aiosqlite.connect(self.db_path) as db:
            db.row_factory = aiosqlite.Row
            cursor = await db.execute(
                "SELECT * FROM episodes WHERE id = ?", (episode_id,)
            )
            row = await cursor.fetchone()
            return dict(row) if row else None

    def fetch_feed(self, alias: str) -> list[dict]:
        """Synchronously parse an RSS feed and return episode dicts."""
        url = self.podcasts.get(alias)
        if not url:
            log.warning("No URL for podcast alias %s", alias)
            return []
        feed = feedparser.parse(url)
        feed_title = getattr(feed.feed, "title", None)
        if feed_title:
            self._feed_titles[alias] = feed_title
        episodes = []
        for entry in feed.entries:
            audio_url = None
            for link in getattr(entry, "enclosures", []) or getattr(entry, "links", []):
                if link.get("type", "").startswith("audio/") or link.get("href", "").endswith(".mp3"):
                    audio_url = link.get("href")
                    break
            if not audio_url:
                continue
            duration_raw = getattr(entry, "itunes_duration", None)
            published_raw = getattr(entry, "published", "") or ""
            try:
                published = parsedate_to_datetime(published_raw).isoformat()
            except Exception:
                published = published_raw
            ep_id = sha256(audio_url.encode()).hexdigest()[:16]
            episodes.append(
                {
                    "id": ep_id,
                    "title": entry.get("title", "Untitled"),
                    "url": audio_url,
                    "published": published,
                    "duration": _parse_podcast_duration(duration_raw),
                }
            )
        return episodes

    async def refresh_all_feeds(self) -> None:
        """Fetch all podcast feeds (in executor) and upsert episodes."""
        loop = asyncio.get_event_loop()
        for alias in self.podcasts:
            try:
                episodes = await loop.run_in_executor(None, self.fetch_feed, alias)
                if episodes:
                    await self.upsert_episodes(alias, episodes)
                    log.info("Refreshed %s: %d episodes", alias, len(episodes))
            except Exception:
                log.exception("Failed to refresh feed: %s", alias)

    def start_background_refresh(self) -> None:
        """Start a daemon thread that refreshes feeds periodically."""
        if self._refresh_thread and self._refresh_thread.is_alive():
            return
        self._stop_event.clear()

        def _run():
            while not self._stop_event.wait(self.refresh_minutes * 60):
                try:
                    loop = asyncio.new_event_loop()
                    loop.run_until_complete(self.refresh_all_feeds())
                    loop.close()
                except Exception:
                    log.exception("Background refresh failed")

        self._refresh_thread = threading.Thread(target=_run, daemon=True)
        self._refresh_thread.start()
        log.info("Background podcast refresh started (every %d min)", self.refresh_minutes)
