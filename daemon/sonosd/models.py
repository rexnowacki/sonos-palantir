from pydantic import BaseModel
from typing import Optional


class TrackInfo(BaseModel):
    title: str
    artist: str
    album: str
    duration: int
    position: int
    art_uri: str


class SpeakerInfo(BaseModel):
    name: str
    alias: Optional[str]
    ip: str
    volume: int
    muted: bool
    state: str
    group_coordinator: Optional[str]
    track: Optional[TrackInfo]
