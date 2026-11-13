# Bear

A UCI chess engine written in Rust.

The design is in [docs/architecture.md](docs/architecture.md): board representation, move generation, search, evaluation (hand-crafted first, then NNUE trained on Bear's own games), threading, the UCI loop, verification and the milestone roadmap.

## Status

Milestone M0 (skeleton). The binary answers `uci`, `isready`, `ucinewgame` and `quit`. It does not play chess yet; move generation arrives in M1 and search in M2.

## Building

Requires the Rust toolchain named in `rust-toolchain.toml`; `rustup` installs it on first use.

```
cargo build --release
```

The binary is `target/release/bear` (`bear.exe` on Windows).

## Checks

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Layout

| Path | Contents |
|---|---|
| `crates/engine-core` | `bear_core`: squares, bitboards, pieces, moves, castling rights. Position, move generation and search are added here. |
| `crates/engine-uci` | The `bear` binary: UCI command parsing and the protocol loop. |
| `docs/architecture.md` | Architecture and implementation plan. |

Further crates (`engine-nnue`, `engine-bench`, `perft`, `datagen`) are added in the milestones that give them content.

## History

Before November 2026 this repository held a Python board for playing against Maia. That version remains in the history up to commit `c9b0aed`.

## Licence

MIT. See [LICENSE](LICENSE).
