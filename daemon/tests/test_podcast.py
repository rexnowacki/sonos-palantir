"""Tests for the podcast manager module."""

import asyncio
import os

import pytest

from sonosd.podcast import PodcastManager, _parse_podcast_duration

DB_PATH = "/tmp/test_podcasts.db"


@pytest.fixture(autouse=True)
def clean_db():
    """Remove test DB before and after each test."""
    if os.path.exists(DB_PATH):
        os.remove(DB_PATH)
    yield
    if os.path.exists(DB_PATH):
        os.remove(DB_PATH)


def _make_manager(**kwargs):
    return PodcastManager(
        podcasts={"testpod": "https://example.com/feed.xml"},
        db_path=DB_PATH,
        **kwargs,
    )


def _run(coro):
    return asyncio.get_event_loop().run_until_complete(coro)


def test_init_creates_table():
    pm = _make_manager()
    _run(pm.init_db())

    import aiosqlite

    async def check():
        async with aiosqlite.connect(DB_PATH) as db:
            cursor = await db.execute(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='episodes'"
            )
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "episodes"

    _run(check())


def test_upsert_and_list_episodes():
    pm = _make_manager()
    _run(pm.init_db())

    episodes = [
        {"id": "ep1", "title": "Episode 1", "url": "https://example.com/ep1.mp3", "published": "2025-01-01", "duration": 600},
        {"id": "ep2", "title": "Episode 2", "url": "https://example.com/ep2.mp3", "published": "2025-06-01", "duration": 1200},
    ]
    _run(pm.upsert_episodes("testpod", episodes))

    result = _run(pm.list_episodes("testpod"))
    assert len(result) == 2
    # Newest first (2025-06-01 before 2025-01-01)
    assert result[0]["id"] == "ep2"
    assert result[1]["id"] == "ep1"


def test_save_and_load_progress():
    pm = _make_manager()
    _run(pm.init_db())

    episodes = [
        {"id": "ep1", "title": "Episode 1", "url": "https://example.com/ep1.mp3", "published": "2025-01-01", "duration": 600},
    ]
    _run(pm.upsert_episodes("testpod", episodes))

    # Save position
    _run(pm.save_progress("ep1", 120, False))
    ep = _run(pm.get_episode("ep1"))
    assert ep["position"] == 120
    assert ep["played"] == 0

    # Mark as played
    _run(pm.save_progress("ep1", 600, True))
    ep = _run(pm.get_episode("ep1"))
    assert ep["position"] == 600
    assert ep["played"] == 1


def test_list_podcasts_with_counts():
    pm = _make_manager()
    _run(pm.init_db())

    episodes = [
        {"id": "ep1", "title": "Episode 1", "url": "https://example.com/ep1.mp3", "published": "2025-01-01", "duration": 600},
        {"id": "ep2", "title": "Episode 2", "url": "https://example.com/ep2.mp3", "published": "2025-06-01", "duration": 1200},
    ]
    _run(pm.upsert_episodes("testpod", episodes))

    # Mark one as played
    _run(pm.save_progress("ep1", 600, True))

    podcasts = _run(pm.list_podcasts())
    assert len(podcasts) == 1
    assert podcasts[0]["alias"] == "testpod"
    assert podcasts[0]["unplayed"] == 1


def test_upsert_preserves_progress():
    pm = _make_manager()
    _run(pm.init_db())

    episodes = [
        {"id": "ep1", "title": "Episode 1", "url": "https://example.com/ep1.mp3", "published": "2025-01-01", "duration": 600},
    ]
    _run(pm.upsert_episodes("testpod", episodes))

    # Save progress
    _run(pm.save_progress("ep1", 300, True))

    # Re-upsert with updated title
    episodes_updated = [
        {"id": "ep1", "title": "Episode 1 (Remastered)", "url": "https://example.com/ep1.mp3", "published": "2025-01-01", "duration": 600},
    ]
    _run(pm.upsert_episodes("testpod", episodes_updated))

    # Progress should be preserved, title should be updated
    ep = _run(pm.get_episode("ep1"))
    assert ep["title"] == "Episode 1 (Remastered)"
    assert ep["position"] == 300
    assert ep["played"] == 1


def test_parse_podcast_duration():
    assert _parse_podcast_duration(None) == 0
    assert _parse_podcast_duration("") == 0
    assert _parse_podcast_duration(120) == 120
    assert _parse_podcast_duration("3600") == 3600
    assert _parse_podcast_duration("5:30") == 330
    assert _parse_podcast_duration("1:05:30") == 3930
