"""Generate M4's ground truth: the policy move list, and golden vectors to test C# against.

Produces two things:

* `src/Bear.App/Assets/maia2-moves.txt` — the 1880 policy moves in index order, embedded in
  Bear. This is *generated, not reimplemented*: the order comes from python-chess's own
  move generation for a lone queen and knight on each square, which is an internal detail
  of that library and not something C# should try to reproduce.

* `tools/golden.json` — for a spread of positions, the exact encoder input and the model's
  actual answer, so the C# port can be checked rather than assumed correct.

The board tensor is entirely 0/1, so golden vectors record the *set* of one-valued cells.
That makes the comparison exact instead of a tolerance.
"""

import json
import os
import sys

MAIA2_REPO = os.environ.get("MAIA2_REPO", r"D:\GitHub\maia2")
sys.path.insert(0, MAIA2_REPO)

import chess  # noqa: E402
import numpy as np  # noqa: E402
import onnxruntime as ort  # noqa: E402

from maia2.utils import (  # noqa: E402
    board_to_tensor,
    create_elo_dict,
    get_all_possible_moves,
    map_to_category,
    mirror_move,
)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MOVES_TXT = os.path.join(REPO, "src", "Bear.App", "Assets", "maia2-moves.txt")
GOLDEN = os.path.join(REPO, "tools", "golden.json")
MODEL = os.path.join(REPO, "artifacts", "models", "maia2-rapid.onnx")

POSITIONS = [
    # opening, white to move
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
    "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3",
    # black to move -- exercises the mirror
    "r1bqkb1r/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
    "rnbqkb1r/pp3ppp/2p1pn2/3p4/2PP4/2N1PN2/PP3PPP/R1BQKB1R b KQkq - 0 6",
    # castling rights partially gone
    "r3k2r/pppq1ppp/2npbn2/2b1p3/2B1P3/2NPBN2/PPPQ1PPP/R3K2R w KQkq - 6 9",
    "r3k2r/pppq1ppp/2npbn2/2b1p3/2B1P3/2NPBN2/PPPQ1PPP/R3K2R b kq - 6 9",
    # no castling at all
    "8/2k5/8/8/8/8/2K5/8 w - - 0 1",
    "8/2k5/8/8/8/8/2K5/8 b - - 0 1",
    # endgames
    "8/5k2/8/8/8/3K4/4P3/8 w - - 0 1",
    "8/8/8/4k3/8/8/4P3/4K3 b - - 0 1",
    "8/P6k/8/8/8/8/6K1/8 w - - 0 1",       # promotion available
    "8/6k1/8/8/8/8/p5K1/8 b - - 0 1",       # promotion available, black
    # en passant live
    "rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3",
    "rnbqkbnr/pppp1ppp/8/8/3pP3/8/PPP2PPP/RNBQKBNR b KQkq e3 0 3",
    # middlegame tactics
    "r2q1rk1/ppp2ppp/2np1n2/2b1p1B1/2B1P3/2NP1N2/PPP2PPP/R2Q1RK1 w - - 0 9",
    "r4rk1/pp2ppbp/2np1np1/q7/3PP3/2N1BN2/PP2BPPP/R2Q1RK1 b - - 0 11",
    # sparse / unusual
    "8/8/8/3k4/8/3K4/8/7R w - - 0 1",
    "4k3/8/8/8/8/8/8/4K2R w K - 0 1",
]

ELOS = [(1100, 1100), (1500, 1500), (1900, 1900), (2200, 1200)]


def encode(fen):
    """Mirror-if-black, then encode. Returns (tensor, mirrored_board, was_mirrored)."""
    mirrored = fen.split(" ")[1] == "b"
    board = chess.Board(fen).mirror() if mirrored else chess.Board(fen)
    return board_to_tensor(board).numpy(), board, mirrored


def main():
    moves = get_all_possible_moves()
    os.makedirs(os.path.dirname(MOVES_TXT), exist_ok=True)
    with open(MOVES_TXT, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(moves) + "\n")
    print(f"wrote {MOVES_TXT}  ({len(moves)} moves)")

    index = {m: i for i, m in enumerate(moves)}
    elo_dict = create_elo_dict()
    session = ort.InferenceSession(MODEL, providers=["CPUExecutionProvider"])

    cases = []
    for fen in POSITIONS:
        tensor, board, mirrored = encode(fen)
        ones = sorted(int(i) for i in np.flatnonzero(tensor.reshape(-1)))
        legal = sorted(index[m.uci()] for m in board.legal_moves)

        per_elo = []
        for elo_self, elo_oppo in ELOS:
            si = map_to_category(elo_self, elo_dict)
            oi = map_to_category(elo_oppo, elo_dict)
            logits = session.run(
                None,
                {
                    "boards": tensor[None, ...],
                    "elo_self": np.array([si], dtype=np.int64),
                    "elo_oppo": np.array([oi], dtype=np.int64),
                },
            )[0][0]

            masked = np.where(np.isin(np.arange(len(moves)), legal), logits, -np.inf)
            order = np.argsort(-masked)[:5]
            top = []
            for i in order:
                uci = moves[i]
                top.append({
                    "index": int(i),
                    "uci_mirrored": uci,
                    "uci": mirror_move(uci) if mirrored else uci,
                    "logit": round(float(masked[i]), 4),
                })
            per_elo.append({
                "elo_self": elo_self, "elo_oppo": elo_oppo,
                "bucket_self": si, "bucket_oppo": oi,
                "top": top,
            })

        cases.append({
            "fen": fen,
            "mirrored": mirrored,
            "ones": ones,
            "legal": legal,
            "by_elo": per_elo,
        })
        print(f"  {fen[:44]:46} mirrored={str(mirrored):5} ones={len(ones):4} legal={len(legal)}")

    with open(GOLDEN, "w", encoding="utf-8", newline="\n") as fh:
        json.dump({"moves": len(moves), "cases": cases}, fh, indent=1)
    print(f"wrote {GOLDEN}  ({len(cases)} positions x {len(ELOS)} rating pairs)")


if __name__ == "__main__":
    main()
