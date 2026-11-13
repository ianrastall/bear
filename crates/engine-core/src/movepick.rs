//! Hands the search its moves one at a time, most promising first, generating each
//! kind only when the kinds before it have not already produced a cutoff.
//!
//! Not in check, the order is: the transposition-table move, captures and queen
//! promotions that do not lose material, quiet moves, then the losing captures.
//! In check every evasion comes from one list. Moves are pseudo-legal; the caller
//! checks legality.

use crate::movegen::{MoveList, ScoredMove, gen_evasions, gen_noisy, gen_quiet};
use crate::{CastlingInfo, Move, PieceType, Position};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Stage {
    TtMove,
    GenNoisy,
    GoodNoisy,
    GenQuiet,
    Quiet,
    BadNoisy,
    GenEvasions,
    Evasions,
    Done,
}

pub struct MovePicker {
    stage: Stage,
    tt_move: Option<Move>,
    /// Only captures and queen promotions, for the quiescence search.
    noisy_only: bool,
    list: MoveList,
    /// The next move of the current stage.
    index: usize,
    /// `list[..bad]` holds the losing captures set aside for the last stage.
    bad: usize,
}

/// Most valuable victim first, then least valuable attacker; promotions by piece.
fn noisy_score(pos: &Position, mv: Move) -> i32 {
    let mut score = 0;
    if mv.is_capture() {
        let victim = if mv.is_en_passant() {
            PieceType::Pawn
        } else {
            pos.piece_on(mv.to())
                .map_or(PieceType::Pawn, |piece| piece.piece_type())
        };
        let attacker = pos
            .piece_on(mv.from())
            .map_or(PieceType::Pawn, |piece| piece.piece_type());
        score += 1_000 + 16 * (victim.index() as i32 + 1) - attacker.index() as i32;
    }
    if let Some(promotion) = mv.promotion() {
        score += 100 * (promotion.index() as i32 + 1);
    }
    score
}

/// Sorts best first. The sort is stable, so equal scores keep generation order and
/// the search visits the same nodes on every platform.
fn sort(moves: &mut [ScoredMove]) {
    for i in 1..moves.len() {
        let current = moves[i];
        let mut j = i;
        while j > 0 && moves[j - 1].score < current.score {
            moves[j] = moves[j - 1];
            j -= 1;
        }
        moves[j] = current;
    }
}

impl MovePicker {
    /// Every move, for the main search. `tt_move` need not be playable here.
    pub fn new(pos: &Position, ci: &CastlingInfo, tt_move: Option<Move>) -> MovePicker {
        MovePicker {
            stage: Stage::TtMove,
            tt_move: tt_move.filter(|&mv| pos.is_pseudo_legal(mv, ci)),
            noisy_only: false,
            list: MoveList::new(),
            index: 0,
            bad: 0,
        }
    }

    /// For the quiescence search: every evasion in check, otherwise captures and queen
    /// promotions only, losing ones included.
    pub fn quiescence(pos: &Position, ci: &CastlingInfo, tt_move: Option<Move>) -> MovePicker {
        let wanted = |mv: Move| {
            pos.in_check() || mv.is_capture() || mv.promotion() == Some(PieceType::Queen)
        };
        MovePicker {
            stage: Stage::TtMove,
            tt_move: tt_move.filter(|&mv| wanted(mv) && pos.is_pseudo_legal(mv, ci)),
            noisy_only: true,
            list: MoveList::new(),
            index: 0,
            bad: 0,
        }
    }

    /// The next move to try, or `None` when there are no more. `pos` and `ci` must be
    /// the ones the picker was made for.
    pub fn next(&mut self, pos: &Position, ci: &CastlingInfo) -> Option<Move> {
        loop {
            match self.stage {
                Stage::TtMove => {
                    self.stage = if pos.in_check() {
                        Stage::GenEvasions
                    } else {
                        Stage::GenNoisy
                    };
                    if self.tt_move.is_some() {
                        return self.tt_move;
                    }
                }
                Stage::GenNoisy => {
                    gen_noisy(pos, &mut self.list);
                    for scored in self.list.as_mut_slice() {
                        scored.score = noisy_score(pos, scored.mv);
                    }
                    self.stage = Stage::GoodNoisy;
                }
                Stage::GoodNoisy => {
                    let Some(mv) = self.pick_best() else {
                        self.stage = if self.noisy_only {
                            Stage::Done
                        } else {
                            Stage::GenQuiet
                        };
                        continue;
                    };
                    if Some(mv) == self.tt_move {
                        continue;
                    }
                    if !self.noisy_only && !pos.see_ge(mv, 0) {
                        // Everything before `index` has been handed out, so its
                        // slots are free to collect the losing captures.
                        self.list.as_mut_slice()[self.bad].mv = mv;
                        self.bad += 1;
                        continue;
                    }
                    return Some(mv);
                }
                Stage::GenQuiet => {
                    self.index = self.list.len();
                    gen_quiet(pos, ci, &mut self.list);
                    self.stage = Stage::Quiet;
                }
                Stage::Quiet => {
                    let Some(mv) = self.next_in_order(self.list.len()) else {
                        self.index = 0;
                        self.stage = Stage::BadNoisy;
                        continue;
                    };
                    return Some(mv);
                }
                Stage::BadNoisy => {
                    let Some(mv) = self.next_in_order(self.bad) else {
                        self.stage = Stage::Done;
                        continue;
                    };
                    return Some(mv);
                }
                Stage::GenEvasions => {
                    gen_evasions(pos, &mut self.list);
                    for scored in self.list.as_mut_slice() {
                        scored.score = noisy_score(pos, scored.mv);
                    }
                    sort(self.list.as_mut_slice());
                    self.stage = Stage::Evasions;
                }
                Stage::Evasions => {
                    let Some(mv) = self.next_in_order(self.list.len()) else {
                        self.stage = Stage::Done;
                        continue;
                    };
                    return Some(mv);
                }
                Stage::Done => return None,
            }
        }
    }

