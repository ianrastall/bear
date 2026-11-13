//! Hand-crafted evaluation: material and piece-square tables, tapered between a
//! middlegame and an endgame score by the material left on the board.
//!
//! The tables are computed from the simple rules below, written for this engine. They
//! exist to validate the search and to label the first training data; the tuner in a
//! later milestone replaces the numbers.

use super::Evaluator;
use crate::{Color, Piece, PieceType, Position};

/// Bonus for having the move.
const TEMPO: i32 = 10;

/// Game phase weight of each piece type; the starting position totals 24.
const PHASE: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// Middlegame and endgame value of a piece on a square, from White's side of the board.
const fn piece_square(piece_type: PieceType, file: i32, rank: i32) -> (i32, i32) {
    // Distance from the nearest edge, 0 to 3, along each axis.
    let file_centre = if file < 4 { file } else { 7 - file };
    let rank_centre = if rank < 4 { rank } else { 7 - rank };
    let centre = file_centre + rank_centre;

    match piece_type {
        PieceType::Pawn => {
            const ADVANCE_MG: [i32; 8] = [0, 0, 2, 6, 12, 24, 50, 0];
            const ADVANCE_EG: [i32; 8] = [0, 0, 5, 12, 25, 50, 90, 0];
            // Centre pawns are worth more once they have left the second rank.
            let centre_files = if rank >= 2 && rank <= 4 {
                match file_centre {
                    3 => 10,
                    2 => 4,
                    _ => 0,
                }
            } else {
                0
            };
            (
                90 + ADVANCE_MG[rank as usize] + centre_files,
                120 + ADVANCE_EG[rank as usize],
            )
        }
        PieceType::Knight => (320 + 8 * centre - 24 + 2 * rank, 300 + 6 * centre - 18),
        PieceType::Bishop => {
            let undeveloped = if rank == 0 { -6 } else { 0 };
            (335 + 5 * centre - 15 + undeveloped, 320 + 4 * centre - 12)
        }
        PieceType::Rook => {
            let seventh = if rank == 6 { 1 } else { 0 };
            (480 + 4 * file_centre + 16 * seventh, 530 + 8 * seventh)
        }
        PieceType::Queen => (960 + 2 * centre - 6, 970 + 6 * centre - 18),
        PieceType::King => {
            // Stay behind the pawns near a corner in the middlegame; walk to the
            // centre in the endgame.
            const SHELTER: [i32; 8] = [10, 16, 4, -10, -10, 4, 16, 10];
            (SHELTER[file as usize] - 14 * rank, 10 * centre - 30)
        }
    }
}

struct Tables {
    /// Indexed by piece, then square; Black's values are mirrored and negated.
    mg: [[i32; 64]; 12],
    eg: [[i32; 64]; 12],
}

// `RangeInclusive::contains` is not usable in `const fn`.
#[allow(clippy::manual_range_contains)]
const fn build_tables() -> Tables {
    let mut tables = Tables {
        mg: [[0; 64]; 12],
        eg: [[0; 64]; 12],
    };
    let mut p = 0;
    while p < 12 {
        let piece = Piece::ALL[p];
        let white = matches!(piece.color(), Color::White);
        let mut sq = 0;
        while sq < 64 {
            let file = (sq & 7) as i32;
            let rank = (sq >> 3) as i32;
            let (mg, eg) = piece_square(
                piece.piece_type(),
                file,
                if white { rank } else { 7 - rank },
            );
            tables.mg[p][sq] = if white { mg } else { -mg };
            tables.eg[p][sq] = if white { eg } else { -eg };
            sq += 1;
        }
        p += 1;
    }
    tables
}

static TABLES: Tables = build_tables();

/// The hand-crafted evaluator. It keeps no state between calls.
#[derive(Copy, Clone, Default, Debug)]
pub struct Hce;

impl Hce {
    /// Middlegame score, endgame score and phase, all from White's point of view.
    fn terms(pos: &Position) -> (i32, i32, i32) {
        let mut mg = 0;
        let mut eg = 0;
        let mut phase = 0;
        for piece in Piece::ALL {
            let squares = pos.piece_bb(piece.color(), piece.piece_type());
            phase += PHASE[piece.piece_type().index()] * squares.count() as i32;
            for sq in squares {
                mg += TABLES.mg[piece.index()][sq.index()];
                eg += TABLES.eg[piece.index()][sq.index()];
            }
        }
        (mg, eg, phase.min(MAX_PHASE))
    }
}

impl Evaluator for Hce {
    fn evaluate(&mut self, pos: &Position) -> i32 {
        let (mg, eg, phase) = Hce::terms(pos);
        let white = (mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE;
        let side = match pos.stm() {
            Color::White => white,
            Color::Black => -white,
        };
        side + TEMPO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(fen: &str) -> i32 {
        let (pos, _) = Position::from_fen(fen).unwrap();
        Hce.evaluate(&pos)
    }

    #[test]
    fn start_position_is_level() {
        assert_eq!(
            eval("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
            TEMPO
        );
        assert_eq!(
            eval("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"),
            TEMPO
        );
    }

    #[test]
    fn colour_symmetry() {
        // Each pair is the same position with colours swapped and the board flipped.
        let pairs = [
            (
                "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
                "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
            ),
            (
                "8/8/8/4k3/8/8/3P4/3K4 w - - 0 1",
                "3k4/3p4/8/8/4K3/8/8/8 b - - 0 1",
            ),
        ];
        for (a, b) in pairs {
            assert_eq!(eval(a), eval(b), "{a}");
        }
    }

    #[test]
    fn material_dominates() {
        // White is a queen up, whoever is to move.
        assert!(eval("4k3/8/8/8/8/8/8/3QK3 w - - 0 1") > 800);
        assert!(eval("4k3/8/8/8/8/8/8/3QK3 b - - 0 1") < -800);
        // A knight is worth more than two pawns and less than four.
        let knight = eval("4k3/8/8/8/8/8/8/1N2K3 w - - 0 1");
        assert!(knight > eval("4k3/8/8/8/8/8/PP6/4K3 w - - 0 1"));
        assert!(knight < eval("4k3/8/8/8/8/8/PPPP4/4K3 w - - 0 1"));
    }

    #[test]
    fn tables_prefer_sensible_squares() {
        // A centralised knight beats one in the corner.
        assert!(eval("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1") > eval("4k3/8/8/8/8/8/8/N3K3 w - - 0 1"));
        // An advanced pawn beats one at home, more so in the endgame.
        assert!(eval("4k3/8/4P3/8/8/8/8/4K3 w - - 0 1") > eval("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"));
        // In the middlegame the king is better off castled than in the centre...
        let castled = piece_square(PieceType::King, 6, 0);
        let central = piece_square(PieceType::King, 4, 3);
        assert!(castled.0 > central.0);
        // ...and in the endgame it is the other way round.
        assert!(castled.1 < central.1);
        assert!(eval("4k3/p7/8/8/4K3/8/P7/8 w - - 0 1") > eval("4k3/p7/8/8/8/8/P7/6K1 w - - 0 1"));
    }
}
