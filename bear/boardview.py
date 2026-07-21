"""The board.

A plain `QWidget` that paints eight ranks and files and the Merida pieces, and turns clicks
into moves. It knows the rules only through python-chess: it asks the board what is legal
and never decides anything itself.

Pieces are SVG and are rendered at whatever size the board happens to be, so the set stays
sharp at every window size. That is the reason for Qt here — it was the one thing a
pre-rendered PNG set could never do well.
"""

from __future__ import annotations

import os

import chess
from PySide6.QtCore import QRectF, Qt, Signal
from PySide6.QtGui import QColor, QPainter
from PySide6.QtSvg import QSvgRenderer
from PySide6.QtWidgets import QMenu, QWidget

from bear import theme
from bear.theme import Palette

ASSETS = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets")

LAST_MOVE = QColor(255, 214, 92, 110)
SELECTED = QColor(120, 190, 120, 130)
TARGET = QColor(40, 40, 40, 46)
CHECK = QColor(214, 78, 62, 150)

#: How much of the board's width the border takes on each side.
BORDER = 0.025


class BoardView(QWidget):
    """Draws a position and reports the moves a person makes on it."""

    move_made = Signal(chess.Move)

    def __init__(self, piece_set: str = "merida", palette: Palette | None = None,
                 parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.board = chess.Board()
        self.flipped = False
        #: Which colour the person may move. None means both — a board for two people.
        self.human_colour: chess.Color | None = chess.WHITE
        self.frozen = False

        self._selected: chess.Square | None = None
        self._targets: set[chess.Square] = set()
        self._renderers: dict[str, QSvgRenderer] = {}
        self.set_palette(palette or theme.get(None))
        self.set_piece_set(piece_set)

        self.setMinimumSize(320, 320)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.NoContextMenu)

    # -- colours ----------------------------------------------------------

    def set_palette(self, palette: Palette) -> None:
        self.palette_choice = palette
        self._light = QColor(*palette.light)
        self._dark = QColor(*palette.dark)
        self._border = QColor(*palette.border)
        self.update()

    # -- pieces -----------------------------------------------------------

    def set_piece_set(self, name: str) -> None:
        self.piece_set = name
        self._renderers.clear()
        folder = os.path.join(ASSETS, "pieces", name)
        for colour in "wb":
            for symbol in "KQRBNP":
                path = os.path.join(folder, f"{colour}{symbol}.svg")
                if os.path.exists(path):
                    self._renderers[f"{colour}{symbol}"] = QSvgRenderer(path)
        self.update()

    # -- geometry ---------------------------------------------------------

    @property
    def _extent(self) -> float:
        """The full drawn area, border included."""
        return min(self.width(), self.height())

    @property
    def _side(self) -> float:
        return (self._extent * (1.0 - 2 * BORDER)) / 8.0

    def _origin(self) -> tuple[float, float]:
        """Top-left of the playing surface, inside the border."""
        inset = self._extent * BORDER
        return ((self.width() - self._extent) / 2.0 + inset,
                (self.height() - self._extent) / 2.0 + inset)

    def _square_rect(self, square: chess.Square) -> QRectF:
        file_, rank = chess.square_file(square), chess.square_rank(square)
        if self.flipped:
            file_, rank = 7 - file_, 7 - rank
        ox, oy = self._origin()
        return QRectF(ox + file_ * self._side, oy + (7 - rank) * self._side,
                      self._side, self._side)

    def _square_at(self, x: float, y: float) -> chess.Square | None:
        ox, oy = self._origin()
        col, row = int((x - ox) // self._side), int((y - oy) // self._side)
        if not (0 <= col <= 7 and 0 <= row <= 7):
            return None
        file_, rank = col, 7 - row
        if self.flipped:
            file_, rank = 7 - file_, 7 - rank
        return chess.square(file_, rank)

    # -- painting ---------------------------------------------------------

    def paintEvent(self, _event) -> None:
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        painter.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform)

        last = self.board.move_stack[-1] if self.board.move_stack else None

        # The border is the third derived tone, and the reason the derivation exists.
        extent = self._extent
        painter.fillRect(QRectF((self.width() - extent) / 2.0,
                                (self.height() - extent) / 2.0, extent, extent),
                         self._border)

        for square in chess.SQUARES:
            rect = self._square_rect(square)
            light = (chess.square_file(square) + chess.square_rank(square)) % 2 == 1
            painter.fillRect(rect, self._light if light else self._dark)

            if last is not None and square in (last.from_square, last.to_square):
                painter.fillRect(rect, LAST_MOVE)
            if square == self._selected:
                painter.fillRect(rect, SELECTED)

        # The king in check, so the player is never surprised by it.
        if self.board.is_check():
            king = self.board.king(self.board.turn)
            if king is not None:
                painter.fillRect(self._square_rect(king), CHECK)

        for square in self._targets:
            rect = self._square_rect(square)
            radius = self._side * (0.42 if self.board.piece_at(square) else 0.16)
            painter.setBrush(TARGET)
            painter.setPen(Qt.PenStyle.NoPen)
            painter.drawEllipse(rect.center(), radius, radius)

        for square, piece in self.board.piece_map().items():
            key = ("w" if piece.color else "b") + piece.symbol().upper()
            renderer = self._renderers.get(key)
            if renderer is not None:
                renderer.render(painter, self._square_rect(square))

        painter.end()

    # -- interaction ------------------------------------------------------

    def _may_move(self) -> bool:
        if self.frozen or self.board.is_game_over():
            return False
        return self.human_colour is None or self.board.turn == self.human_colour

    def mousePressEvent(self, event) -> None:
        if event.button() == Qt.MouseButton.RightButton:
            self.customContextMenuRequested.emit(event.pos())
            return
        if not self._may_move():
            return

        square = self._square_at(event.position().x(), event.position().y())
        if square is None:
            return

        if self._selected is not None and square in self._targets:
            self._play(self._selected, square)
            return

        piece = self.board.piece_at(square)
        if piece is not None and piece.color == self.board.turn:
            self._selected = square
            self._targets = {m.to_square for m in self.board.legal_moves
                             if m.from_square == square}
        else:
            self._selected, self._targets = None, set()
        self.update()

    def _play(self, origin: chess.Square, target: chess.Square) -> None:
        candidates = [m for m in self.board.legal_moves
                      if m.from_square == origin and m.to_square == target]
        self._selected, self._targets = None, set()

        if not candidates:
            self.update()
            return

        move = candidates[0]
        if len(candidates) > 1:  # a promotion: ask rather than assume a queen
            chosen = self._ask_promotion(candidates)
            if chosen is None:
                self.update()
                return
            move = chosen

        self.update()
        self.move_made.emit(move)

    def _ask_promotion(self, candidates: list[chess.Move]) -> chess.Move | None:
        menu = QMenu(self)
        names = {chess.QUEEN: "Queen", chess.ROOK: "Rook",
                 chess.BISHOP: "Bishop", chess.KNIGHT: "Knight"}
        actions = {}
        for move in sorted(candidates, key=lambda m: -(m.promotion or 0)):
            if move.promotion:
                actions[menu.addAction(names.get(move.promotion, "?"))] = move
        picked = menu.exec(self.mapToGlobal(
            self._square_rect(candidates[0].to_square).center().toPoint()))
        return actions.get(picked)

    # -- state ------------------------------------------------------------

    def set_board(self, board: chess.Board) -> None:
        self.board = board
        self._selected, self._targets = None, set()
        self.update()

    def flip(self) -> None:
        self.flipped = not self.flipped
        self.update()
