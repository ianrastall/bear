# Bear

A native chess GUI for Windows: play a human-like opponent, analyse, keep games, export PGN.

C# 14 · .NET 10 · WinUI 3 (Windows App SDK 2.3.1) · **win-x64 only** · ships as a single `.exe`.

## Build

```powershell
dotnet build                                  # debug
dotnet publish src/Bear.App -c Release -o artifacts/publish
```

The publish output is one file: `artifacts/publish/Bear.exe` (~106 MB). The `.pdb`
files beside it are debug symbols and are not needed for distribution.

No .NET runtime and no Windows App SDK runtime are required on the target machine —
both are inside the exe.

## Deliberate build decisions

These are load-bearing; changing any one of them breaks the single-exe requirement.

| Property | Value | Why |
| --- | --- | --- |
| `WindowsPackageType` | `None` | Unpackaged. The deliverable is a bare exe, not an MSIX. |
| `EnableMsixTooling` | `true` | Counter-intuitive, but Windows App SDK's `SingleFile.targets` **refuses to run without it** — it needs the MSIX tooling to generate the `resources.pri` that gets embedded. It does not make the app packaged. |
| `SelfContained` | `true` | Bundles the .NET runtime. |
| `WindowsAppSDKSelfContained` | `true` | Bundles the Windows App SDK runtime. |
| `IncludeNativeLibrariesForSelfExtract` | `true` | WinUI ships native DLLs; without this they land beside the exe instead of inside it. |
| `PublishTrimmed` | `false` | **Leave off.** WinUI resolves types from XAML by name at runtime; the trimmer can't see those references, so trimmed builds compile cleanly and then fail when a page loads. |
| `Platforms` / `RuntimeIdentifier` | `x64` / `win-x64` | Set solution-wide in `Directory.Build.props` so no project can drift to AnyCPU or x86. |

Dark mode is set once, at the application level: `RequestedTheme="Dark"` in `App.xaml`.

## Layout

```text
src/Bear.Engine/           UCI engine wrapper — no chess rules, no UI, no dependencies
src/Bear.App/
  Controls/                the board: rendering, drag-and-drop, right-click menu
  Theming/                 luminance-linked palettes, embedded Merida SVGs
```

`Bear.Engine` deliberately knows nothing about chess. It speaks UCI to a subprocess
and nothing more, so lc0 (running Maia weights), Stockfish, or any other UCI engine
drives through the same `IChessEngine` interface.

## Design notes

**The board is the app.** No move list (nobody reads their own notation mid-game — a
record is enough, and that's what PGN export is for), no player names, no avatars, no
eval bar. The window is therefore essentially square. Every command lives on the board's
**right-click menu**; the only permanent chrome is a thin clock strip above and below.

**Board colours.** Only the dark square is authored; border and light square are *derived*
from it, enforcing `Border < Dark < Light`.

The derivation is a faithful port of the Board Colors tool at
<https://chessnerd.net/board-colors.html> — same hue throughout, HSL lightness stepped
-0.18 and +0.22, and saturation *reduced* moving away from the dark square
(border ×0.92, light ×0.78). That desaturation is the part that keeps light squares from
going lurid, and it is why this is done in HSL rather than in linear light.

Relative luminance is a **diagnostic, not the mechanism**: it only classifies the dark
square as Too Dark (<0.11) / OK / Too Light (>0.42), as the tool reports it. All twenty
presets sit inside the OK band — seven candidates were lightened because they failed it.

The C# port was verified to emit hex values identical to the JavaScript original.
Keep `Theming/ColorMath.cs` in step with the tool rather than "improving" it separately.

**Coordinates** are drawn on all four sides, centred in the border thickness and aligned
to square centres, matching the tool's SVG export (border = 0.405 × square, coordinate
font = 20/84 × square, `#F3F3F3`).

**Clocks.** Conventional pair: Black's face dark with light digits, White's light with
dark digits; the inactive side dims. An earlier attempt borrowed the Lc0 logo's pink
segment-font look, but without an actual segment font it just read as a pink clock rather
than an homage, so it was dropped.

**Pieces.** Merida SVGs from Lichess, embedded as resources (GPLv2+ — see
`LICENSE-ASSETS.md`).

