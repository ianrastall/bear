"""Maia 3, driven over UCI.

Bear does not import maia3. It launches `maia3-uci` and talks to it the way CSSLab document,
which is the only interface they treat as stable. Everything interesting — the Chessformer
network, the eight-position board history, the Elo embeddings, temperature and nucleus
sampling — lives behind that boundary and stays theirs to change. When Maia 3 updates, Bear
changes a model alias and nothing else.

Calls block for as long as a forward pass takes, so nothing here may run on the UI thread.
`bear.window` drives it from a worker.
"""

from __future__ import annotations

import os
import shutil
import sys
from dataclasses import dataclass

import chess
import chess.engine

#: A single forward pass. Maia has no search, so there is nothing for a larger limit to buy.
ONE_PASS = chess.engine.Limit(nodes=1)

MODELS = ("maia3-5m", "maia3-23m", "maia3-79m")


class EngineUnavailable(RuntimeError):
    """Maia 3 could not be started. Bear stays usable as a board for two people."""


@dataclass(frozen=True)
class Reply:
    """A move, and Maia's read of the position it was asked about."""

    move: chess.Move
    score: "chess.engine.PovScore | None" = None


def _command(model: str) -> list[str]:
    """Prefer the installed console script; fall back to module execution."""
    script = shutil.which("maia3-uci")
    base = [script] if script else [sys.executable, "-m", "maia3.uci"]
    return base + [
        "--model", model,
        # Maia 3 conditions on the previous seven positions. Without this it only ever sees
        # the current one, and plays measurably worse for no reason.
        "--use-uci-history",
        "--device", "cpu",
        "--no-use-amp",
    ]


def _environment() -> dict[str, str] | None:
    """Let Bear run against a maia3 *clone* that was never pip-installed.

    Set `BEAR_MAIA3` to the checkout. Only consulted when maia3 is not importable, so an
    installed copy always wins.
    """
    import importlib.util

    if importlib.util.find_spec("maia3") is not None:
        return None

    clone = os.environ.get("BEAR_MAIA3")
    if not clone or not os.path.isdir(clone):
        return None

    environment = dict(os.environ)
    environment["PYTHONPATH"] = clone + os.pathsep + environment.get("PYTHONPATH", "")
    return environment


class Maia:
    """One Maia 3 process, configured for a rating."""

    def __init__(self, model: str = "maia3-5m", elo: int = 1500,
                 temperature: float = 1.0, top_p: float = 1.0) -> None:
        self.model = model
        self._elo = elo
        self._temperature = temperature
        self._top_p = top_p

        try:
            self._engine = chess.engine.SimpleEngine.popen_uci(
                _command(model), env=_environment(), timeout=600)
        except Exception as exc:  # noqa: BLE001 - surfaced to the UI as one condition
            raise EngineUnavailable(
                f"Could not start Maia 3 ({model}).\n\n"
                f"    pip install -e <maia3 checkout>\n"
                f"    maia3-cache --model {model}\n\n"
                f"Or set BEAR_MAIA3 to a maia3 checkout to run it without installing.\n\n"
                f"{exc}"
            ) from exc

        self._apply()

    # -- configuration ----------------------------------------------------

    def _apply(self) -> None:
        """Push current settings, skipping anything this build does not expose."""
        wanted = {
            "Elo": self._elo,
            "Temperature": self._temperature,
            "TopP": self._top_p,
        }
        available = {k: v for k, v in wanted.items() if k in self._engine.options}
        if available:
            self._engine.configure(available)

    @property
    def elo(self) -> int:
        return self._elo

    @elo.setter
    def elo(self, value: int) -> None:
        self._elo = int(value)
        self._apply()

    @property
    def temperature(self) -> float:
        return self._temperature

    @temperature.setter
    def temperature(self, value: float) -> None:
        self._temperature = float(value)
        self._apply()

    # -- play -------------------------------------------------------------

    def move(self, board: chess.Board) -> "Reply":
        """Maia's move for this position, with its assessment of it. Blocking.

        The score comes from Maia's value head and is a prediction of how a human game
        from here ends — not a search evaluation. It is worth recording in the PGN, but
        it should never be presented as an engine eval.
        """
        result = self._engine.play(board, ONE_PASS, info=chess.engine.INFO_SCORE)
        if result.move is None:
            raise EngineUnavailable("Maia 3 returned no move")
        return Reply(result.move, result.info.get("score"))

    def close(self) -> None:
        try:
            self._engine.close()
        except Exception:  # noqa: BLE001 - shutting down; nothing useful to do
            pass

    def __enter__(self) -> "Maia":
        return self

    def __exit__(self, *_) -> None:
        self.close()
