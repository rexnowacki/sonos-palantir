from sonosd.sonos import _parse_duration


def test_parse_duration_full():
    assert _parse_duration("0:02:43") == 163


def test_parse_duration_hours():
    assert _parse_duration("1:00:00") == 3600


def test_parse_duration_zero():
    assert _parse_duration("0:00:00") == 0


def test_parse_duration_invalid_returns_zero():
    assert _parse_duration("") == 0
