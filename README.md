# Bear

A UCI chess engine written in Rust.

The design is in [docs/architecture.md](docs/architecture.md): board representation, move generation, search, evaluation (hand-crafted first, then NNUE trained on Bear's own games), threading, the UCI loop, verification and the milestone roadmap.

## Status

Milestone M1 (board and move generation). The engine library generates and plays legal moves for standard chess and Chess960, and the `perft` tool verifies it. The `bear` binary still only answers `uci`, `isready`, `ucinewgame` and `quit`; search arrives in M2.

## Building

Requires the Rust toolchain named in `rust-toolchain.toml`; `rustup` installs it on first use.

```
cargo build --release
```

The binaries are `target/release/bear` and `target/release/perft` (`.exe` on Windows).

## Checks

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --release -p perft -- suite crates/perft/suites/standard.epd
cargo run --release -p perft -- suite crates/perft/suites/extended.epd
```

## Perft

```
perft <depth> [fen]                 count positions at <depth>
perft divide <depth> [fen]          the count below each legal move
perft suite <file> [--max-depth N]  check every line of an EPD file
perft fuzz --oracle <engine> [--positions N] [--depth D] [--seed S] [--chess960]
```

`fuzz` plays random games and compares the per-move counts of every position with another UCI engine's `go perft` output. The other engine is only run as a separate program.

## Layout

| Path | Contents |
|---|---|
| `crates/engine-core` | `bear_core`: squares, bitboards, pieces, moves, attack tables, Zobrist keys, position, FEN, move generation, perft. Search is added here. |
| `crates/engine-uci` | The `bear` binary: UCI command parsing and the protocol loop. |
| `crates/perft` | The `perft` binary and its suites. |
| `xtask` | Developer tasks. `cargo xtask find-magics` regenerates the slider magic numbers. |
| `docs/architecture.md` | Architecture and implementation plan. |

Further crates (`engine-nnue`, `engine-bench`, `datagen`) are added in the milestones that give them content.

## History

Before November 2026 this repository held a Python board for playing against Maia. That version remains in the history up to commit `c9b0aed`.

## Licence

MIT. See [LICENSE](LICENSE).
