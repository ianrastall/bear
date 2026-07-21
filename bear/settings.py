"""What Bear remembers between runs.

A handful of keys in one JSON file under the user's config directory. Nothing about a game
is stored — games go to PGN — only the choices a player would be annoyed to make twice.

Every read and write is best effort. A settings file that cannot be read is not a reason to
refuse to open a chess board.
"""

from __future__ import annotations

import json
import os
from dataclasses import asdict, dataclass, fields


def _path() -> str:
    root = os.environ.get("APPDATA") or os.path.expanduser("~/.config")
    return os.path.join(root, "Bear", "settings.json")


@dataclass
class Settings:
    palette: str = ""          # empty means "the default palette"
    pieces: str = "merida"
    elo: int = 1500
    model: str = "maia3-5m"
    time_control: str = "10+0"

    @classmethod
    def load(cls) -> "Settings":
        try:
            with open(_path(), "r", encoding="utf-8") as handle:
                stored = json.load(handle)
        except (OSError, ValueError):
            return cls()

        known = {f.name for f in fields(cls)}
        return cls(**{k: v for k, v in stored.items() if k in known})

    def save(self) -> None:
        try:
            path = _path()
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as handle:
                json.dump(asdict(self), handle, indent=1)
        except OSError:
            pass
