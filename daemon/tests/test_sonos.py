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
