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
