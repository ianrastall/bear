"""Bear's one window: a board, two clocks, and two lines of text.

The two lines are the whole of Bear's status display — the opponent's strength, and the
opening as it gets named. Everything else lives behind a right-click.

Maia runs on a worker thread. A Chessformer forward pass is fast but not instant, and doing
it on the UI thread would freeze the board for the duration of every reply.
"""

from __future__ import annotations

import chess
from PySide6.QtCore import QObject, QThread, QTimer, Qt, Signal, Slot
from PySide6.QtGui import QAction, QActionGroup, QFont
from PySide6.QtWidgets import (QHBoxLayout, QLabel, QMainWindow, QMenu,
                               QMessageBox, QVBoxLayout, QWidget)

from bear import openings as openings_module
from bear.boardview import BoardView
from bear.clock import Clocks, format_time
from bear.engine import MODELS, EngineUnavailable, Maia
from bear.gamelog import GameLog

TIME_CONTROLS = [("1+0", 1, 0), ("3+2", 3, 2), ("5+0", 5, 0),
                 ("10+0", 10, 0), ("15+10", 15, 10), ("Unlimited", 0, 0)]
STRENGTHS = [800, 1000, 1200, 1400, 1600, 1800, 2000, 2200]


class EngineWorker(QObject):
    """Owns the Maia process. Lives on its own thread; only ever spoken to by signal."""

    move_ready = Signal(object)
    failed = Signal(str)
    started = Signal()

    def __init__(self, model: str, elo: int) -> None:
        super().__init__()
        self._model, self._elo = model, elo
        self._maia: Maia | None = None

    @Slot()
    def start(self) -> None:
        try:
            self._maia = Maia(self._model, self._elo)
        except EngineUnavailable as exc:
            self.failed.emit(str(exc))
        else:
            self.started.emit()

    @Slot(object)
    def think(self, board: chess.Board) -> None:
        if self._maia is None:
            self.failed.emit("Maia is not running.")
            return
        try:
            self.move_ready.emit(self._maia.move(board))
        except Exception as exc:  # noqa: BLE001 - any failure is "no move"
            self.failed.emit(str(exc))

    @Slot(int)
    def set_elo(self, elo: int) -> None:
        self._model = self._model
        if self._maia is not None:
            self._maia.elo = elo

    @Slot(str)
    def set_model(self, model: str) -> None:
        self._model = model
        if self._maia is not None:
            self._maia.close()
            self._maia = None
        self.start()

    @Slot()
    def shutdown(self) -> None:
        if self._maia is not None:
            self._maia.close()
            self._maia = None


