//! What the search has learned about which quiet moves tend to be good, kept from
//! node to node and from one search to the next.

use crate::{Move, Piece, Position, Square};

/// No history score goes beyond this in either direction.
pub const HISTORY_MAX: i32 = 16_384;

/// Moves `entry` towards `bonus`, the more slowly the closer it already is to the
/// limit on that side, so that it can never pass [`HISTORY_MAX`].
#[inline]
fn gravity<const MAX: i32>(entry: &mut i16, bonus: i32) {
    let bonus = bonus.clamp(-MAX, MAX);
    let old = i32::from(*entry);
    *entry = (old + bonus - old * bonus.abs() / MAX) as i16;
}

/// The reward for a quiet move that caused a cutoff at `depth`.
#[inline]
pub fn bonus(depth: i32) -> i32 {
    (170 * depth - 50).min(1800)
}

/// The penalty for the quiet moves tried before it.
#[inline]
pub fn malus(depth: i32) -> i32 {
    (150 * depth - 30).min(1500)
}

/// Scores for quiet moves by side to move, from and to square, and whether each of
/// those squares is attacked by the opponent.
pub struct History {
    main: Box<[[[[i16; 2]; 2]; 4096]]>,
}

impl History {
    pub fn new() -> History {
        History {
            main: vec![[[[0; 2]; 2]; 4096]; 2].into_boxed_slice(),
        }
    }

    #[inline]
    fn slot(pos: &Position, mv: Move) -> (usize, usize, usize, usize) {
        let threats = pos.state().threats;
        (
            pos.stm().index(),
            mv.from() as usize * 64 + mv.to() as usize,
            threats.contains(mv.from()) as usize,
            threats.contains(mv.to()) as usize,
        )
    }

    /// The score of the quiet move `mv` in `pos`.
    #[inline]
    pub fn quiet(&self, pos: &Position, mv: Move) -> i32 {
        let (stm, squares, from, to) = History::slot(pos, mv);
        self.main[stm][squares][from][to] as i32
    }

    /// Rewards (or, with a negative `bonus`, penalises) the quiet move `mv` in `pos`.
    #[inline]
    pub fn update_quiet(&mut self, pos: &Position, mv: Move, bonus: i32) {
        let (stm, squares, from, to) = History::slot(pos, mv);
        gravity::<HISTORY_MAX>(&mut self.main[stm][squares][from][to], bonus);
    }
}

/// Entries in each correction table.
const CORRECTION_SIZE: usize = 16_384;
/// No correction entry goes beyond this in either direction.
const CORRECTION_MAX: i32 = 1024;
/// The weighted sum of the entries is divided by this, so that entries at their
/// limit correct the evaluation by 256 points.
const CORRECTION_DIVISOR: i32 = 2048;

/// What the static evaluation tends to get wrong, learned from the scores the search
/// returns. Three tables are indexed by hashes of the pawns, of White's other pieces
/// and of Black's other pieces, and a fourth by the piece and square of the move
/// that led to the position; each keeps a separate entry for each side to move.
pub(crate) struct CorrectionHistory {
    structure: [Box<[[i16; 2]]>; 3],
    previous: Box<[[i16; Piece::COUNT * 64]; 2]>,
}

impl CorrectionHistory {
    pub(crate) fn new() -> Self {
        Self {
            structure: std::array::from_fn(|_| vec![[0; 2]; CORRECTION_SIZE].into_boxed_slice()),
            previous: Box::new([[0; Piece::COUNT * 64]; 2]),
        }
    }

    #[inline]
    fn indices(pos: &Position) -> [usize; 3] {
        let state = pos.state();
        [state.pawn_key, state.non_pawn_key[0], state.non_pawn_key[1]]
            .map(|key| key as usize & (CORRECTION_SIZE - 1))
    }

    /// The amount to add to the static evaluation of `pos`, which `previous` led to.
    #[inline]
    pub(crate) fn adjustment(&self, pos: &Position, previous: Option<(Piece, Square)>) -> i32 {
        let stm = pos.stm().index();
        let [pawn, white, black] = Self::indices(pos);
        let continuation = previous.map_or(0, |(piece, square)| {
            i32::from(self.previous[stm][piece.index() * 64 + square.index()])
        });
        (192 * i32::from(self.structure[0][pawn][stm])
            + 96 * i32::from(self.structure[1][white][stm])
            + 96 * i32::from(self.structure[2][black][stm])
            + 128 * continuation)
            / CORRECTION_DIVISOR
    }

