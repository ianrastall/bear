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
| Merida pieces | GPLv2 or later | **Yes** |
| Maia 2 weights | MIT | Not yet (roadmap M3) |

A build made today is therefore **not cleanly MIT**: the Merida pieces carry copyleft.
Closing that gap is roadmap M8, and it is now the *only* thing standing between Bear and an
honestly MIT release.

## Chess pieces

### Shipping now — Merida

`src/Bear.App/Assets/Pieces/merida/*.svg`

Retrieved from [lichess-org/lila](https://github.com/lichess-org/lila) (`public/piece/merida/`),
licensed **GPLv2 or later**. Asset files only — no Lichess source code is compiled in.

Because they are embedded in the executable, distributing a build carries the GPL's
source-availability and attribution obligations for them.

### Planned replacement — CBurnett under BSD-3-Clause

CBurnett is **multi-licensed at the recipient's choice**: CC BY-SA 3.0, GFDL,
3-clause BSD, *or* GPL. Bear elects the **3-clause BSD** option, which requires attribution
here and nothing further — no copyleft, no source offer.

That election is what makes an MIT build honest. Lichess takes the same artwork under
GPL 2+; that is their choice and it does not bind ours.

When the swap lands, replace this section with the BSD-3-Clause text and credit to
Colin M. L. Burnett, and record exactly which files were taken and from where.

### Rejected — Merida_shaded

Visually a slight improvement on Merida, and **AGPL-3.0** — stricter than the Merida it
derives from, and the most restrictive option considered. Not usable here at any quality.

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