## Licence

Bear's source is **MIT** (`LICENSE`) — permissive, with the one condition that the copyright
notice travels with it. That condition is the point: CC0 was considered and rejected because
a rights waiver gives up attribution, which is the only thing actually worth keeping here.

Embedded artwork and network weights are licensed separately, and a build is only as
permissive as the most restrictive thing inside it. `LICENSE-ASSETS.md` records what is
actually in the exe; it is the file to check before cutting a release, not this one.

Trained weights would be Bear's own if produced here — GPLv3 does not reach a program's
output, and the Lichess database is CC0 — so self-training is a real escape hatch from the
weights question. It is also a larger project than Bear, needing a port of a 2020
TensorFlow/CUDA stack, ~410 M positions per net, and an evaluation harness to prove the
result plays like a human at all. Someday, not first.

## Not in scope

These are deliberate omissions, each considered and dropped. Don't reintroduce one without
a reason that outlives the itch.

- **Analysis.** No eval bar, no hints, no best-move arrows, no second engine. It's a large
  feature that drags in a search engine and an entire second UI language, and it works
  against the point of the app, which is to play a game and find out how it went.
- **A move list or notation panel.** Nobody reads their own notation mid-game. A record is
  enough, and PGN export is the record.
- **Any display of the engine's thinking.** No depth, no node count, no principal variation,
  no evaluation. At one forward pass there is nothing to report, and reporting it would
  invite the analysis feature back in through the side door.
- **Simulated thinking time.** Maia's move after ten seconds is its move after a tenth of a
  second — search is off, so a delay would be pure theatre. Bear moves when it has the move.
- **User profiles.** Two people sharing one machine is likely, but not painful enough to
  justify accounts.
- **A stored game database.** Keeping history implies a single owner for that history, which
  smuggles profiles back in. The current game exports to PGN; that is the whole story.
- **A tracked player rating.** Maia 2 wants the opponent's rating, which looks like a reason
  to build a database, a profile and a Glicko-2 implementation. It isn't.
  `preprocessing()` calls `map_to_category()` and bins the value into one of eleven
  100-point buckets, so a meticulously maintained rating and a rough guess produce the
  *identical tensor*. And the rating would be fiction anyway: the player picks their
  opponent's strength each game, so the opposition is self-selected, and Maia's nets play
  above their label. Bear would be building a measurement apparatus that cannot measure —
  worse than none, because the output would be trusted. See *The opponent* for what it does
  instead.

Exactly one thing persists between sessions: the board palette. That is not a leftover of
the rating question and does not fall with it — it is there because hunting down your
preferred board through a menu of sixty on every launch is precisely the annoyance
persistence exists to remove. The more palettes ship, the more it earns its keep.

## The opponent

Bear plays **Maia 2** — a network trained to predict what a human of a given rating actually
plays, blunders included.

**Maia runs in-process as ONNX, not through lc0.** The model is exported once, offline, with
`torch.onnx.export`, and embedded as a resource; at runtime `Microsoft.ML.OnnxRuntime`
evaluates it on the CPU. No engine subprocess, no external binary, no GPL executable to
redistribute — the single-`.exe` deliverable survives intact.

**lc0 is never compiled and never shipped.** It appears in this project only as the source
of an offline conversion tool. Compiling it would produce a binary tuned to one machine and
solve nothing: Maia does not search, so none of lc0's backend machinery is in play.

