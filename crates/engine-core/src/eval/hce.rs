//! Hand-crafted evaluation, tapered between the middlegame and endgame.
//! Search and tuning share the same feature extraction.

mod pawn;
mod tuned;

use super::Evaluator;
use crate::attacks::{bishop_attacks, king_attacks, knight_attacks, queen_attacks, rook_attacks};
use crate::{Bitboard, Color, Piece, PieceType, Position, Square};

const TEMPO: i32 = 10;
const PHASE: [i32; 6] = [0, 1, 1, 2, 4, 0];
pub const MAX_PHASE: i32 = 24;

const ISOLATED: usize = PieceType::COUNT * 64;
const DOUBLED: usize = ISOLATED + 1;
const BACKWARD: usize = DOUBLED + 1;
const CONNECTED: usize = BACKWARD + 1;
const PHALANX: usize = CONNECTED + 8;
const PASSED: usize = PHALANX + 8;
const PASSED_CLEAR: usize = PASSED + 8;
const PASSED_BLOCKED: usize = PASSED_CLEAR + 1;
const PASSED_KINGS: usize = PASSED_BLOCKED + 1;
const PASSED_ROOK: usize = PASSED_KINGS + 1;
const BISHOP_PAIR: usize = PASSED_ROOK + 1;
const ROOK_OPEN: usize = BISHOP_PAIR + 1;
const ROOK_SEMIOPEN: usize = ROOK_OPEN + 1;
const KING_PRESSURE: usize = ROOK_SEMIOPEN + 1;
const KING_CHECK: usize = KING_PRESSURE + 1;
const KING_SHELTER: usize = KING_CHECK + 1;
const KING_STORM: usize = KING_SHELTER + 1;
const KING_FILES: usize = KING_STORM + 1;
const MOBILITY: usize = KING_FILES + 1;
const MOBILITY_COUNTS: usize = 28;
const MOBILITY_PIECES: [PieceType; 4] = [
    PieceType::Knight,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::Queen,
];
const TO_MOVE: usize = MOBILITY + MOBILITY_PIECES.len() * MOBILITY_COUNTS;
pub const FEATURE_COUNT: usize = TO_MOVE + 1;

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

/// Middlegame and endgame weights for every evaluation feature.
pub struct Params {
    weights: [[i16; 2]; FEATURE_COUNT],
}

impl Params {
    pub fn weights(&self) -> &[[i16; 2]; FEATURE_COUNT] {
        &self.weights
    }

    const fn with_tuned_weights() -> Self {
        let mut params = Self::untuned();
        let mut i = 0;
        while i < tuned::WEIGHTS.len() {
            let (term, values) = tuned::WEIGHTS[i];
            params.weights[term] = values;
            i += 1;
        }
        params
    }

    #[allow(clippy::manual_range_contains)]
    pub const fn untuned() -> Self {
        let mut weights = [[0; 2]; FEATURE_COUNT];
        let mut piece = 0;
        while piece < PieceType::COUNT {
            let mut square = 0;
            while square < 64 {
                let (mg, eg) = piece_square(
                    PieceType::ALL[piece],
                    (square & 7) as i32,
                    (square >> 3) as i32,
                );
                weights[piece * 64 + square] = [mg as i16, eg as i16];
                square += 1;
            }
            piece += 1;
        }

        weights[ISOLATED] = [-11, -13];
        weights[DOUBLED] = [-12, -18];
        weights[BACKWARD] = [-9, -12];
        let passed_mg = [0, 0, 2, 6, 15, 30, 55, 0];
        let passed_eg = [0, 4, 8, 15, 29, 54, 96, 0];
        let mut rank = 1;
        while rank < 7 {
            weights[CONNECTED + rank] = [(2 + rank * 2) as i16, (rank * 2) as i16];
            weights[PHALANX + rank] = [(rank + 1) as i16, (rank * 2) as i16];
            weights[PASSED + rank] = [passed_mg[rank], passed_eg[rank]];
            rank += 1;
        }
        weights[PASSED_CLEAR] = [1, 3];
        weights[PASSED_BLOCKED] = [-5, -12];
        weights[PASSED_KINGS] = [0, 2];
        weights[PASSED_ROOK] = [8, 16];
        weights[BISHOP_PAIR] = [22, 34];
        weights[ROOK_OPEN] = [21, 10];
        weights[ROOK_SEMIOPEN] = [12, 7];
        weights[KING_PRESSURE] = [2, 0];
        weights[KING_CHECK] = [12, 0];
        weights[KING_SHELTER] = [-5, 0];
        weights[KING_STORM] = [-9, 0];
        weights[KING_FILES] = [-10, 0];
        let center = [4, 5, 6, 9];
        let scale = [[6, 4], [5, 5], [3, 4], [2, 3]];
        let mut kind = 0;
        while kind < MOBILITY_PIECES.len() {
            let mut moves = 0;
            while moves < MOBILITY_COUNTS {
                let reach = moves as i16 - center[kind];
                weights[MOBILITY + kind * MOBILITY_COUNTS + moves] =
                    [scale[kind][0] * reach, scale[kind][1] * reach];
                moves += 1;
            }
            kind += 1;
        }
        weights[TO_MOVE] = [TEMPO as i16; 2];
        Self { weights }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            weights: PARAMS.weights,
        }
    }
}

