from sonosd.sonos import _detect_source


def test_spotify():
    assert _detect_source("x-sonos-spotify:spotify:track:abc") == "Spotify"


def test_apple():
    assert _detect_source("x-sonos-http:apple.com/stream") == "Apple Music"


def test_empty():
    assert _detect_source("") == ""


def test_local_library():
    assert _detect_source("x-file-cifs://nas/music/song.flac") == "Local Library"


def test_line_in():
    assert _detect_source("x-rincon-stream:RINCON_123") == "Line-In"


def test_unknown():
    assert _detect_source("http://example.com/stream") == ""


def test_tidal():
    assert _detect_source("x-sonos-http:tidal.com/track/123") == "Tidal"


def test_amazon():
    assert _detect_source("x-sonosapi-hls-static:amazon-music:track") == "Amazon Music"
