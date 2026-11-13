//! Hands the search its moves one at a time, most promising first, generating each
//! kind only when the kinds before it have not already produced a cutoff.
//!
//! Not in check, the order is: the transposition-table move, captures and queen
//! promotions that do not lose material, the killer moves, the other quiet moves by
//! their history score, then the losing captures.
//! In check every evasion comes from one list. Moves are pseudo-legal; the caller
//! checks legality.

use crate::history::History;
use crate::movegen::{MoveList, ScoredMove, gen_evasions, gen_noisy, gen_quiet};
use crate::{CastlingInfo, Move, PieceType, Position};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Stage {
    TtMove,
    GenNoisy,
    GoodNoisy,
    Refutations,
    GenQuiet,
    Quiet,
    BadNoisy,
    GenEvasions,
    Evasions,
    Done,
}

/// Quiet moves tried before the rest because they refuted a sibling position.
pub type Refutations = [Option<Move>; 2];

/// Captures and queen promotions: the moves of the first generation stage.
#[inline]
pub fn is_noisy(mv: Move) -> bool {
    mv.is_capture() || mv.promotion() == Some(PieceType::Queen)
}

pub struct MovePicker {
    stage: Stage,
    tt_move: Option<Move>,
    refutations: Refutations,
    /// The next refutation to try.
    refutation: usize,
    /// Only captures and queen promotions, for the quiescence search.
    noisy_only: bool,
    list: MoveList,
    /// The next move of the current stage.
    index: usize,
    /// `list[..bad]` holds the losing captures set aside for the last stage.
    bad: usize,
}

/// Added to the score of a capturing evasion to put it above every quiet one.
const EVASION_NOISY: i32 = 1 << 20;

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
    /// Every move, for the main search. Neither `tt_move` nor the refutations need be
    /// playable here.
    pub fn new(
        pos: &Position,
        ci: &CastlingInfo,
        tt_move: Option<Move>,
        refutations: Refutations,
    ) -> MovePicker {
        MovePicker {
            stage: Stage::TtMove,
            tt_move: tt_move.filter(|&mv| pos.is_pseudo_legal(mv, ci)),
            // In check there is no quiet stage for them to come before.
            refutations: if pos.in_check() {
                [None; 2]
            } else {
                refutations
            },
            refutation: 0,
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
            refutations: [None; 2],
            refutation: 0,
            noisy_only: true,
            list: MoveList::new(),
            index: 0,
            bad: 0,
        }
    }

    /// The next move to try, or `None` when there are no more. `pos` and `ci` must be
    /// the ones the picker was made for.
    pub fn next(&mut self, pos: &Position, ci: &CastlingInfo, history: &History) -> Option<Move> {
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
                            Stage::Refutations
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
                Stage::Refutations => {
                    let Some(&candidate) = self.refutations.get(self.refutation) else {
                        self.stage = Stage::GenQuiet;
                        continue;
                    };
                    let i = self.refutation;
                    self.refutation += 1;
                    // A refutation that cannot be played here is dropped, so that the
                    // quiet stage does not pass over a real move that looks like it.
                    let playable = candidate.filter(|&mv| {
                        Some(mv) != self.tt_move
                            && !self.refutations[..i].contains(&Some(mv))
                            && !is_noisy(mv)
                            && pos.is_pseudo_legal(mv, ci)
                    });
                    self.refutations[i] = playable;
                    if playable.is_some() {
                        return playable;
                    }
                }
                Stage::GenQuiet => {
                    self.index = self.list.len();
                    gen_quiet(pos, ci, &mut self.list);
                    let quiets = &mut self.list.as_mut_slice()[self.index..];
                    for scored in quiets.iter_mut() {
                        scored.score = history.quiet(pos, scored.mv);
                    }
                    sort(quiets);
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
                        // Captures and promotions first, whatever the history says.
                        scored.score = if is_noisy(scored.mv) || scored.mv.is_promotion() {
                            EVASION_NOISY + noisy_score(pos, scored.mv)
                        } else {
                            history.quiet(pos, scored.mv)
                        };
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

    /// The next move before `end` in list order, passing over the table move and the
    /// refutations, which have already been tried.
    fn next_in_order(&mut self, end: usize) -> Option<Move> {
        let moves = self.list.as_slice();
        while self.index < end {
            let mv = moves[self.index].mv;
            self.index += 1;
            if Some(mv) != self.tt_move && !self.refutations.contains(&Some(mv)) {
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

    fn drain_with(
        mut picker: MovePicker,
        pos: &Position,
        ci: &CastlingInfo,
        history: &History,
    ) -> Vec<Move> {
        let mut moves = Vec::new();
        while let Some(mv) = picker.next(pos, ci, history) {
            moves.push(mv);
        }
        moves
    }

    fn drain(picker: MovePicker, pos: &Position, ci: &CastlingInfo) -> Vec<Move> {
        drain_with(picker, pos, ci, &History::new())
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
            // Refutations taken from this position's moves and from elsewhere,
            // sometimes equal to the table move or to each other.
            let mut refutations: Vec<Refutations> = vec![[None; 2], [Move::from_raw(0x0ABC); 2]];
            for (i, mv) in all.iter().enumerate().step_by(4) {
                refutations.push([Some(mv), all.iter().nth(i / 2)]);
            }
            for hint in hints {
                for &pair in &refutations {
                    let moves = drain(MovePicker::new(pos, ci, hint, pair), pos, ci);
                    if let Some(mv) = hint.filter(|&mv| all.contains(mv)) {
                        assert_eq!(moves[0], mv, "{}", pos.fen(ci));
                    }
                    assert_eq!(sorted(moves), expected, "{} {hint:?} {pair:?}", pos.fen(ci));
                }
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
        let moves = drain(MovePicker::new(&pos, &ci, None, [None; 2]), &pos, &ci);
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
        let with_hint = drain(MovePicker::new(&pos, &ci, hint, [None; 2]), &pos, &ci);
        assert_eq!(with_hint[0], hint.unwrap());
        assert_eq!(with_hint.len(), moves.len());

        // Killers come straight after the promotion, in their own order, and a
        // capture offered as a killer is not moved up.
        let find = |text: &str| {
            moves
                .iter()
                .copied()
                .find(|mv| mv.uci(false).to_string() == text)
        };
        let killers = [find("e1f2"), find("e3d5")];
        let with_killers = drain(MovePicker::new(&pos, &ci, None, killers), &pos, &ci);
        assert_eq!(with_killers[1], killers[0].unwrap());
        assert_eq!(with_killers[with_killers.len() - 2], killers[1].unwrap());
        assert_eq!(with_killers.len(), moves.len());

        // History decides the order of the other quiet moves.
        let mut history = History::new();
        let favourite = find("e3c2").unwrap();
        let disliked = find("e1f2").unwrap();
        history.update_quiet(&pos, favourite, 900);
        history.update_quiet(&pos, disliked, -900);
        let picker = MovePicker::new(&pos, &ci, None, [None; 2]);
        let by_history = drain_with(picker, &pos, &ci, &history);
        assert_eq!(by_history[1], favourite);
        assert_eq!(by_history[by_history.len() - 3], disliked);
        assert_eq!(by_history.len(), moves.len());
    }
}
