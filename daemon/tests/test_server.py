import asyncio
from unittest.mock import MagicMock, patch
from fastapi.testclient import TestClient


def _make_client():
    """Build a TestClient with a fully mocked SonosManager."""
    mock_speaker = MagicMock()
    mock_speaker.player_name = "cthulhu"
    mock_speaker.ip_address = "192.168.1.99"
    mock_speaker.volume = 25
    mock_speaker.mute = False
    mock_speaker.group = None

    transport = {"current_transport_state": "PLAYING"}
    track = {
        "title": "Alt Wave Track",
        "artist": "Some Artist",
        "album": "Some Album",
        "duration": "0:03:00",
        "position": "0:01:00",
        "album_art": "",
    }
    mock_speaker.get_current_transport_info.return_value = transport
    mock_speaker.get_current_track_info.return_value = track

    mock_manager = MagicMock()
    mock_manager.get_all_speakers.return_value = {"cthulhu": mock_speaker}
    mock_manager.get_speaker_info.return_value = {
        "name": "cthulhu",
        "alias": "cthulhu",
        "ip": "192.168.1.99",
        "volume": 25,
        "muted": False,
        "state": "PLAYING",
        "group_coordinator": None,
        "track": {
            "title": "Alt Wave Track",
            "artist": "Some Artist",
            "album": "Some Album",
            "duration": 180,
            "position": 60,
            "art_uri": "",
        },
    }
    mock_manager._playlist_map = {"altwave": "Alt Wave"}
    mock_manager.get_playlists_map.return_value = {"altwave": "Alt Wave"}
    mock_manager.get_speaker.return_value = mock_speaker
    mock_manager.get_coordinator.return_value = mock_speaker

    import sonosd.server as server_module
    server_module.manager = mock_manager

    from sonosd.server import app
    return TestClient(app), mock_manager, mock_speaker


def test_get_speakers_returns_list():
    client, _, _ = _make_client()
    resp = client.get("/speakers")
    assert resp.status_code == 200
    data = resp.json()
    assert "speakers" in data
    assert data["speakers"][0]["name"] == "cthulhu"


def test_get_playlists():
    client, _, _ = _make_client()
    resp = client.get("/playlists")
    assert resp.status_code == 200
    assert resp.json()["playlists"]["altwave"] == "Alt Wave"


def test_play_returns_200():
    client, mock_manager, mock_speaker = _make_client()
    resp = client.post("/play", json={"speaker": "cthulhu", "playlist": "altwave"})
    assert resp.status_code == 200
    assert resp.json()["status"] == "playing"


def test_play_unknown_speaker_returns_404():
    client, mock_manager, _ = _make_client()
    mock_manager.get_speaker.side_effect = KeyError("Speaker not found: ghost")
    resp = client.post("/play", json={"speaker": "ghost", "playlist": "altwave"})
    assert resp.status_code == 404


def test_volume_clamps_to_100():
    client, mock_manager, mock_speaker = _make_client()
    resp = client.post("/volume", json={"speaker": "cthulhu", "volume": 150})
    assert resp.status_code == 200
    assert resp.json()["volume"] == 100


def test_pause_returns_200():
    client, _, _ = _make_client()
    resp = client.post("/pause", json={"speaker": "cthulhu"})
    assert resp.status_code == 200


def test_next_track_returns_200():
    client, _, _ = _make_client()
    resp = client.post("/next", json={"speaker": "cthulhu"})
    assert resp.status_code == 200


def test_next_track_upnp_error_returns_422():
    from soco.exceptions import SoCoUPnPException
    client, mock_manager, mock_speaker = _make_client()
    mock_speaker.next.side_effect = SoCoUPnPException("UPnP Error 800", error_code=800, error_xml="")
    resp = client.post("/next", json={"speaker": "cthulhu"})
    assert resp.status_code == 422


def test_previous_track_upnp_error_returns_422():
    from soco.exceptions import SoCoUPnPException
    client, mock_manager, mock_speaker = _make_client()
    mock_speaker.previous.side_effect = SoCoUPnPException("UPnP Error 800", error_code=800, error_xml="")
    resp = client.post("/previous", json={"speaker": "cthulhu"})
    assert resp.status_code == 422


def test_get_favorites():
    client, mock_manager, mock_speaker = _make_client()
    mock_fav = MagicMock()
    mock_fav.title = "Jazz Classics"
    mock_speaker.music_library.get_sonos_favorites.return_value = [mock_fav]
    resp = client.get("/favorites")
    assert resp.status_code == 200
    data = resp.json()
    assert any(f["title"] == "Jazz Classics" for f in data["favorites"])


def test_get_config_returns_playlist_sort():
    client, _, _ = _make_client()
    resp = client.get("/config")
    assert resp.status_code == 200
    assert "playlist_sort" in resp.json()


def test_reload_endpoint_returns_200():
    client, mock_manager, _ = _make_client()
    resp = client.post("/reload")
    assert resp.status_code == 200
    assert resp.json()["status"] == "reloaded"


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
    assert data["episodes"][0]["title"] == "Episode 2"


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
    resp = client.post("/podcasts/refresh")
    assert resp.status_code == 200