    /// Moves the best remaining move to the front of the unsearched part of the list
    /// and returns it. The first of equals wins.
    fn pick_best(&mut self) -> Option<Move> {
        let moves = self.list.as_mut_slice();
        if self.index >= moves.len() {
            return None;
        }
        let mut best = self.index;
        for i in self.index + 1..moves.len() {
            if moves[i].score > moves[best].score {
                best = i;
            }
        }
        moves.swap(self.index, best);
        self.index += 1;
        Some(moves[self.index - 1].mv)
    }

    /// The next move before `end` in list order, passing over the table move, which
    /// has already been tried.
    fn next_in_order(&mut self, end: usize) -> Option<Move> {
        let moves = self.list.as_slice();
        while self.index < end {
            let mv = moves[self.index].mv;
            self.index += 1;
            if Some(mv) != self.tt_move {
                return Some(mv);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen::{gen_all, tests::random_positions};

    fn drain(mut picker: MovePicker, pos: &Position, ci: &CastlingInfo) -> Vec<Move> {
        let mut moves = Vec::new();
        while let Some(mv) = picker.next(pos, ci) {
            moves.push(mv);
        }
        moves
    }

    fn sorted(mut moves: Vec<Move>) -> Vec<u16> {
        moves.sort_by_key(|mv| mv.raw());
        moves.into_iter().map(Move::raw).collect()
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn yields_every_generated_move_exactly_once() {
        let mut positions = 0;
        random_positions(4, 100, |pos, ci| {
            let mut all = MoveList::new();
            gen_all(pos, ci, &mut all);
            let expected = sorted(all.iter().collect());

            // Without a table move, with each real move as the table move, and with
            // table moves that are not playable here.
            let mut hints: Vec<Option<Move>> = vec![None, Move::from_raw(0x1234)];
            hints.extend(all.iter().step_by(5).map(Some));
            for hint in hints {
                let moves = drain(MovePicker::new(pos, ci, hint), pos, ci);
                if let Some(mv) = hint.filter(|&mv| all.contains(mv)) {
                    assert_eq!(moves[0], mv, "{}", pos.fen(ci));
                }
                assert_eq!(sorted(moves), expected, "{} {hint:?}", pos.fen(ci));
            }
            positions += 1;
        });
        assert!(positions > 1000);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn quiescence_yields_noisy_moves_or_evasions() {
        random_positions(4, 100, |pos, ci| {
            let mut wanted = MoveList::new();
            if pos.in_check() {
                gen_evasions(pos, &mut wanted);
            } else {
                gen_noisy(pos, &mut wanted);
            }
            let expected = sorted(wanted.iter().collect());
            let mut all = MoveList::new();
            gen_all(pos, ci, &mut all);
            for hint in std::iter::once(None).chain(all.iter().step_by(3).map(Some)) {
                let moves = drain(MovePicker::quiescence(pos, ci, hint), pos, ci);
                assert_eq!(sorted(moves), expected, "{} {hint:?}", pos.fen(ci));
            }
        });
    }

    #[test]
    fn stages_come_in_order() {
        // White can win a pawn with the knight, lose the queen for a pawn, or promote.
        let fen = "4k3/1P6/2p5/3p4/8/4N3/8/3QK3 w - - 0 1";
        let (pos, ci) = Position::from_fen(fen).unwrap();
        let moves = drain(MovePicker::new(&pos, &ci, None), &pos, &ci);
        let text: Vec<String> = moves.iter().map(|mv| mv.uci(false).to_string()).collect();
        // The promotion and nothing else is a winning noisy move.
        assert_eq!(text[0], "b7b8q");
        // Both captures of the defended pawn lose material and come last, the
        // cheaper attacker first.
        assert_eq!(text[text.len() - 2..], ["e3d5", "d1d5"]);
        assert!(moves[1..moves.len() - 2].iter().all(|mv| !mv.is_capture()));

        // A table move comes before everything, whatever it is.
        let hint = moves
            .iter()
            .copied()
            .find(|mv| mv.uci(false).to_string() == "d1d5");
        let with_hint = drain(MovePicker::new(&pos, &ci, hint), &pos, &ci);
        assert_eq!(with_hint[0], hint.unwrap());
        assert_eq!(with_hint.len(), moves.len());
    }
}
