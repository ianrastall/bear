//! The search: iterative deepening over a fail-soft alpha-beta with a quiescence
//! search at the leaves.
//!
//! [`ThreadData`] holds what one search thread keeps between searches. A search borrows
//! it together with the [`Shared`] state for the duration of one `go`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::eval::Evaluator;
use crate::history::{self, History};
use crate::movegen::{MoveList, gen_all};
use crate::movepick::{MovePicker, Refutations, is_noisy};
use crate::time::{Limits, TimeBudget};
use crate::tt::{Bound, NO_EVAL, TranspositionTable, TtEntry, from_tt, to_tt};
use crate::{CastlingInfo, Move, Piece, PieceType, Position, Square};

/// Deepest ply the search can reach.
pub const MAX_PLY: usize = 240;
/// Score of delivering mate on the spot; mate in `n` plies scores `MATE - n`.
pub const MATE: i32 = 32_000;
/// Scores beyond this magnitude are mate scores.
pub const MATE_BOUND: i32 = MATE - MAX_PLY as i32;
const INFINITY: i32 = MATE + 1;

/// Reverse futility pruning applies up to this depth.
const RFP_MAX_DEPTH: i32 = 8;
/// How far above beta the evaluation must be, per ply of remaining depth.
const RFP_MARGIN: i32 = 75;
/// Null-move pruning applies from this depth.
const NMP_MIN_DEPTH: i32 = 3;
/// From this depth a null-move cutoff is checked by a search without null moves.
const NMP_VERIFY_DEPTH: i32 = 14;
/// One ply of reduction in the fixed-point units of the reduction table.
const LMR_ONE: i32 = 1024;

/// The natural logarithm of `x >= 1`, from additions, multiplications and divisions
/// only, so that every platform computes the same reduction table.
fn ln(x: f64) -> f64 {
    let mut halvings = 0.0;
    let mut m = x;
    while m >= 2.0 {
        m /= 2.0;
        halvings += 1.0;
    }
    // ln m = 2 atanh((m - 1) / (m + 1)), and the argument is below one third.
    let y = (m - 1.0) / (m + 1.0);
    let mut power = y;
    let mut sum = 0.0;
    let mut n = 1.0;
    for _ in 0..40 {
        sum += power / n;
        power *= y * y;
        n += 2.0;
    }
    2.0 * sum + halvings * std::f64::consts::LN_2
}

/// Base late-move reductions by depth and move number, in units of `1 / LMR_ONE` ply.
fn lmr_table() -> Box<[[i32; 64]]> {
    let mut table = vec![[0; 64]; 64].into_boxed_slice();
    for (depth, row) in table.iter_mut().enumerate().skip(1) {
        for (number, entry) in row.iter_mut().enumerate().skip(1) {
            let plies = 0.78 + ln(depth as f64) * ln(number as f64) / 2.47;
            *entry = (plies * LMR_ONE as f64) as i32;
        }
    }
    table
}

/// How many quiet moves a node remembers having tried, for the history penalty.
const MAX_QUIETS: usize = 64;

/// What a search shares with other threads, borrowed for the length of one `go`.
#[derive(Copy, Clone)]
pub struct Shared<'a> {
    /// Set to make a running search return as soon as it can.
    pub stop: &'a AtomicBool,
    pub tt: &'a TranspositionTable,
}

/// What the search knows after completing an iteration.
#[derive(Debug)]
pub struct Report<'a> {
    pub depth: i32,
    /// Deepest ply reached, including quiescence.
    pub seldepth: usize,
    /// Score for the side to move at the root.
    pub score: i32,
    pub nodes: u64,
    pub elapsed: Duration,
    /// Share of the transposition table used by this search, in parts per thousand.
    pub hashfull: usize,
    /// The principal variation, starting with the best move.
    pub pv: &'a [Move],
}

/// The final result of a search.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// `None` when the side to move has no legal move.
    pub best: Option<Move>,
    /// The move expected in reply, when the search saw that far.
    pub ponder: Option<Move>,
    pub score: i32,
    /// Last depth searched to completion.
    pub depth: i32,
    pub nodes: u64,
}

/// Moves to mate (positive when the side to move mates) if `score` is a mate score.
pub fn mate_in(score: i32) -> Option<i32> {
    if score > MATE_BOUND {
        Some((MATE - score + 1) / 2)
    } else if score < -MATE_BOUND {
        Some(-(MATE + score) / 2)
    } else {
        None
    }
}

#[derive(Clone)]
struct PvLine {
    len: usize,
    moves: [Option<Move>; MAX_PLY],
}

