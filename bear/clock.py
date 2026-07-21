"""Two chess clocks.

Deliberately dumb: the clock knows about elapsed time and increment, and nothing about the
game. `bear.window` ticks it and asks how much is left. Time is tracked against a monotonic
source, so the clocks do not jump when the system clock is adjusted.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field

import chess


@dataclass
class Clock:
    """A single side's remaining time, in seconds."""

    remaining: float
    increment: float = 0.0
    _started: float | None = field(default=None, repr=False)

    def start(self) -> None:
        if self._started is None:
            self._started = time.monotonic()

    def stop(self) -> None:
        """Stop counting and bank the increment."""
        if self._started is not None:
            self.remaining = max(0.0, self.remaining - (time.monotonic() - self._started))
            self.remaining += self.increment
            self._started = None

    @property
    def running(self) -> bool:
        return self._started is not None

    def left(self) -> float:
        """Seconds remaining right now, whether running or not."""
        if self._started is None:
            return max(0.0, self.remaining)
        return max(0.0, self.remaining - (time.monotonic() - self._started))

    @property
    def flagged(self) -> bool:
        return self.left() <= 0.0


def format_time(seconds: float) -> str:
    """`m:ss` normally, tenths under ten seconds — the convention players expect."""
    seconds = max(0.0, seconds)
    if seconds < 10:
        return f"{seconds:.1f}"
    minutes, rest = divmod(int(seconds), 60)
    if minutes >= 60:
        hours, minutes = divmod(minutes, 60)
        return f"{hours}:{minutes:02d}:{rest:02d}"
    return f"{minutes}:{rest:02d}"


class Clocks:
    """The pair, with only one running at a time."""

    def __init__(self, minutes: float = 10.0, increment: float = 0.0) -> None:
        self.initial = minutes * 60.0
        self.increment = increment
        self.white = Clock(self.initial, increment)
        self.black = Clock(self.initial, increment)

    def reset(self, minutes: float | None = None, increment: float | None = None) -> None:
        if minutes is not None:
            self.initial = minutes * 60.0
        if increment is not None:
            self.increment = increment
        self.white = Clock(self.initial, self.increment)
        self.black = Clock(self.initial, self.increment)

    def __getitem__(self, colour: chess.Color) -> Clock:
        return self.white if colour == chess.WHITE else self.black

    def press(self, moved: chess.Color) -> None:
        """Hand the clock over: the side that just moved stops, the other starts."""
        self[moved].stop()
        self[not moved].start()

    def stop(self) -> None:
        self.white.stop()
        self.black.stop()

    @property
    def running(self) -> bool:
        return self.white.running or self.black.running
