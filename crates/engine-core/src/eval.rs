//! The seam between the search and whatever scores positions.
//!
//! The search is generic over [`Evaluator`], so the hand-crafted evaluation here and
//! the neural network that replaces it are both compiled in without dynamic dispatch.

mod hce;

pub use hce::{EvalFeatures, FEATURE_COUNT, Hce, MAX_PHASE, Params};

use crate::Position;

pub trait Evaluator {
    /// Static score of `pos` in centipawns, from the side to move's point of view.
    ///
    /// The result must stay well inside the mate score range.
    fn evaluate(&mut self, pos: &Position) -> i32;

    /// The search is about to start from `root`, which is ply 0 of every line.
    fn set_root(&mut self, _root: &Position) {}

    /// The position at `ply` of the current line is now `child`, made by one move
    /// (or a null move) from `parent` at `ply - 1`.
    fn made(&mut self, _ply: usize, _parent: &Position, _child: &Position) {}

    /// Static score of `pos`, the position at `ply` of the current line. An evaluator
    /// that keeps state along the line can work from the plies before it.
    fn evaluate_at(&mut self, _ply: usize, pos: &Position) -> i32 {
        self.evaluate(pos)
    }
}
