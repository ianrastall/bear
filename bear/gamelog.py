"""Every game, written to PGN as it is played.

Games are recorded automatically — there is nothing to export and nothing to remember to
press. Each move carries the mover's remaining time as `[%clk]`, and Maia's assessment of
the resulting position as `[%eval]`, which is the same annotation convention Lichess uses,
so the files open correctly in any viewer that understands it.

One honest caveat travels with the eval. Maia's number comes from its value head: it
predicts how a *human* game continues from here, at the rating it is playing. It is not a
search evaluation and will not agree with one, so the PGN says so in a header rather than
letting a reader assume Stockfish.
"""

from __future__ import annotations

import datetime as _dt
import os
from dataclasses import dataclass

import chess
import chess.engine
import chess.pgn


@dataclass
class Record:
    """One ply: the move, the mover's clock after it, and how the position then looked."""

    move: chess.Move
    clock: float | None = None
    score: "chess.engine.PovScore | None" = None


class GameLog:
    """Accumulates a game and writes it out."""

    def __init__(self, folder: str | os.PathLike[str] | None = None) -> None:
        if folder is None:
            root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            folder = os.path.join(root, "games")
        self.folder = str(folder)
        self.records: list[Record] = []
        self.started = _dt.datetime.now()

    def reset(self) -> None:
        self.records.clear()
        self.started = _dt.datetime.now()

    def add(self, move: chess.Move, clock: float | None) -> None:
        self.records.append(Record(move, clock))

    def annotate_last(self, score) -> None:
        """Attach a score to the most recent move.

        Maia evaluates the position it is asked to move in, which is the position the
        *previous* move created — so its score belongs to that move, matching the usual
        reading of `[%eval]` as "the evaluation after this move".
        """
        if self.records:
            self.records[-1].score = score

    def build(self, board: chess.Board, *, white: str, black: str,
              elo: int, model: str, time_control: str,
              opening: str | None = None) -> chess.pgn.Game:
        game = chess.pgn.Game()
        game.headers["Event"] = "Bear"
        game.headers["Site"] = "Bear"
        game.headers["Date"] = self.started.strftime("%Y.%m.%d")
        game.headers["White"] = white
        game.headers["Black"] = black
        game.headers["Result"] = board.result(claim_draw=True)
        game.headers["TimeControl"] = time_control
        game.headers["Annotator"] = f"Maia 3 ({model}) value head, not a search evaluation"
        if opening:
            game.headers["Opening"] = opening
        for colour, name in (("White", white), ("Black", black)):
            if name.startswith("Maia"):
                game.headers[f"{colour}Elo"] = str(elo)

        node: chess.pgn.GameNode = game
        for record in self.records:
            node = node.add_variation(record.move)
            if record.clock is not None:
                node.set_clock(record.clock)
            if record.score is not None:
                node.set_eval(record.score)
        return game

    def save(self, game: chess.pgn.Game) -> str:
        """Append to a file named for the day, and return its path."""
        os.makedirs(self.folder, exist_ok=True)
        path = os.path.join(self.folder, f"bear-{self.started:%Y-%m-%d}.pgn")
        with open(path, "a", encoding="utf-8") as handle:
            print(game, file=handle, end="\n\n")
        return path