This is a proven route rather than a guess: CSSLab's own playing site,
[maia-platform-frontend](https://github.com/CSSLab/maia-platform-frontend), runs these same
nets — `maia_kdd_1100` … `maia_kdd_1900` — as ONNX under `onnxruntime-web`. Its JavaScript
is the reference for the two fiddly parts, input-plane encoding and policy-head decoding.

**There is no search.** Maia is a policy network: the move is a single forward pass,
`go nodes 1` in lc0 terms. Nothing improves with more time. That fact propagates into the
UI — see *Not in scope*.

### Why not Maia 2 or Maia 3

| | Licence | Input encoding | Size | Verdict |
| --- | --- | --- | --- | --- |
| **Maia 2** | **MIT**, no weights carve-out | **18 channels, no history** | ~20–25 M params, est. ~90 MB fp32 | **Chosen.** |
| **Maia 1** | GPL-3.0 | 112 planes incl. 8 steps of history | ~11 MB for all nine nets | Fallback if Maia 2's size proves unworkable. |
| **Maia 3** | **AGPL-3.0** | — | — | **Rejected.** No export path at all. |

Maia 2 wins on three counts. The licence is confirmed MIT (`Copyright (c) 2024 CSSLab`,
asserted in `LICENSE`, `pyproject.toml` and the README, with nothing separate attached to
the checkpoints), which is what lets Bear ship honestly as MIT.

More importantly it wins on **encoding**. Maia 1 inherits lc0's 112-plane input, which
carries eight steps of position history and repetition counts — that history is where the
fiddly bugs live. Maia 2 takes 18 stateless channels: twelve piece planes, side to move,
four castling rights, one en passant. That collapses the hardest part of M4 into an
afternoon.

And it answers the strong-player question honestly: its rating buckets run to **`>=2000`**,
which is what the unofficial 2200 net was reaching for, with a licence attached.

The cost is size. Maia 1 is a 6×64 residual net; Maia 2 is a 5-block ResNet feeding a
2-block ViT at 1024 dims, roughly an order of magnitude larger. fp16 export halves it and
int8 quantisation quarters it, both supported by ONNX Runtime — but that is a deliberate
step, not a freebie, and it is the one thing that could still send this back to Maia 1.

### What Maia 2 is *not*

It is **not** a continuous rating dial. `create_elo_dict()` bins into eleven buckets —
`<1100`, then 1100-1199 … 1900-1999, then `>=2000`. The API accepts an Elo number and
`map_to_category` rounds it. Granularity is the same 100 points as Maia 1; what is new is
the floor, the ceiling, and opponent-rating awareness.

That last one has a UI consequence. `forward(boards, elos_self, elos_oppo)` wants *both*
ratings — and note which is which: `elo_self` is the rating of the player whose move is being
predicted, so that is **Maia's**, straight from the strength selector. The player's rating is
`elo_oppo`, the weaker of the two conditioning inputs.

Bear therefore **mirrors the selector**: both arguments get the same bucket, and Maia plays
as though facing an equal. The player is never asked to self-rate. Given eleven buckets and a
second-order input, the accuracy given up is negligible, and it keeps a rating prompt, a
settings field and a PGN tag out of the app entirely.

### Two models, and the time controls they define

Maia 2 ships **`blitz_model.pt` and `rapid_model.pt`**, and Bear embeds both — roughly
180 MB, a deliberate trade of size for correctness. Size is not the scarce resource here;
speed is, and two forward passes' worth of weights costs nothing at one move per turn.

The time controls are **not chosen by taste — they are read off the models.** `main.py`
filters training data on Lichess's own `Event` header (`_GAME_TYPE_EVENT_MARKERS` is
`{"rapid": "Rapid", "blitz": "Blitz"}`, and `_game_filter` additionally requires `Rated`),
so each model saw exactly one Lichess speed category and nothing else. Lichess classifies by
estimated duration, `base + 40 × increment`: Blitz is 180–479 s, Rapid is 480–1499 s.

Every offered time control must therefore land inside one of those two windows:

| | Est. | Model |
| --- | --- | --- |
| 3+0 | 180 s | blitz |
| 3+2 | 260 s | blitz |
| 5+0 | 300 s | blitz |
| 5+3 | 420 s | blitz |
| 10+0 | 600 s | rapid |
| 10+5 | 800 s | rapid |
| 20+0 | 1200 s | rapid |
| 15+10 | 1300 s | rapid |

**This removes 1+0 and 30+0**, which the app previously offered. 1+0 estimates at 60 s —
Lichess Bullet — and 30+0 at 1800 s — Classical. Maia 2 has no model for either, so both
would have been served by a net that never saw a game at that speed. Offering them looked
like generosity and was actually the one place where the opponent would quietly stop being
human-like.

The menu therefore groups by model: **Blitz ▸** and **Rapid ▸**, where the group *is* the
weights file. Direct map, no mapping table to drift.

Maia 3 (Chessformer) is the strongest model and is where CSSLab's research now goes, so the
temptation is real. It fails on three independent counts, any one of which is disqualifying:

1. **AGPL-3.0.** Stricter than Maia 1's GPL and Maia 2's MIT — the licensing tightened as
   the work improved. Incorporating it would relicense the whole of Bear.
2. **No export path.** No ONNX, no TorchScript, no C++. Embedding is impossible, so the only
   arm's-length option is shipping a Python interpreter and a PyTorch wheel — hundreds of
   megabytes — beside the exe. That doesn't bend the single-file constraint, it destroys it.
3. **Its own authors don't ship it.** CSSLab's production site still serves Maia 1 as ONNX.
   They faced this exact trade-off and made this exact call.

Network weights are **not committed** (see `.gitignore`) — large, and separately licensed.

## Done

- [x] Solution scaffold, win-x64, single-file publish **verified** (builds, launches, window opens)
- [x] Rules library chosen: **Gera.Chess 1.2.0** — zero dependencies, PGN/FEN built in,
      and its `Moves(Position)` overload is exactly what click-to-move needs.
      (Rejected Rudzoft.ChessLib: still 0.0.x, and drags in ZString plus a .NET 6-era
      `Microsoft.Extensions.ObjectPool`.)
- [x] Interactive board — drag-and-drop **and** click-to-move, legal-move markers
      (dot for quiet moves, disc for captures), last-move and check highlighting
- [x] Merida SVG pieces, embedded
- [x] Twenty luminance-linked board palettes, right-click menu, board flip, PGN to clipboard
- [x] Clocks with increment and six time controls; opponent strength selector
- [x] `Bear.Engine` — UCI wrapper (see M0: it may no longer have a consumer)

## Roadmap

Ordered. Each milestone is independently finishable and leaves the app working.

**M0 — Decide `Bear.Engine`'s fate.** With Maia in-process and analysis out of scope, the
UCI wrapper has no caller. Keep it as a dormant seam for a future conventional engine, or
delete it and recover the option from git history. Cheap either way; decide before M5 so the
wiring doesn't get built around a stub.

**M1 — Promotion picker.** The last rules gap; the board currently auto-queens. Small,
self-contained, and independent of everything below.

**M2 — Palette persistence.** The one thing that survives restart. Hand-rolled JSON at
`%LOCALAPPDATA%\Bear\settings.json`, written on change and read at startup — no settings
framework, and note `ApplicationData.Current` is unavailable to unpackaged apps. Store the
palette *name*, not its colours, so `BoardTheme.All` stays the single source of truth and an
unknown name falls back to `BoardTheme.Default`. Palette only: no rating, no player name, no
window geometry. One key is not an architecture, and it stays that way.

**M2a — 111 palettes on a colour-space grid, plus a custom picker.** Twenty is thin, but the
count was never the hard part. Any large set has to be *navigable*, and evocative names are
not navigation: nobody recalls the difference between Cadet, Powder and Dodger blue, so a
list of 137 poetic labels is a list you read rather than a set you choose from.

So the palettes are generated **as a grid over the three axes colour actually has** —
hue, saturation, lightness. Twelve hue families × three saturation tiers (Muted / Soft /
Rich) × three lightness tiers (Dark / Mid / Light), plus three neutrals: **111 palettes,
every cell filled, no duplicate hexes.** A name like `Blue-Soft-Dark` needs no colour
vocabulary at all — it states its own coordinates.

The lightness tiers are defined by **target luminance, not HSL lightness**, and that
distinction is load-bearing. `tools/gen_palettes.py` binary-searches the lightness that puts
each dark square at 0.155 / 0.245 / 0.350. Grid on HSL lightness instead and "Dark" means
something different for every hue — `Amber-Rich-Dark` solves to L 0.34 while
`Indigo-Rich-Dark` solves to L 0.57, and only because both were pinned to the same luminance
do they look equally dark on screen. All 108 chromatic cells are reachable inside the `Ok`
band; none had to be dropped.

**Two menu levels, not three.** Family, then a **3 × 3 swatch grid** with saturation on one
axis and lightness on the other. Three levels would be four clicks deep for one colour; a
grid shows all nine at once and its layout teaches the taxonomy without explaining it. The
names are labels and tooltips, not the means of getting there — once a swatch is visible,
what it's *called* stops mattering.

