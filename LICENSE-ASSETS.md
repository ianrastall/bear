# Third-party assets

Bear's own source code is **MIT** (see `LICENSE`). That covers the code only. Bundled
artwork and neural-network weights are separately licensed, and this file is the record
of what is actually inside a shipped build.

**Read this before cutting a release.** A build is only as permissive as the most
restrictive thing embedded in it, so the table below — not the MIT header on the source —
describes the artifact users actually download.

## Current state

| Asset | Licence | Embedded today? |
| --- | --- | --- |
| CBurnett pieces | BSD-3-Clause (elected) | **Yes** |
| Maia 2 weights | MIT | Not yet (roadmap M3) |

**A build made today carries no copyleft.** Both embedded and pending assets are permissive,
so the exe is honestly MIT provided the two attribution notices below travel with it.

## Chess pieces

### Shipping — CBurnett, under BSD-3-Clause

`src/Bear.App/Assets/Pieces/cburnett/*.svg`

By **Colin M. L. Burnett**. Retrieved from Wikimedia Commons via
`https://commons.wikimedia.org/wiki/Special:FilePath/Chess_<piece>t45.svg`, twelve files,
renamed to Bear's `{colour}{PIECE}.svg` convention:

| Commons | Bear | | Commons | Bear |
| --- | --- | --- | --- | --- |
| `Chess_klt45.svg` | `wK.svg` | | `Chess_kdt45.svg` | `bK.svg` |
| `Chess_qlt45.svg` | `wQ.svg` | | `Chess_qdt45.svg` | `bQ.svg` |
| `Chess_rlt45.svg` | `wR.svg` | | `Chess_rdt45.svg` | `bR.svg` |
| `Chess_blt45.svg` | `wB.svg` | | `Chess_bdt45.svg` | `bB.svg` |
| `Chess_nlt45.svg` | `wN.svg` | | `Chess_ndt45.svg` | `bN.svg` |
| `Chess_plt45.svg` | `wP.svg` | | `Chess_pdt45.svg` | `bP.svg` |

The artwork is **multi-licensed at the recipient's choice**: CC BY-SA 3.0, GFDL,
**3-clause BSD**, or GPL. **Bear elects the 3-clause BSD option**, which requires the notice
below and nothing else — no copyleft, no source offer, no constraint on Bear's own licence.

Taken from Commons deliberately rather than from Lichess. The artwork is identical, but
Lichess distributes its copy under GPL 2+ — that is *their* election, and it does not bind
ours. The multi-licence offer lives with Cburnett's original upload.

> Copyright (c) Colin M. L. Burnett
>
> Redistribution and use in source and binary forms, with or without modification, are
> permitted provided that the following conditions are met:
>
> 1. Redistributions of source code must retain the above copyright notice, this list of
>    conditions and the following disclaimer.
> 2. Redistributions in binary form must reproduce the above copyright notice, this list of
>    conditions and the following disclaimer in the documentation and/or other materials
>    provided with the distribution.
> 3. The name of the author may not be used to endorse or promote products derived from
>    this software without specific prior written permission.
>
> THIS SOFTWARE IS PROVIDED BY THE AUTHOR "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
> INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
> A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
> DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
> BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
> PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
> IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY
> WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

### Removed — Merida

Previously embedded, from [lichess-org/lila](https://github.com/lichess-org/lila) under
**GPLv2 or later**. Removed at M8: it was the last copyleft asset in the build, and its
source-availability obligation was the only thing making an MIT claim awkward. The
preferred set aesthetically, and dropped anyway — the licence decided it.

### Rejected — Merida_shaded

Visually a slight improvement on Merida, and **AGPL-3.0** — stricter than the Merida it
derives from, and the most restrictive option considered. Not usable here at any quality.

### Rejected — Fritz and other proprietary sets

ChessBase's piece sets are assets of a commercial product with no separate grant for the
artwork. Being shipped inside proprietary software is what makes them unavailable, not
incidental to it.

## Neural-network weights

Not committed (see `.gitignore`) — large, and separately licensed.

### Maia 2 — chosen, MIT

**MIT License, Copyright (c) 2024 CSSLab** — asserted in the repository's `LICENSE`, in
`pyproject.toml` (`License :: OSI Approved :: MIT License`), and in the README's own licence
section. Checkpoints (`blitz_model.pt`, `rapid_model.pt`) are distributed from Google Drive
with sha256 pins in `maia2/model.py` and carry **no separate licence or model card**, so the
repository's MIT terms are the only stated terms.

MIT requires the copyright notice to travel with the work. When the exported `.onnx` is
embedded, reproduce CSSLab's notice here and credit
[maia2](https://github.com/CSSLab/maia2).

### Not used

- **Maia 1** — **GPL-3.0**. Superseded by Maia 2 on licence, encoding simplicity, and rating
  coverage. Would have needed a NOTICE and a contested argument about whether copyleft
  reaches weights loaded as data.
- **Maia 3** — **AGPL-3.0**, and no export path at all. Rejected; see the README.
- **The unofficial 2200 net** — licence **unknown**, which means *all rights reserved*, not
  permissive. Dropped: it existed only to reach past Maia 1's 1900 ceiling, and Maia 2's
  `>=2000` bucket does that with a licence attached.

Weights trained from scratch would be Bear's own to license: GPLv3 does not reach a
program's output, and the Lichess game database is CC0. A viable someday path, not a
prerequisite — see the README roadmap.

## Engines

lc0 and Stockfish are **not** bundled, and lc0 is not compiled as part of this project.
Both are GPL-3.0; bundling either would carry source-offer and attribution duties.