class BearWindow(QMainWindow):
    think_requested = Signal(object)
    elo_requested = Signal(int)
    model_requested = Signal(str)

    def __init__(self, model: str = "maia3-5m", elo: int = 1500,
                 minutes: float = 10, increment: float = 0) -> None:
        super().__init__()
        self.setWindowTitle("Bear")

        self.board = chess.Board()
        self.elo = elo
        self.model = model
        self.human_colour: chess.Color = chess.WHITE
        self.clocks = Clocks(minutes, increment)
        self.time_control = "10+0"
        self.openings = self._load_openings()
        self.log = GameLog()
        self._engine_ready = False

        self._build()
        self._start_engine()
        self.new_game()

        self._ticker = QTimer(self)
        self._ticker.timeout.connect(self._tick)
        self._ticker.start(100)

    # -- construction -----------------------------------------------------

    def _load_openings(self):
        try:
            return openings_module.load()
        except OSError:
            return None

    def _build(self) -> None:
        self.view = BoardView("merida")
        self.view.move_made.connect(self.on_human_move)
        self.view.customContextMenuRequested.connect(self._show_menu)

        mono = QFont("Consolas")
        mono.setStyleHint(QFont.StyleHint.Monospace)
        mono.setPointSize(20)

        self.black_clock = QLabel("10:00")
        self.white_clock = QLabel("10:00")
        for label in (self.black_clock, self.white_clock):
            label.setFont(mono)
            label.setAlignment(Qt.AlignmentFlag.AlignCenter)

        self.strength_line = QLabel("")
        self.opening_line = QLabel("")
        self.opening_line.setAlignment(Qt.AlignmentFlag.AlignRight)
        for label in (self.strength_line, self.opening_line):
            label.setStyleSheet("color: #6b6b6b;")

        status = QHBoxLayout()
        status.addWidget(self.strength_line)
        status.addStretch(1)
        status.addWidget(self.opening_line)

        clocks = QHBoxLayout()
        clocks.addWidget(self.black_clock)
        clocks.addStretch(1)
        clocks.addWidget(self.white_clock)

        layout = QVBoxLayout()
        layout.addLayout(clocks)
        layout.addWidget(self.view, 1)
        layout.addLayout(status)

        central = QWidget()
        central.setLayout(layout)
        self.setCentralWidget(central)
        self.resize(720, 820)

    def _start_engine(self) -> None:
        self._thread = QThread(self)
        self._worker = EngineWorker(self.model, self.elo)
        self._worker.moveToThread(self._thread)

        self._thread.started.connect(self._worker.start)
        self._worker.started.connect(self.on_engine_started)
        self._worker.move_ready.connect(self.on_engine_move)
        self._worker.failed.connect(self.on_engine_failed)
        self.think_requested.connect(self._worker.think)
        self.elo_requested.connect(self._worker.set_elo)
        self.model_requested.connect(self._worker.set_model)

        self._thread.start()

    # -- game flow --------------------------------------------------------

    def new_game(self, human_white: bool | None = None) -> None:
        import random
        self.board = chess.Board()
        self.log.reset()
        self.human_colour = (chess.WHITE if human_white else chess.BLACK) \
            if human_white is not None else random.choice([chess.WHITE, chess.BLACK])

        self.view.set_board(self.board)
        self.view.human_colour = self.human_colour
        self.view.flipped = self.human_colour == chess.BLACK
        self.view.frozen = False

        minutes, increment = next((m, i) for n, m, i in TIME_CONTROLS
                                  if n == self.time_control)
        self.clocks.reset(minutes or 0, increment)
        self._refresh()

        if self.board.turn != self.human_colour:
            self._ask_maia()
        elif self.time_control != "Unlimited":
            self.clocks[self.board.turn].start()

    @Slot(object)
    def on_human_move(self, move: chess.Move) -> None:
        self._commit(move)
        if not self.board.is_game_over():
            self._ask_maia()

    def _ask_maia(self) -> None:
        if not self._engine_ready:
            return
        self.view.frozen = True
        self.think_requested.emit(self.board.copy())

    @Slot(object)
    def on_engine_move(self, reply) -> None:
        self.view.frozen = False
        # Maia scored the position it was handed, which the previous move created.
        if reply.score is not None:
            self.log.annotate_last(reply.score)
        if reply.move in self.board.legal_moves:
            self._commit(reply.move)

    def _commit(self, move: chess.Move) -> None:
        mover = self.board.turn
        self.board.push(move)
        if self.time_control != "Unlimited":
            self.clocks.press(mover)
        self.log.add(move, self.clocks[mover].left()
                     if self.time_control != "Unlimited" else None)
        self.view.set_board(self.board)
        self._refresh()
        self._check_over()

    def _finish(self, message: str) -> None:
        """Stop the game, write the PGN, and say what happened."""
        self.clocks.stop()
        self.view.frozen = True
        try:
            game = self.log.build(
                self.board,
                white="You" if self.human_colour == chess.WHITE else "Maia 3",
                black="You" if self.human_colour == chess.BLACK else "Maia 3",
                elo=self.elo, model=self.model, time_control=self.time_control,
                opening=self.openings.name_for(self.board) if self.openings else None,
            )
            self.log.save(game)
        except OSError:
            pass  # a game that cannot be filed is still a game that was played
        QMessageBox.information(self, "Game over", message)

    def _check_over(self) -> None:
        if not self.board.is_game_over(claim_draw=True):
            return
        outcome = self.board.outcome(claim_draw=True)
        reason = outcome.termination.name.replace("_", " ").title() if outcome else "Over"
        self._finish(f"{self.board.result(claim_draw=True)} — {reason}")

    # -- status -----------------------------------------------------------

    def _refresh(self) -> None:
        side = "White" if self.human_colour == chess.WHITE else "Black"
        state = "Maia 3" if self._engine_ready else "Maia 3 (not loaded)"
        self.strength_line.setText(
            f"{state} · {self.elo} · {self.model.replace('maia3-', '')} · you play {side}")

        name = self.openings.name_for(self.board) if self.openings else None
        self.opening_line.setText(name or "")

    def _tick(self) -> None:
        self.white_clock.setText(format_time(self.clocks.white.left()))
        self.black_clock.setText(format_time(self.clocks.black.left()))

        for colour in (chess.WHITE, chess.BLACK):
            clock = self.clocks[colour]
            if clock.running and clock.flagged:
                loser = "White" if colour == chess.WHITE else "Black"
                self._finish(f"{loser} flagged.")
                return

    # -- menu -------------------------------------------------------------

    def _show_menu(self, position) -> None:
        menu = QMenu(self)

        menu.addAction("New game", lambda: self.new_game())
        menu.addAction("Flip board", self.view.flip)
        menu.addSeparator()

        strength = menu.addMenu("Strength")
        group = QActionGroup(strength)
        for value in STRENGTHS:
            action = QAction(str(value), strength, checkable=True)
            action.setChecked(value == self.elo)
            action.triggered.connect(lambda _=False, v=value: self._set_elo(v))
            group.addAction(action)
            strength.addAction(action)

        model_menu = menu.addMenu("Model")
        for name in MODELS:
            action = QAction(name.replace("maia3-", ""), model_menu, checkable=True)
            action.setChecked(name == self.model)
            action.triggered.connect(lambda _=False, m=name: self._set_model(m))
            model_menu.addAction(action)

        time_menu = menu.addMenu("Time")
        for name, _m, _i in TIME_CONTROLS:
            action = QAction(name, time_menu, checkable=True)
            action.setChecked(name == self.time_control)
            action.triggered.connect(lambda _=False, n=name: self._set_time(n))
            time_menu.addAction(action)

        pieces = menu.addMenu("Pieces")
        for name in ("merida", "cburnett"):
            action = QAction(name.title(), pieces, checkable=True)
            action.setChecked(name == self.view.piece_set)
            action.triggered.connect(lambda _=False, n=name: self.view.set_piece_set(n))
            pieces.addAction(action)

        menu.addSeparator()
        menu.addAction("Quit", self.close)
        menu.exec(self.view.mapToGlobal(position))

    def _set_elo(self, value: int) -> None:
        self.elo = value
        self.elo_requested.emit(value)
        self._refresh()

    def _set_model(self, name: str) -> None:
        self.model = name
        self._engine_ready = False
        self.model_requested.emit(name)
        self._refresh()

    def _set_time(self, name: str) -> None:
        self.time_control = name
        self.new_game(self.human_colour == chess.WHITE)

    # -- engine lifecycle -------------------------------------------------

    @Slot()
    def on_engine_started(self) -> None:
        self._engine_ready = True
        self._refresh()
        if self.board.turn != self.human_colour and not self.board.is_game_over():
            self._ask_maia()

    @Slot(str)
    def on_engine_failed(self, message: str) -> None:
        self._engine_ready = False
        self.view.frozen = False
        # Without an opponent Bear is still a board two people can use.
        self.view.human_colour = None
        self._refresh()
        QMessageBox.warning(self, "Maia 3 unavailable", message)

    def closeEvent(self, event) -> None:
        self._ticker.stop()
        self._worker.shutdown()
        self._thread.quit()
        self._thread.wait(5000)
        super().closeEvent(event)
