from sonosd.sonos import _parse_duration
from unittest.mock import MagicMock, patch


def test_parse_duration_full():
    assert _parse_duration("0:02:43") == 163


def test_parse_duration_hours():
    assert _parse_duration("1:00:00") == 3600


def test_parse_duration_zero():
    assert _parse_duration("0:00:00") == 0


def test_parse_duration_invalid_returns_zero():
    assert _parse_duration("") == 0


def _make_manager(config=None):
    """Build a SonosManager with mocked soco.discover."""
    if config is None:
        config = {
            "speakers": {"cthulhu": "cthulhu"},
            "playlists": {"altwave": "Alt Wave"},
        }

    mock_speaker = MagicMock()
    mock_speaker.player_name = "cthulhu"
    mock_speaker.ip_address = "192.168.1.99"
    mock_speaker.volume = 25
    mock_speaker.mute = False

    with patch("soco.discover", return_value={mock_speaker}):
        from sonosd.sonos import SonosManager
        manager = SonosManager(config)

    return manager, mock_speaker


def test_get_speaker_by_alias():
    manager, mock_speaker = _make_manager()
    result = manager.get_speaker("cthulhu")
    assert result is mock_speaker


def test_get_speaker_unknown_raises():
    manager, _ = _make_manager()
    import pytest
    with pytest.raises(KeyError):
        manager.get_speaker("nonexistent")


def test_get_all_speakers_returns_dict():
    manager, mock_speaker = _make_manager()
    all_speakers = manager.get_all_speakers()
    assert "cthulhu" in all_speakers


def test_get_coordinator_returns_self_when_not_grouped():
    manager, mock_speaker = _make_manager()
    mock_speaker.group = None
    result = manager.get_coordinator("cthulhu")
    assert result is mock_speaker


def test_get_coordinator_returns_coordinator_when_follower():
    manager, mock_follower = _make_manager()
    mock_coordinator = MagicMock()
    mock_follower.group = MagicMock()
    mock_follower.group.coordinator = mock_coordinator
    result = manager.get_coordinator("cthulhu")
    assert result is mock_coordinator


def test_play_favorite_uses_coordinator_when_speaker_is_follower():
    """play_favorite must call play_uri on the coordinator, not on a follower."""
    manager, mock_follower = _make_manager()

    # Set up a separate coordinator mock
    mock_coordinator = MagicMock()
    mock_coordinator.player_name = "Family Room"

    # cthulhu is a group member — its coordinator is Family Room
    mock_follower.group = MagicMock()
    mock_follower.group.coordinator = mock_coordinator

    # Set up favorites on the coordinator (where they should be fetched from)
    mock_fav = MagicMock()
    mock_fav.title = "Alt Wave"
    mock_fav.reference.get_uri.return_value = "x-sonos-spotify:..."
    mock_fav.resource_meta_data = "<meta/>"
    mock_coordinator.music_library.get_sonos_favorites.return_value = [mock_fav]
    mock_follower.music_library.get_sonos_favorites.return_value = [mock_fav]

    manager.play_favorite(mock_follower, "altwave")

    # play_uri must be called on the coordinator, not the follower
    mock_coordinator.play_uri.assert_called_once()
    mock_follower.play_uri.assert_not_called()


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
    assert info["group_coordinator"] == "Family Room"
    assert info["name"] == "cthulhu"


def test_get_speaker_info_coordinator_does_not_double_fetch():
    """A coordinator must not call get_current_track_info on itself twice."""
    manager, mock_speaker = _make_manager()

    # The speaker is its own coordinator
    mock_speaker.group = MagicMock()
    mock_speaker.group.coordinator = mock_speaker

    mock_speaker.get_current_transport_info.return_value = {
        "current_transport_state": "PLAYING"
    }
    mock_speaker.get_current_track_info.return_value = {
        "title": "Coordinator Track",
        "artist": "Some Artist",
        "album": "Some Album",
        "duration": "0:03:00",
        "position": "0:01:00",
        "album_art": "",
    }

    info = manager.get_speaker_info(mock_speaker)

    mock_speaker.get_current_track_info.assert_called_once()
    assert info["track"]["title"] == "Coordinator Track"