static PARAMS: Params = Params::with_tuned_weights();

#[inline]
const fn sign(color: Color) -> i32 {
    if matches!(color, Color::White) { 1 } else { -1 }
}

trait Accumulate {
    fn add(&mut self, term: usize, count: i32);
}

/// Signed feature counts and material phase, from White's point of view.
pub struct EvalFeatures {
    pub counts: [i16; FEATURE_COUNT],
    pub phase: i32,
}

impl Accumulate for EvalFeatures {
    fn add(&mut self, term: usize, count: i32) {
        self.counts[term] += count as i16;
    }
}

impl EvalFeatures {
    pub fn new(pos: &Position) -> Self {
        let mut features = Self {
            counts: [0; FEATURE_COUNT],
            phase: 0,
        };
        let pawns = pawn::analyze(pos, &mut features);
        features.phase = board_terms(pos, pawns, &mut features);
        features
    }

    /// White's tapered evaluation using these features.
    #[cfg(test)]
    fn score(&self, params: &Params) -> i32 {
        let mut score = Score {
            params,
            values: [0; 2],
        };
        for (term, &count) in self.counts.iter().enumerate() {
            score.add(term, i32::from(count));
        }
        score.taper(self.phase)
    }
}

struct Score<'a> {
    params: &'a Params,
    values: [i32; 2],
}

impl Accumulate for Score<'_> {
    #[inline]
    fn add(&mut self, term: usize, count: i32) {
        let [mg, eg] = self.params.weights[term];
        self.values[0] += i32::from(mg) * count;
        self.values[1] += i32::from(eg) * count;
    }
}

impl Score<'_> {
    fn taper(&self, phase: i32) -> i32 {
        (self.values[0] * phase + self.values[1] * (MAX_PHASE - phase)) / MAX_PHASE
    }
}

fn attacks(kind: PieceType, sq: Square, occupied: Bitboard) -> Bitboard {
    match kind {
        PieceType::Knight => knight_attacks(sq),
        PieceType::Bishop => bishop_attacks(sq, occupied),
        PieceType::Rook => rook_attacks(sq, occupied),
        PieceType::Queen => queen_attacks(sq, occupied),
        _ => unreachable!("mobility pieces only"),
    }
}

fn forward(color: Color, square: Square, steps: u8) -> Option<Square> {
    let rank = if color == Color::White {
        square.rank().checked_add(steps).filter(|&rank| rank < 8)?
    } else {
        square.rank().checked_sub(steps)?
    };
    Some(Square::from_file_rank(square.file(), rank))
}

fn shelter(pos: &Position, color: Color, score: &mut impl Accumulate) {
    let king = pos.king_square(color);
    let friends = pos.piece_bb(color, PieceType::Pawn);
    let enemies = pos.piece_bb(!color, PieceType::Pawn);
    let first_file = king.file().saturating_sub(1).min(5);
    for file in first_file..first_file + 3 {
        let on_file = Bitboard::file(file);
        let origin = Square::from_file_rank(file, king.rank());
        let mut cover = 4;
        let mut storm = 0;
        for distance in 1..=4 {
            let Some(sq) = forward(color, origin, distance) else {
                break;
            };
            if friends.contains(sq) && cover == 4 {
                cover = i32::from(distance) - 1;
            }
            if enemies.contains(sq) && storm == 0 {
                storm = 5 - i32::from(distance);
            }
        }
        score.add(KING_SHELTER, sign(color) * cover);
        score.add(KING_STORM, sign(color) * storm);
        if (friends & on_file).is_empty() {
            score.add(
                KING_FILES,
                sign(color) * (1 + i32::from((enemies & on_file).is_empty())),
            );
        }
    }
}