**Plus `Custom…`** for unlimited choice. `FromDarkSquare` derives a whole theme from a single
hex, so a `ColorPicker` constrained to the `Ok` band offers infinitely many boards with
nothing enumerated. The 111 are the scannable set; the picker is for fiddlers. Persist either
the palette *name* or the custom hex, whichever produced the current board.

**M3 — Export both models.** `torch.onnx.export` over `from_pretrained("blitz")` *and*
`from_pretrained("rapid")`, once, offline. Call `model.eval()` first or dropout stays live in
the graph. Three inputs: `boards`, `elos_self`, `elos_oppo` — the Elo inputs are *bucket
indices* from `create_elo_dict()`, not raw ratings. Size is accepted rather than fought:
fp32 is fine, and quantisation is only worth revisiting if CPU latency disappoints, since
speed is what players notice. Deliverable: a script under `tools/` plus two gitignored
`.onnx` files that load cleanly in ONNX Runtime. No app code.

**M4 — Encoder and decoder, verified against the Python original.** Board → 18 channels
(twelve piece planes, side to move, four castling rights, one en passant — no history), and
policy logits → a legal move using the `get_all_possible_moves()` index order. Build it as a
console harness and prove it by running `maia2`'s own `inference` over a fixed FEN suite,
requiring identical move choices. Much smaller than the equivalent job on Maia 1's 112-plane
history-carrying input, but still the milestone that de-risks everything after it.

**M5 — Maia plays.** Wire M4 to the board: bucket from the strength selector, model from the
time control, move on the opponent's turn, no delay, no reporting. Pass the selector's bucket
as **both** `elos_self` and `elos_oppo` — see *The opponent*. **Randomise colour** at the
start of each game, and flip the board to suit so the player always faces up the board.
First game against a real human-like opponent.

**M6 — File commands and PGN export.** Group the file operations together at the top of the
right-click menu: New game, Copy PGN (exists), and Save PGN as… Add the Seven Tag Roster
plus the two tags that actually matter here:

- `TimeControl` in PGN seconds-plus-increment form (`600+0`, `180+2`) — the clock decides the
  game's category *and* which model played, so this is the tag that makes an exported game
  reconstructible.
- `WhiteElo`/`BlackElo` for **Maia's side only**, set to its bucket. The player's side is
  omitted: Bear doesn't track a rating, and `?` is the honest value for an unknown one.
- `White`/`Black` as `Player` and `Maia <bucket>`, assigned per the randomised colour.

No database — see *Not in scope*.

**M7 — Game-end and new-game flow.** Result presented, board resettable, clocks and palette
behaving correctly across a fresh game.

### Licence cleanup

These two are independent of M1–M7 and can be done at any time, but both must land before
any public release — until they do, the shipped exe is not honestly MIT.

**M8 — Swap Merida for CBurnett (BSD-3).** The last copyleft asset in the build. Elect the
BSD-3-Clause option, credit Colin M. L. Burnett in `LICENSE-ASSETS.md`, and check legibility
at small board sizes before committing. Look at both sets side by side first — this is a
licence decision with an aesthetic cost, and Merida is the stated preference.

**M9 — Maia 2 weights licence. ✅ Done.** MIT, `Copyright (c) 2024 CSSLab`, asserted in
`LICENSE`, `pyproject.toml` and the README, with no separate terms attached to the
checkpoints (Google Drive, sha256-pinned in `maia2/model.py`). Weights are clear to embed.

**Dropped: the unofficial 2200 net.** It was a workaround for Maia 1's 1900 ceiling.
Maia 2 has a `>=2000` bucket with a licence attached, so the workaround is unnecessary — and
the net's own licence was never established, which means all rights reserved rather than
permissive. Not shipping it. (Architecturally it was sound: uncompressed, byte-identical in
size to the official nine at 1,738,564, same input/policy/value formats.)

### Fixed

**Dragged moves never completed.** `OnPointerReleased` called `ReleasePointerCapture`
first; that raises `PointerCaptureLost` synchronously, whose handler cancelled the drag,
so the drop target was never tested. The drop is now resolved *before* capture is
released. If drag handling is ever reworked, keep that ordering.