impl PvLine {
    const EMPTY: PvLine = PvLine {
        len: 0,
        moves: [None; MAX_PLY],
    };

    /// Becomes `first` followed by `rest`.
    fn set(&mut self, first: Move, rest: &PvLine) {
        let n = rest.len.min(MAX_PLY - 1);
        self.moves[0] = Some(first);
        self.moves[1..=n].copy_from_slice(&rest.moves[..n]);
        self.len = n + 1;
    }
}

/// Everything one search thread owns across searches.
pub struct ThreadData<E: Evaluator> {
    eval: E,
    /// The position at each ply of the current line; index 0 is the root.
    positions: Box<[Position]>,
    pv: Box<[PvLine]>,
    /// For each ply, the last two quiet moves that caused a cutoff there.
    killers: Box<[Refutations]>,
    /// The static evaluation at each ply of the current line, or `NO_EVAL` in check.
    evals: Box<[i32]>,
    /// The piece and target square of the move that led to each ply; `None` at the
    /// root and after a null move.
    moved: Box<[Option<(Piece, Square)>]>,
    lmr: Box<[[i32; 64]]>,
    history: History,
    /// Keys of every position from the start of the game to the current node.
    keys: Vec<u64>,
    castling: CastlingInfo,
}

impl<E: Evaluator> ThreadData<E> {
    pub fn new(eval: E) -> ThreadData<E> {
        let (start, castling) = Position::startpos();
        let keys = vec![start.key()];
        ThreadData {
            eval,
            positions: vec![start; MAX_PLY + 1].into_boxed_slice(),
            pv: vec![PvLine::EMPTY; MAX_PLY + 2].into_boxed_slice(),
            killers: vec![[None; 2]; MAX_PLY + 1].into_boxed_slice(),
            evals: vec![NO_EVAL; MAX_PLY + 1].into_boxed_slice(),
            moved: vec![None; MAX_PLY + 2].into_boxed_slice(),
            lmr: lmr_table(),
            history: History::new(),
            keys,
            castling,
        }
    }

    /// Sets the position to search. `history` holds the keys of every earlier position
    /// in the game, oldest first, and is what repetition detection looks back through.
    pub fn set_position(&mut self, root: &Position, castling: &CastlingInfo, history: &[u64]) {
        self.positions[0].clone_from(root);
        self.castling.clone_from(castling);
        self.keys.clear();
        self.keys.extend_from_slice(history);
        self.keys.push(root.key());
    }

    /// The static evaluation of the position set by [`set_position`](Self::set_position).
    pub fn evaluate_root(&mut self) -> i32 {
        self.eval.evaluate(&self.positions[0])
    }

    /// Searches the current position within `limits`, calling `on_iteration` after each
    /// completed depth. `overhead_ms` is kept back from the clock.
    pub fn search(
        &mut self,
        shared: Shared,
        limits: &Limits,
        overhead_ms: u64,
        mut on_iteration: impl FnMut(&Report),
    ) -> Outcome {
        let start = Instant::now();
        shared.tt.new_search();
        let stm = self.positions[0].stm();
        let budget = TimeBudget::new(limits, stm, overhead_ms);
        // Room for the deepest line plus this search's own pushes.
        self.keys.reserve(MAX_PLY + 1);
        // Killers belong to one tree; the next search starts a different one.
        self.killers.fill([None; 2]);

        let mut worker = Worker {
            td: self,
            shared,
            start,
            hard: budget.hard,
            node_limit: limits.nodes,
            nodes: 0,
            seldepth: 0,
            stopped: false,
            can_stop: false,
            root_best: None,
            nmp_min_ply: 0,
        };

        let root_moves = worker.legal_root_moves();
        let mut outcome = Outcome {
            best: None,
            ponder: None,
            score: 0,
            depth: 0,
            nodes: 0,
        };
        if root_moves == 0 {
            outcome.score = if worker.td.positions[0].in_check() {
                -MATE
            } else {
                0
            };
            return outcome;
        }

        let max_depth = limits
            .depth
            .unwrap_or(MAX_PLY as i32 - 1)
            .clamp(1, MAX_PLY as i32 - 1);
        let mut pv = Vec::with_capacity(MAX_PLY);
        for depth in 1..=max_depth {
            worker.seldepth = 0;
            let score = worker.alpha_beta(0, -INFINITY, INFINITY, depth, false);
            if worker.stopped {
                break;
            }

            let line = &worker.td.pv[0];
            pv.clear();
            pv.extend(line.moves[..line.len].iter().flatten().copied());
            outcome = Outcome {
                best: pv.first().copied(),
                ponder: pv.get(1).copied(),
                score,
                depth,
                nodes: worker.nodes,
            };
            worker.root_best = outcome.best;
            // From here on there is a move to play, so the search may be interrupted.
            worker.can_stop = true;

            let elapsed = start.elapsed();
            on_iteration(&Report {
                depth,
                seldepth: worker.seldepth,
                score,
                nodes: worker.nodes,
                elapsed,
                hashfull: shared.tt.hashfull(),
                pv: &pv,
            });

            if shared.stop.load(Ordering::Relaxed) {
                break;
            }
            if budget.soft.is_some_and(|soft| elapsed >= soft) {
                break;
            }
            // With a clock running, a forced move needs no thought.
            if root_moves == 1 && limits.is_timed(stm) {
                break;
            }
        }
        outcome.nodes = worker.nodes;
        outcome
    }
}