fn board_terms(pos: &Position, pawns: pawn::Pawns, score: &mut impl Accumulate) -> i32 {
    let mut phase = 0;
    for piece in Piece::ALL {
        let color = piece.color();
        let kind = piece.piece_type();
        let squares = pos.piece_bb(color, kind);
        phase += PHASE[kind.index()] * squares.count() as i32;
        for sq in squares {
            let relative = if color == Color::White {
                sq.index()
            } else {
                sq.index() ^ 56
            };
            score.add(kind.index() * 64 + relative, sign(color));
        }
    }
    let occupied = pos.occupied();
    let kings = Color::ALL.map(|color| pos.king_square(color));
    let king_rings = kings.map(king_attacks);
    let mut side_attacks = [
        king_rings[0] | pawns.attacks[0],
        king_rings[1] | pawns.attacks[1],
    ];
    let mut by_kind = [[Bitboard::EMPTY; 4]; 2];

    for color in Color::ALL {
        let us = color.index();
        let them = (!color).index();
        let sign = sign(color);
        let friendly = pos.colors(color);
        let pawn_safe = !pawns.attacks[them] & !friendly;
        let own_pawns = pos.piece_bb(color, PieceType::Pawn);
        let enemy_pawns = pos.piece_bb(!color, PieceType::Pawn);
        let mut ring_units = 0;
        for (i, kind) in MOBILITY_PIECES.into_iter().enumerate() {
            for sq in pos.piece_bb(color, kind) {
                let reach = attacks(kind, sq, occupied);
                let mobility = (reach & pawn_safe).count() as usize;
                score.add(
                    MOBILITY + i * MOBILITY_COUNTS + mobility.min(MOBILITY_COUNTS - 1),
                    sign,
                );
                let zone_hits = (reach & king_rings[them] & !friendly).count().min(3);
                ring_units += [2, 2, 3, 5][i] * zone_hits as i32;
                side_attacks[us] |= reach;
                by_kind[us][i] |= reach & !friendly;
                if kind == PieceType::Rook {
                    let file = Bitboard::file(sq.file());
                    if (file & own_pawns).is_empty() {
                        score.add(
                            if (file & enemy_pawns).is_empty() {
                                ROOK_OPEN
                            } else {
                                ROOK_SEMIOPEN
                            },
                            sign,
                        );
                    }
                }
            }
        }
        score.add(KING_PRESSURE, sign * (ring_units * ring_units / 4).min(120));
        if pos.piece_bb(color, PieceType::Bishop).more_than_one() {
            score.add(BISHOP_PAIR, sign);
        }
        shelter(pos, color, score);
    }

    for color in Color::ALL {
        let other = (!color).index();
        for (i, kind) in MOBILITY_PIECES.into_iter().enumerate() {
            let check_squares = attacks(kind, kings[other], occupied);
            let safe = by_kind[color.index()][i] & check_squares & !side_attacks[other];
            score.add(KING_CHECK, sign(color) * safe.count().min(3) as i32);
        }
    }

    pawns.passed_extras(pos, score);
    score.add(TO_MOVE, sign(pos.stm()));
    phase.min(MAX_PHASE)
}

const PAWN_ENTRIES: usize = 16_384;

#[derive(Clone, Copy, Default)]
struct PawnEntry {
    key: u64,
    valid: bool,
    pawns: pawn::Pawns,
    score: [i32; 2],
}

/// The hand-crafted evaluator and its thread-local pawn hash.
pub struct Hce {
    pawns: Box<[PawnEntry]>,
}

impl Default for Hce {
    fn default() -> Self {
        Self {
            pawns: vec![PawnEntry::default(); PAWN_ENTRIES].into_boxed_slice(),
        }
    }
}

impl Evaluator for Hce {
    fn evaluate(&mut self, pos: &Position) -> i32 {
        let key = pos.state().pawn_key;
        let entry = &mut self.pawns[key as usize & (PAWN_ENTRIES - 1)];
        if !entry.valid || entry.key != key {
            let mut score = Score {
                params: &PARAMS,
                values: [0; 2],
            };
            let pawns = pawn::analyze(pos, &mut score);
            *entry = PawnEntry {
                key,
                valid: true,
                pawns,
                score: score.values,
            };
        }
        let mut score = Score {
            params: &PARAMS,
            values: entry.score,
        };
        let phase = board_terms(pos, entry.pawns, &mut score);
        (score.taper(phase) * sign(pos.stm())).clamp(-30_000, 30_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(fen: &str) -> i32 {
        let (pos, _) = Position::from_fen(fen).unwrap();
        Hce::default().evaluate(&pos)
    }

    #[test]
    fn start_position_is_level() {
        assert_eq!(
            eval("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"),
            i32::from(PARAMS.weights[TO_MOVE][0])
        );
        assert_eq!(
            eval("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1"),
            i32::from(PARAMS.weights[TO_MOVE][0])
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
    fn cached_evaluation_matches_tuning_features() {
        let mut eval = Hce::default();
        for fen in [
            "4k3/2p5/8/1P1P4/8/8/1P6/4K3 w - - 0 1",
            "5k2/2p5/8/1P1P4/8/8/1P6/2K5 b - - 0 1",
            "4k3/8/8/8/8/8/8/R3K3 w - - 0 1",
        ] {
            let (pos, _) = Position::from_fen(fen).unwrap();
            assert_eq!(
                eval.evaluate(&pos),
                sign(pos.stm()) * EvalFeatures::new(&pos).score(&PARAMS)
            );
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
