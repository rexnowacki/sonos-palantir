import soco
from typing import Optional


def _parse_duration(time_str: str) -> int:
    """Parse 'H:MM:SS' to total seconds."""
    parts = time_str.split(":")
    if len(parts) == 3:
        try:
            return int(parts[0]) * 3600 + int(parts[1]) * 60 + int(parts[2])
        except ValueError:
            return 0
    return 0
