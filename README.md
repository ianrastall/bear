# Bear

A chess board that plays like a person.

Bear's opponent is [Maia 3](https://github.com/CSSLab/maia3), CSSLab's human move-prediction
model. It is not an engine trying to beat you. It models what a player of a given rating
actually plays, mistakes included — so a game against Bear at 1400 feels like a game against
someone rated 1400, not like a crippled Stockfish.

Bear draws a board, keeps time, and stays out of the way. No analysis, no evaluation bar,
no move list. Everything lives on the board's right-click menu.

Python 3.10+ · PySide6 · python-chess.

## Running it

```powershell
python -m pip install pyside6
python -m pip install -e path\to\maia3      # https://github.com/CSSLab/maia3
maia3-cache --model maia3-5m                # pre-fetch the weights, once
```

Then `python bear-start.py`, or double-click `bear.bat`.

Without `maia3-cache`, the first game downloads the checkpoint from Hugging Face, which can
take long enough to look like a hang. If Maia cannot start at all, Bear says so once and
then works as a board for two people rather than refusing to open.

## Playing

Click a piece, click where it goes. Right-click the board for **New game**, **Flip board**,
**Strength** (800–2200), **Model**, **Time** (1+0 through 15+10, or unlimited), and
**Pieces** (Merida or CBurnett).

The 5M model is the right default: 55.4% move-matching against the 79M's 57.1%, at a
fifteenth of the size, and comfortable on a CPU.

## Recorded games

Every game is written to `games/bear-YYYY-MM-DD.pgn` as it finishes. Nothing to export and
nothing to remember to press.

Moves carry the mover's remaining time as `[%clk]` and Maia's read of the position as
`[%eval]` — the convention Lichess uses, so the files open correctly anywhere that
understands them.

**The eval is not a search evaluation.** It comes from Maia's value head and predicts how a
*human* game continues from that position, at the rating being played. It will not agree
with Stockfish and is not meant to. Every PGN says so in its `Annotator` header.

The opening is named from the [Lichess openings list](https://github.com/lichess-org/chess-openings)
(`all.tsv`) and recorded in an `Opening` header. The name only moves forward: once a game
leaves theory it keeps the last opening it genuinely reached.

## How it fits together

```text
bear-start.py      launches the window
bear/window.py     the window: board, clocks, two status lines, right-click menus
bear/boardview.py  draws the board, turns clicks into moves
bear/engine.py     Maia 3, over UCI
bear/openings.py   names the opening from all.tsv
bear/clock.py      two chess clocks
bear/gamelog.py    PGN with clocks and evals
```

**Bear talks to Maia 3 over UCI and never imports it.** That boundary is the whole design.
The Chessformer architecture, the eight-position board history, the Elo embeddings,
temperature and nucleus sampling all live on CSSLab's side of it and stay theirs to change.
When Maia 3 updates, Bear changes a model alias and nothing else. Reaching into
`maia3.models` would look like working more closely with their system and would in fact be
the brittle path.

A Chessformer forward pass is fast but not instant, so Maia runs on a worker thread; the
board never blocks on it.

## Not in scope

No analysis, no move list, no engine output, no player profiles, no game database. The board
is the app.

## Board colours

Only the dark square is authored. Border and light square are *derived* from it, enforcing
`border < dark < light`: same hue throughout, HSL lightness stepped −0.18 and +0.22, and
saturation reduced moving away from the dark square (border ×0.92, light ×0.78). The
desaturation is what keeps light squares from going lurid, and is why this is done in HSL
rather than in linear light. It is a port of the Board Colors tool at
<https://chessnerd.net/board-colors.html>.

Palettes are solved to land on a target *relative luminance* rather than a target HSL
lightness, which is what lets "dark" mean the same darkness across every hue.

That maths lives in [tools/gen_palettes.py](tools/gen_palettes.py) and
[tools/_colormath.py](tools/_colormath.py). It is not yet wired into the Qt board, which
currently uses one fixed pair — that is the next piece of work.

## History

[LESSONS.md](LESSONS.md) records what an earlier C# / WinUI version of Bear cost to learn —
including why Maia 2 cannot play the opening at all, and why verifying a port against an
artifact generated from that same port proves nothing.

## Credits

- **Maia 3 / Chessformer** — Monroe, Eilender, Chalmers, Tang and Anderson, University of
  Toronto CSSLab. [Paper](https://arxiv.org/abs/2605.19091) ·
  [code](https://github.com/CSSLab/maia3)
- **Pieces** — Merida and CBurnett, via Lichess. See [LICENSE-ASSETS.md](LICENSE-ASSETS.md).
- **Openings** — [lichess-org/chess-openings](https://github.com/lichess-org/chess-openings)
- **Rules** — [python-chess](https://github.com/niklasf/python-chess)

Bear itself is MIT. See [LICENSE](LICENSE).