    /// Moves the entries for `pos` towards `error`, the search score less the
    /// corrected static evaluation, the more so the deeper the search was.
    pub(crate) fn update(
        &mut self,
        pos: &Position,
        previous: Option<(Piece, Square)>,
        depth: i32,
        error: i32,
    ) {
        let stm = pos.stm().index();
        let delta = (error * depth / 8).clamp(-CORRECTION_MAX / 4, CORRECTION_MAX / 4);
        for (table, slot) in self.structure.iter_mut().zip(Self::indices(pos)) {
            gravity::<CORRECTION_MAX>(&mut table[slot][stm], delta);
        }
        if let Some((piece, square)) = previous {
            gravity::<CORRECTION_MAX>(
                &mut self.previous[stm][piece.index() * 64 + square.index()],
                delta,
            );
        }
    }
}

impl Default for History {
    fn default() -> History {
        History::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen::find_uci_move;

    #[test]
    fn gravity_stays_within_bounds() {
        let mut entry = 0i16;
        for _ in 0..1000 {
            gravity::<HISTORY_MAX>(&mut entry, 1800);
            assert!((0..=HISTORY_MAX).contains(&(entry as i32)));
        }
        assert!(entry as i32 > HISTORY_MAX - 200, "{entry}");
        for _ in 0..1000 {
            gravity::<HISTORY_MAX>(&mut entry, -100_000);
            assert!((-HISTORY_MAX..=HISTORY_MAX).contains(&(entry as i32)));
        }
        assert_eq!(entry as i32, -HISTORY_MAX);
    }

    #[test]
    fn correction_is_kept_per_side_and_bounded() {
        let (pos, ci) = Position::startpos();
        let black_to_move = pos.make(find_uci_move(&pos, &ci, "g1f3").unwrap(), &ci);
        let mut correction = CorrectionHistory::new();
        assert_eq!(correction.adjustment(&pos, None), 0);
        // An error of 100 at depth 8 moves each table entry by 100.
        correction.update(&pos, None, 8, 100);
        assert_eq!(
            correction.adjustment(&pos, None),
            (192 + 96 + 96) * 100 / 2048
        );
        assert_eq!(correction.adjustment(&black_to_move, None), 0);
        // The move that led here has its own entry.
        let previous = Some((Piece::WhiteKnight, Square::F3));
        correction.update(&black_to_move, previous, 8, -100);
        assert_eq!(
            correction.adjustment(&black_to_move, previous),
            -512 * 100 / 2048
        );
        assert_eq!(
            correction.adjustment(&black_to_move, None),
            -384 * 100 / 2048
        );
        // However large the errors, the correction stays within 256.
        for _ in 0..1000 {
            correction.update(&pos, None, 40, 100_000);
        }
        let most = correction.adjustment(&pos, None);
        assert!(most > 0 && most <= 256 * 384 / 512, "{most}");
    }

    #[test]
    fn bonus_and_malus_are_capped() {
        assert_eq!(bonus(1), 120);
        assert_eq!(bonus(40), 1800);
        assert_eq!(malus(1), 120);
        assert_eq!(malus(40), 1500);
    }

    #[test]
    fn scores_are_kept_per_side_and_per_threat() {
        let mut history = History::new();
        let (pos, ci) = Position::startpos();
        let e4 = find_uci_move(&pos, &ci, "e2e4").unwrap();
        let d4 = find_uci_move(&pos, &ci, "d2d4").unwrap();
        history.update_quiet(&pos, e4, 500);
        assert_eq!(history.quiet(&pos, e4), 500);
        assert_eq!(history.quiet(&pos, d4), 0);
        history.update_quiet(&pos, e4, -200);
        assert!(history.quiet(&pos, e4) < 500);

        // The same squares with the pawn's destination attacked are another entry.
        let (attacked, ci) = Position::from_fen("4k3/8/8/3p4/8/8/4P3/4K3 w - - 0 1").unwrap();
        let e4 = find_uci_move(&attacked, &ci, "e2e4").unwrap();
        assert_eq!(history.quiet(&attacked, e4), 0);
    }
}
