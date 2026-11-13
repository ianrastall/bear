//! The seam between the search and whatever scores positions.
//!
//! The search is generic over [`Evaluator`], so the hand-crafted evaluation here and
//! the neural network that replaces it are both compiled in without dynamic dispatch.

mod hce;

pub use hce::Hce;

use crate::Position;

pub trait Evaluator {
    /// Static score of `pos` in centipawns, from the side to move's point of view.
    ///
    /// The result must stay well inside the mate score range.
    fn evaluate(&mut self, pos: &Position) -> i32;
}