/// One thread's view of one search.
struct Worker<'a, E: Evaluator> {
    td: &'a mut ThreadData<E>,
    shared: Shared<'a>,
    start: Instant,
    hard: Option<Duration>,
    node_limit: Option<u64>,
    nodes: u64,
    seldepth: usize,
    stopped: bool,
    /// False until the first iteration has produced a move.
    can_stop: bool,
    /// Best move of the previous iteration, searched first at the root.
    root_best: Option<Move>,
    /// While a null-move cutoff is being verified, null moves are not tried above
    /// this ply.
    nmp_min_ply: usize,
}

impl<E: Evaluator> Worker<'_, E> {
    fn legal_root_moves(&self) -> usize {
        let pos = &self.td.positions[0];
        let mut list = MoveList::new();
        gen_all(pos, &self.td.castling, &mut list);
        list.iter()
            .filter(|&mv| pos.is_legal(mv, &self.td.castling))
            .count()
    }

    /// Counts a node and decides whether the search has to end.
    #[inline]
    fn enter_node(&mut self, ply: usize) {
        self.nodes += 1;
        self.seldepth = self.seldepth.max(ply);
        if !self.can_stop {
            return;
        }
        if self.node_limit.is_some_and(|limit| self.nodes >= limit) {
            self.stopped = true;
        }
        if self.nodes & 1023 == 0 {
            if self.shared.stop.load(Ordering::Relaxed) {
                self.stopped = true;
            }
            if self.nodes & 2047 == 0 && self.hard.is_some_and(|hard| self.start.elapsed() >= hard)
            {
                self.stopped = true;
            }
        }
    }

    /// Whether the position at `ply` is drawn by repetition, the fifty-move rule or
    /// lack of mating material.
    fn is_draw(&self, ply: usize) -> bool {
        let pos = &self.td.positions[ply];
        let state = pos.state();

        if state.halfmove >= 100 {
            // Checkmate on the hundredth ply still counts.
            return !pos.in_check() || self.has_legal_move(ply);
        }

        let minors = pos.pieces(PieceType::Knight) | pos.pieces(PieceType::Bishop);
        let others = pos.pieces(PieceType::Pawn)
            | pos.pieces(PieceType::Rook)
            | pos.pieces(PieceType::Queen);
        if others.is_empty() && !minors.more_than_one() {
            return true;
        }

        // A position can only repeat one with the same side to move, and nothing
        // before the last irreversible move or null move can recur.
        let keys = &self.td.keys;
        let last = keys.len() - 1;
        let limit = (state.halfmove.min(state.plies_from_null) as usize).min(last);
        let mut seen_before_root = false;
        let mut back = 4;
        while back <= limit {
            if keys[last - back] == keys[last] {
                // Once inside the search tree is enough; before the root it takes two.
                if back <= ply || seen_before_root {
                    return true;
                }
                seen_before_root = true;
            }
            back += 2;
        }
        false
    }

    fn has_legal_move(&self, ply: usize) -> bool {
        let pos = &self.td.positions[ply];
        let mut list = MoveList::new();
        gen_all(pos, &self.td.castling, &mut list);
        list.iter().any(|mv| pos.is_legal(mv, &self.td.castling))
    }

    /// Plays `mv` from the position at `ply` into the next slot of the stack.
    #[inline]
    fn make(&mut self, ply: usize, mv: Move) {
        let (parents, children) = self.td.positions.split_at_mut(ply + 1);
        parents[ply].make_into(mv, &self.td.castling, &mut children[0]);
        self.td.keys.push(children[0].key());
        self.td.moved[ply + 1] = parents[ply]
            .piece_on(mv.from())
            .map(|piece| (piece, mv.to()));
    }

    /// Passes the move: the position at `ply` with the other side to move goes into
    /// the next slot of the stack.
    #[inline]
    fn make_null(&mut self, ply: usize) {
        let (parents, children) = self.td.positions.split_at_mut(ply + 1);
        parents[ply].make_null_into(&mut children[0]);
        self.td.keys.push(children[0].key());
        self.td.moved[ply + 1] = None;
    }

    #[inline]
    fn unmake(&mut self) {
        self.td.keys.pop();
    }

    /// The main search. A node with a window wider than one point is on the principal
    /// variation; everywhere else only the side of the window matters. `cut_node` says
    /// that a cutoff is expected here, which makes its later moves unlikely to matter.
    fn alpha_beta(
        &mut self,
        ply: usize,
        mut alpha: i32,
        mut beta: i32,
        depth: i32,
        cut_node: bool,
    ) -> i32 {
        if depth <= 0 {
            return self.quiescence(ply, alpha, beta);
        }
        self.enter_node(ply);
        if self.stopped {
            return 0;
        }
        self.td.pv[ply].len = 0;

        let root = ply == 0;
        let is_pv = beta - alpha > 1;
        if !root {
            if self.is_draw(ply) {
                // A draw scores a point either side of zero, varying with the node
                // count, so that the search does not see every repetition as equal.
                return 1 - (self.nodes & 2) as i32;
            }
            if ply >= MAX_PLY {
                return self.td.eval.evaluate(&self.td.positions[ply]);
            }
            // No line from here can beat a mate already found closer to the root.
            alpha = alpha.max(-MATE + ply as i32);
            beta = beta.min(MATE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        let key = self.td.positions[ply].key();
        let halfmove = self.td.positions[ply].state().halfmove;
        let in_check = self.td.positions[ply].in_check();
        let mut tt_move = None;
        let mut static_eval = NO_EVAL;
        // The static evaluation, replaced by a table score that is known to be
        // nearer the truth on the side that matters.
        let mut eval = NO_EVAL;
        // Whether this position is, or has been, on a principal variation.
        let mut tt_pv = is_pv;
        let entry = self.shared.tt.probe(key);
        if let Some(entry) = entry {
            tt_move = entry.mv;
            tt_pv |= entry.pv;
            static_eval = entry.eval;
            let score = from_tt(entry.score, ply, halfmove);
            // Near the fifty-move limit the stored score may rest on lines that the
            // rule now cuts short.
            if !is_pv
                && entry.depth >= depth
                && halfmove < 90
                && entry.bound.cuts(score, alpha, beta)
            {
                return score;
            }
        }
        if in_check {
            static_eval = NO_EVAL;
        } else {
            if static_eval == NO_EVAL {
                static_eval = self.td.eval.evaluate(&self.td.positions[ply]);
            }
            eval = static_eval;
            if let Some(entry) = entry {
                let score = from_tt(entry.score, ply, halfmove);
                let nearer = match entry.bound {
                    Bound::Lower => score > eval,
                    Bound::Upper => score < eval,
                    Bound::Exact => true,
                    Bound::None => false,
                };
                if nearer {
                    eval = score;
                }
            }
        }
        self.td.evals[ply] = static_eval;
        // Whether the side to move stands better than it did two plies ago; when it
        // was in check then, four plies ago.
        let improving = !in_check && {
            let before = |plies: usize| {
                (ply >= plies)
                    .then(|| self.td.evals[ply - plies])
                    .filter(|&earlier| earlier != NO_EVAL)
            };
            before(2)
                .or_else(|| before(4))
                .is_none_or(|earlier| static_eval > earlier)
        };

        // Pruning of the whole node, for positions that are not on the principal
        // variation and not in check.
        if !is_pv && !in_check {
            // Reverse futility: so far above beta that a shallow search will not
            // bring it back below.
            if depth <= RFP_MAX_DEPTH
                && eval - RFP_MARGIN * (depth - improving as i32) >= beta
                && eval < MATE_BOUND
                && beta > -MATE_BOUND
            {
                return (eval + beta) / 2;
            }

            // Null move: if the position still holds beta after giving the opponent a
            // free move, a real move will hold it too. Not twice in a row, and not
            // with only pawns, where having to move can be the whole problem.
            if depth >= NMP_MIN_DEPTH
                && eval >= beta
                && static_eval >= beta - 20 * depth + 180
                && self.td.moved[ply].is_some()
                && ply >= self.nmp_min_ply
                && beta > -MATE_BOUND
                && self.td.positions[ply].has_non_pawn_material(self.td.positions[ply].stm())
            {
                let reduction = 4 + depth / 4 + ((eval - beta) / 200).min(3);
                self.make_null(ply);
                let mut score =
                    -self.alpha_beta(ply + 1, -beta, -beta + 1, depth - reduction, !cut_node);
                self.unmake();
                if self.stopped {
                    return 0;
                }
                if score >= beta {
                    // A mate found with a free move in hand is not a mate.
                    if score >= MATE_BOUND {
                        score = beta;
                    }
                    if depth < NMP_VERIFY_DEPTH || self.nmp_min_ply > 0 {
                        return score;
                    }
                    // At high depth, confirm with a reduced search in which this
                    // side may not pass again for most of the way down.
                    self.nmp_min_ply = ply + (3 * (depth - reduction) / 4) as usize;
                    let verified = self.alpha_beta(ply, beta - 1, beta, depth - reduction, false);
                    self.nmp_min_ply = 0;
                    if self.stopped {
                        return 0;
                    }
                    if verified >= beta {
                        return score;
                    }
                }
            }
        }

        if root {
            // The last iteration's choice is at least as good a guide as the table.
            tt_move = self.root_best.or(tt_move);
        }
        let killers = self.td.killers[ply];
        let mut picker =
            MovePicker::new(&self.td.positions[ply], &self.td.castling, tt_move, killers);

        let old_alpha = alpha;
        let mut best = -INFINITY;
        let mut best_move = None;
        let mut legal = 0;
        // Quiet moves searched without causing a cutoff.
        let mut quiets = [None; MAX_QUIETS];
        let mut quiet_count = 0;
        while let Some(mv) =
            picker.next(&self.td.positions[ply], &self.td.castling, &self.td.history)
        {
            if !self.td.positions[ply].is_legal(mv, &self.td.castling) {
                continue;
            }
            legal += 1;
            let history_score = if is_noisy(mv) {
                0
            } else {
                self.td.history.quiet(&self.td.positions[ply], mv)
            };

            self.make(ply, mv);
            let new_depth = depth - 1;
            let gives_check = self.td.positions[ply + 1].in_check();

            // The first move is expected to be best. The others only have to be shown
            // not to beat it, which a window of one point does; one that does beat it
            // on the principal variation is searched again for its real score.
            let mut score = -INFINITY;
            if depth >= 2 && legal > 1 + root as usize + is_pv as usize {
                // Late moves are searched to reduced depth first, and at full depth
                // only if they then look better than expected.
                let mut reduction = self.td.lmr[depth.min(63) as usize][legal.min(63)];
                if is_noisy(mv) {
                    reduction /= 2;
                }
                reduction += LMR_ONE * (!improving) as i32;
                reduction += LMR_ONE * cut_node as i32;
                reduction -= LMR_ONE * is_pv as i32;
                reduction -= LMR_ONE * tt_pv as i32;
                reduction -= LMR_ONE * gives_check as i32;
                reduction -= LMR_ONE * killers.contains(&Some(mv)) as i32;
                reduction += LMR_ONE * tt_move.is_some_and(|mv| mv.is_capture()) as i32;
                // A full history score is worth two plies either way.
                reduction -= history_score * LMR_ONE / 8192;

                let reduced = (new_depth - reduction / LMR_ONE).clamp(1, new_depth);
                score = -self.alpha_beta(ply + 1, -alpha - 1, -alpha, reduced, true);
                if score > alpha && reduced < new_depth {
                    score = -self.alpha_beta(ply + 1, -alpha - 1, -alpha, new_depth, !cut_node);
                }
            } else if !is_pv || legal > 1 {
                score = -self.alpha_beta(ply + 1, -alpha - 1, -alpha, new_depth, !cut_node);
            }
            if is_pv && (legal == 1 || (score > alpha && score < beta)) {
                score = -self.alpha_beta(ply + 1, -beta, -alpha, new_depth, false);
            }
            self.unmake();
            if self.stopped {
                return 0;
            }

            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    best_move = Some(mv);
                    if is_pv {
                        let (line, rest) = self.td.pv.split_at_mut(ply + 1);
                        line[ply].set(mv, &rest[0]);
                    }
                    if score >= beta {
                        if !is_noisy(mv) {
                            self.remember_cutoff(ply, mv, depth, &quiets[..quiet_count]);
                        }
                        break;
                    }
                }
            }
            if !is_noisy(mv) && quiet_count < MAX_QUIETS {
                quiets[quiet_count] = Some(mv);
                quiet_count += 1;
            }
        }

        if legal == 0 {
            return if self.td.positions[ply].in_check() {
                -MATE + ply as i32
            } else {
                0
            };
        }

        let bound = if best >= beta {
            Bound::Lower
        } else if best > old_alpha {
            Bound::Exact
        } else {
            Bound::Upper
        };
        self.shared.tt.store(
            key,
            TtEntry {
                mv: best_move,
                score: to_tt(best, ply),
                eval: static_eval,
                depth,
                bound,
                pv: tt_pv,
            },
        );
        best
    }

    /// Searches captures (or, in check, every evasion) until the position is quiet
    /// enough for the static evaluation to be trusted.
    fn quiescence(&mut self, ply: usize, mut alpha: i32, beta: i32) -> i32 {
        self.enter_node(ply);
        if self.stopped {
            return 0;
        }
        self.td.pv[ply].len = 0;

        if self.is_draw(ply) {
            return 1 - (self.nodes & 2) as i32;
        }
        if ply >= MAX_PLY {
            return self.td.eval.evaluate(&self.td.positions[ply]);
        }

        let is_pv = beta - alpha > 1;
        let key = self.td.positions[ply].key();
        let halfmove = self.td.positions[ply].state().halfmove;
        let mut tt_move = None;
        let mut static_eval = NO_EVAL;
        if let Some(entry) = self.shared.tt.probe(key) {
            tt_move = entry.mv;
            static_eval = entry.eval;
            let score = from_tt(entry.score, ply, halfmove);
            if !is_pv && halfmove < 90 && entry.bound.cuts(score, alpha, beta) {
                return score;
            }
        }

        let in_check = self.td.positions[ply].in_check();
        let old_alpha = alpha;
        let mut best = -INFINITY;
        let mut best_move = None;
        if in_check {
            static_eval = NO_EVAL;
        } else {
            // The side to move may decline every capture and keep the static score.
            if static_eval == NO_EVAL {
                static_eval = self.td.eval.evaluate(&self.td.positions[ply]);
            }
            best = static_eval;
            if best >= beta {
                self.store_quiescence(key, None, best, static_eval, Bound::Lower, ply);
                return best;
            }
            alpha = alpha.max(best);
        }

        let mut picker =
            MovePicker::quiescence(&self.td.positions[ply], &self.td.castling, tt_move);

        let mut legal = 0;
        while let Some(mv) =
            picker.next(&self.td.positions[ply], &self.td.castling, &self.td.history)
        {
            // A capture that loses material will not rescue a position that standing
            // pat does not.
            if !in_check && !self.td.positions[ply].see_ge(mv, 0) {
                continue;
            }
            if !self.td.positions[ply].is_legal(mv, &self.td.castling) {
                continue;
            }
            legal += 1;

            self.make(ply, mv);
            let score = -self.quiescence(ply + 1, -beta, -alpha);
            self.unmake();
            if self.stopped {
                return 0;
            }

            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    best_move = Some(mv);
                    if score >= beta {
                        break;
                    }
                }
            }
        }

        if in_check && legal == 0 {
            return -MATE + ply as i32;
        }

        let bound = if best >= beta {
            Bound::Lower
        } else if best > old_alpha {
            Bound::Exact
        } else {
            Bound::Upper
        };
        self.store_quiescence(key, best_move, best, static_eval, bound, ply);
        best
    }

    /// Notes that the quiet move `mv` refuted the position at `ply` after the quiet
    /// moves in `failed` did not: it becomes a killer for this ply and gains history,
    /// and they lose some.
    fn remember_cutoff(&mut self, ply: usize, mv: Move, depth: i32, failed: &[Option<Move>]) {
        let killers = &mut self.td.killers[ply];
        if killers[0] != Some(mv) {
            killers[1] = killers[0];
            killers[0] = Some(mv);
        }

        let pos = &self.td.positions[ply];
        self.td.history.update_quiet(pos, mv, history::bonus(depth));
        for &earlier in failed.iter().flatten() {
            self.td
                .history
                .update_quiet(pos, earlier, -history::malus(depth));
        }
    }

    fn store_quiescence(
        &self,
        key: u64,
        mv: Option<Move>,
        score: i32,
        static_eval: i32,
        bound: Bound,
        ply: usize,
    ) {
        self.shared.tt.store(
            key,
            TtEntry {
                mv,
                score: to_tt(score, ply),
                eval: static_eval,
                depth: 0,
                bound,
                pv: false,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Hce;
    use crate::movegen::find_uci_move;

    /// The stop flag and a small table for one test.
    struct Env {
        stop: AtomicBool,
        tt: TranspositionTable,
    }

    impl Env {
        fn new() -> Env {
            Env {
                stop: AtomicBool::new(false),
                tt: TranspositionTable::new(1),
            }
        }

        fn shared(&self) -> Shared<'_> {
            Shared {
                stop: &self.stop,
                tt: &self.tt,
            }
        }
    }

    fn search_fen(fen: &str, limits: &Limits) -> (Outcome, ThreadData<Hce>) {
        let (pos, ci) = Position::from_fen(fen).unwrap();
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &[]);
        let outcome = td.search(Env::new().shared(), limits, 0, |_| {});
        (outcome, td)
    }

    fn best(fen: &str, depth: i32) -> String {
        let (outcome, _) = search_fen(fen, &Limits::depth(depth));
        outcome
            .best
            .map_or("none".to_string(), |mv| mv.uci(false).to_string())
    }

    #[test]
    fn reduction_table() {
        for x in [1.0, 1.5, 2.0, 3.0, 10.0, 63.0, 64.0] {
            assert!((ln(x) - f64::ln(x)).abs() < 1e-12, "{x}");
        }
        let table = lmr_table();
        // The first move at any depth and any move at depth 1 get the constant only.
        assert_eq!(table[1][17], 798);
        assert_eq!(table[9][1], 798);
        // 0.78 + ln 8 * ln 8 / 2.47 = 2.53 plies.
        assert_eq!(table[8][8] / 102, 25);
        assert!(table[63][63] > table[8][8] && table[63][63] < 8 * LMR_ONE);
    }

    #[test]
    fn mate_score_conversion() {
        assert_eq!(mate_in(MATE - 1), Some(1));
        assert_eq!(mate_in(MATE - 3), Some(2));
        assert_eq!(mate_in(-MATE + 2), Some(-1));
        assert_eq!(mate_in(-MATE), Some(0));
        assert_eq!(mate_in(250), None);
    }

    #[test]
    fn finds_mate_in_one() {
        // Back-rank mate.
        let (outcome, _) = search_fen("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", &Limits::depth(2));
        assert_eq!(outcome.best.unwrap().uci(false).to_string(), "a1a8");
        assert_eq!(mate_in(outcome.score), Some(1));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn finds_mate_in_two() {
        // Two rooks walk the king down: 1. Ra7 Kg8 (or any) 2. Rb8#.
        let (outcome, _) = search_fen("7k/8/8/8/8/8/R7/1R4K1 w - - 0 1", &Limits::depth(4));
        assert_eq!(mate_in(outcome.score), Some(2));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn sees_being_mated() {
        // Black to move is mated next move whatever it plays.
        let (outcome, _) = search_fen("6k1/R7/1R6/8/8/8/8/6K1 b - - 0 1", &Limits::depth(3));
        assert_eq!(mate_in(outcome.score), Some(-1));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn wins_material() {
        // The queen on d8 is undefended and attacked by the rook.
        assert_eq!(best("3q2k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1", 3), "d1d8");
        // A knight fork of king and queen.
        assert_eq!(best("4k3/8/8/3q4/8/4N3/8/4K3 w - - 0 1", 3), "e3d5");
    }

    #[test]
    fn no_legal_moves() {
        let (mated, _) = search_fen("R6k/6pp/8/8/8/8/8/K7 b - - 0 1", &Limits::depth(3));
        assert_eq!(mated.best, None);
        assert_eq!(mated.score, -MATE);
        let (stalemate, _) = search_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1", &Limits::depth(3));
        assert_eq!(stalemate.best, None);
        assert_eq!(stalemate.score, 0);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn bare_kings_and_lone_minor_are_drawn() {
        for fen in [
            "8/8/4k3/8/8/3NK3/8/8 w - - 0 1",
            "8/8/4k3/8/8/3BK3/8/8 b - - 0 1",
        ] {
            let (outcome, _) = search_fen(fen, &Limits::depth(4));
            assert!(outcome.score.abs() <= 1, "{fen}: {}", outcome.score);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn fifty_move_rule_is_a_draw() {
        // White is a rook up, but the counter runs out on the next move.
        let (outcome, _) = search_fen("7k/8/8/8/8/8/8/R5K1 w - - 99 80", &Limits::depth(4));
        assert!(outcome.score.abs() <= 1, "{}", outcome.score);
        // With the counter reset, the rook counts.
        let (outcome, _) = search_fen("7k/8/8/8/8/8/8/R5K1 w - - 0 80", &Limits::depth(4));
        assert!(outcome.score > 300);
    }

    fn worker<'a>(td: &'a mut ThreadData<Hce>, shared: Shared<'a>) -> Worker<'a, Hce> {
        Worker {
            td,
            shared,
            start: Instant::now(),
            hard: None,
            node_limit: None,
            nodes: 0,
            seldepth: 0,
            stopped: false,
            can_stop: false,
            root_best: None,
            nmp_min_ply: 0,
        }
    }

    /// Shuffles the king's knights out and back `rounds` times from the start position.
    fn shuffle(rounds: usize) -> (Position, CastlingInfo, Vec<u64>) {
        let (mut pos, ci) = Position::startpos();
        let mut history = Vec::new();
        for _ in 0..rounds {
            for text in ["g1f3", "g8f6", "f3g1", "f6g8"] {
                history.push(pos.key());
                pos = pos.make(find_uci_move(&pos, &ci, text).unwrap(), &ci);
            }
        }
        (pos, ci, history)
    }

    #[test]
    fn repetition_inside_the_search_tree_is_a_draw() {
        let (pos, ci) = Position::startpos();
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &[]);
        let env = Env::new();
        let mut worker = worker(&mut td, env.shared());
        for (ply, text) in ["g1f3", "g8f6", "f3g1", "f6g8"].into_iter().enumerate() {
            assert!(!worker.is_draw(ply), "ply {ply}");
            let mv = find_uci_move(&worker.td.positions[ply], &ci, text).unwrap();
            worker.make(ply, mv);
        }
        // Ply 4 repeats the root.
        assert!(worker.is_draw(4));
        worker.unmake();
        assert!(!worker.is_draw(3));
    }

    #[test]
    fn repetition_before_the_root_needs_two_occurrences() {
        let env = Env::new();

        // The start position has occurred once before: not yet a draw.
        let (pos, ci, history) = shuffle(1);
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &history);
        assert!(!worker(&mut td, env.shared()).is_draw(0));

        // It has occurred twice before: this is the third time.
        let (pos, ci, history) = shuffle(2);
        td.set_position(&pos, &ci, &history);
        assert!(worker(&mut td, env.shared()).is_draw(0));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn search_avoids_repeating_when_ahead() {
        // White is a rook up. Returning the rook to a1 would repeat the position for
        // the third time, so the search must pick something else and keep its score.
        let (mut pos, ci) = Position::from_fen("7k/8/8/8/8/8/8/R5K1 w - - 0 1").unwrap();
        let mut history = Vec::new();
        for text in [
            "a1a2", "h8g8", "a2a1", "g8h8", "a1a2", "h8g8", "a2a1", "g8h8", "a1a2", "h8g8",
        ] {
            history.push(pos.key());
            pos = pos.make(find_uci_move(&pos, &ci, text).unwrap(), &ci);
        }
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &history);
        let outcome = td.search(Env::new().shared(), &Limits::depth(3), 0, |_| {});
        assert!(outcome.score > 300, "{}", outcome.score);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn node_limit_is_respected_and_deterministic() {
        let limits = Limits {
            nodes: Some(20_000),
            ..Limits::default()
        };
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let (a, _) = search_fen(fen, &limits);
        let (b, _) = search_fen(fen, &limits);
        assert_eq!(a, b);
        assert!(a.best.is_some());
        assert!(
            a.nodes >= 20_000 && a.nodes < 20_000 + 2 * MAX_PLY as u64,
            "{}",
            a.nodes
        );
    }

    #[test]
    fn stop_flag_still_returns_a_move() {
        let (pos, ci) = Position::startpos();
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &[]);
        let env = Env::new();
        env.stop.store(true, Ordering::Relaxed);
        let outcome = td.search(env.shared(), &Limits::default(), 0, |_| {});
        assert!(outcome.best.is_some());
        assert_eq!(outcome.depth, 1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn reports_every_iteration_with_a_legal_line() {
        let (pos, ci) = Position::startpos();
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &[]);
        let mut depths = Vec::new();
        td.search(Env::new().shared(), &Limits::depth(4), 0, |report| {
            depths.push(report.depth);
            assert!(!report.pv.is_empty());
            let mut line = pos.clone();
            for &mv in report.pv {
                assert!(
                    line.is_pseudo_legal(mv, &ci) && line.is_legal(mv, &ci),
                    "{mv:?}"
                );
                line = line.make(mv, &ci);
            }
        });
        assert_eq!(depths, [1, 2, 3, 4]);
    }
}
