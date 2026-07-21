"""Naming the opening as it happens.

The source is the Lichess `chess-openings` list (`all.tsv`): 3806 named openings, each with
the position it arrives at. Bear does not use it as a book — Maia 3 is trained on the
opening and plays it perfectly well on its own — but naming the position costs one dictionary
lookup per move and tells the player what they have walked into.

The name only ever moves forward. Once a game leaves the book, the last name it matched is
what it stays: a game that reaches the Réti and then goes its own way is still a Réti, and
blanking the label the moment theory runs out would be worse than useless.
"""

from __future__ import annotations

import csv
import os

import chess


class Openings:
    """Position to opening name."""

    def __init__(self, names: dict[str, str]) -> None:
        self._names = names

    def __len__(self) -> int:
        return len(self._names)

    @classmethod
    def from_tsv(cls, path: str | os.PathLike[str]) -> "Openings":
        names: dict[str, str] = {}
        with open(path, "r", encoding="utf-8", newline="") as handle:
            for row in csv.DictReader(handle, delimiter="\t"):
                epd, name = (row.get("epd") or "").strip(), (row.get("name") or "").strip()
                eco = (row.get("eco") or "").strip()
                if not epd or not name:
                    continue
                # Deeper rows overwrite shallower ones only if we have not seen this exact
                # position; the file lists each position once, so first write wins.
                names.setdefault(epd, f"{eco} {name}".strip())
        return cls(names)

    def lookup(self, board: chess.Board) -> str | None:
        """The name for this exact position, or None."""
        return self._names.get(board.epd())

    def name_for(self, board: chess.Board) -> str | None:
        """The deepest name reached along the game so far.

        Replays the move stack rather than trusting the current position alone, so a game
        that transposes out of the book keeps the last name it genuinely matched.
        """
        replay = chess.Board()
        found = self.lookup(replay)
        for move in board.move_stack:
            replay.push(move)
            found = self.lookup(replay) or found
        return found


def load(path: str | os.PathLike[str] | None = None) -> Openings:
    """Load `all.tsv` from the repository root unless told otherwise."""
    if path is None:
        root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        path = os.path.join(root, "all.tsv")
    return Openings.from_tsv(path)
