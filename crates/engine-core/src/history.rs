//! What the search has learned about which quiet moves tend to be good, kept from
//! node to node and from one search to the next.

use crate::{Move, Position};

/// No history score goes beyond this in either direction.
pub const HISTORY_MAX: i32 = 16_384;

/// Moves `entry` towards `bonus`, the more slowly the closer it already is to the
/// limit on that side, so that it can never pass [`HISTORY_MAX`].
#[inline]
fn gravity(entry: &mut i16, bonus: i32) {
    let bonus = bonus.clamp(-HISTORY_MAX, HISTORY_MAX);
    *entry += (bonus - *entry as i32 * bonus.abs() / HISTORY_MAX) as i16;
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
        gravity(&mut self.main[stm][squares][from][to], bonus);
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
            gravity(&mut entry, 1800);
            assert!((0..=HISTORY_MAX).contains(&(entry as i32)));
        }
        assert!(entry as i32 > HISTORY_MAX - 200, "{entry}");
        for _ in 0..1000 {
            gravity(&mut entry, -100_000);
            assert!((-HISTORY_MAX..=HISTORY_MAX).contains(&(entry as i32)));
        }
        assert_eq!(entry as i32, -HISTORY_MAX);
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
