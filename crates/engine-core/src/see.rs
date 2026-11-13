//! Static exchange evaluation: what a move wins or loses on its destination square
//! once both sides have captured there for as long as it pays.

use crate::attacks::{bishop_attacks, rook_attacks};
use crate::{Bitboard, Move, PieceType, Position};

/// Piece values for exchanges, indexed by [`PieceType::index`]. The king's is never
/// counted: a king that captures cannot itself be taken.
pub const SEE_VALUES: [i32; 6] = [100, 300, 300, 500, 900, 0];

#[inline]
fn value(piece_type: PieceType) -> i32 {
    SEE_VALUES[piece_type.index()]
}

/// The piece that lands on the destination square: the promoted piece for a promotion.
#[inline]
fn arriving_piece(pos: &Position, mv: Move) -> PieceType {
    mv.promotion().unwrap_or_else(|| {
        pos.piece_on(mv.from())
            .map_or(PieceType::Pawn, |piece| piece.piece_type())
    })
}

/// What the move gains before any recapture.
#[inline]
fn immediate_gain(pos: &Position, mv: Move) -> i32 {
    let mut gain = 0;
    if mv.is_en_passant() {
        gain += value(PieceType::Pawn);
    } else if mv.is_capture() {
        gain += pos
            .piece_on(mv.to())
            .map_or(0, |piece| value(piece.piece_type()));
    }
    if let Some(promotion) = mv.promotion() {
        gain += value(promotion) - value(PieceType::Pawn);
    }
    gain
}

/// The occupancy after `mv`, as far as the exchange on its destination is concerned.
#[inline]
fn occupancy_after(pos: &Position, mv: Move) -> Bitboard {
    let mut occupied = pos.occupied().without(mv.from()).with(mv.to());
    if mv.is_en_passant() {
        // The captured pawn stands beside the capturing one, on the destination file.
        let captured = crate::Square::from_file_rank(mv.to().file(), mv.from().rank());
        occupied = occupied.without(captured);
    }
    occupied
}

const BY_VALUE: [PieceType; 6] = [
    PieceType::Pawn,
    PieceType::Knight,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::Queen,
    PieceType::King,
];

impl Position {
    /// Whether the exchange started by `mv` nets the side to move at least
    /// `threshold`. `mv` must be pseudo-legal. Pins are ignored, and so are promotions
    /// by pawns that recapture.
    pub fn see_ge(&self, mv: Move, threshold: i32) -> bool {
        if mv.is_castle() {
            return threshold <= 0;
        }
        let to = mv.to();

        // Assume first that the arriving piece is never recaptured, then that it is
        // lost for nothing; the answer is only open between the two.
        let mut balance = immediate_gain(self, mv) - threshold;
        if balance < 0 {
            return false;
        }
        balance -= value(arriving_piece(self, mv));
        if balance >= 0 {
            return true;
        }

        let diagonal = self.pieces(PieceType::Bishop) | self.pieces(PieceType::Queen);
        let straight = self.pieces(PieceType::Rook) | self.pieces(PieceType::Queen);
        let mut occupied = occupancy_after(self, mv);
        let mut attackers = self.attackers_to(to, occupied) & occupied;
        let mut side = !self.stm();

        loop {
            let mine = attackers & self.colors(side);
            if mine.is_empty() {
                break;
            }
            let mut attacker = PieceType::King;
            for piece_type in BY_VALUE {
                if (mine & self.pieces(piece_type)).any() {
                    attacker = piece_type;
                    break;
                }
            }

            // Capturing uncovers whatever stood behind the capturer.
            occupied = occupied.without((mine & self.pieces(attacker)).first());
            if matches!(
                attacker,
                PieceType::Pawn | PieceType::Bishop | PieceType::Queen
            ) {
                attackers |= bishop_attacks(to, occupied) & diagonal;
            }
            if matches!(attacker, PieceType::Rook | PieceType::Queen) {
                attackers |= rook_attacks(to, occupied) & straight;
            }
            attackers &= occupied;

            side = !side;
            balance = -balance - 1 - value(attacker);
            if balance >= 0 {
                // A king may not capture into an attack; if it just did, its side is
                // the one that has run out of captures.
                if attacker == PieceType::King && (attackers & self.colors(side)).any() {
                    side = !side;
                }
                break;
            }
        }

        // `side` is the one left unable or unwilling to capture.
        side != self.stm()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movegen::{MoveList, find_uci_move, gen_all};
    use crate::{CastlingInfo, Color};

    /// The best `side` can make by capturing on `to`, where a piece worth `victim`
    /// stands, or by declining to. Plain recursion, no shortcuts.
    fn exchange(
        pos: &Position,
        to: crate::Square,
        occupied: Bitboard,
        side: Color,
        victim: i32,
    ) -> i32 {
        let attackers = pos.attackers_to(to, occupied) & occupied;
        let mine = attackers & pos.colors(side);
        for piece_type in BY_VALUE {
            let candidates = mine & pos.pieces(piece_type);
            if candidates.is_empty() {
                continue;
            }
            let after = occupied.without(candidates.first());
            if piece_type == PieceType::King {
                let theirs = pos.attackers_to(to, after) & after & pos.colors(!side);
                if theirs.any() {
                    return 0;
                }
            }
            return (victim - exchange(pos, to, after, !side, value(piece_type))).max(0);
        }
        0
    }

    /// The exact exchange value of `mv`.
    fn see_value(pos: &Position, mv: Move) -> i32 {
        if mv.is_castle() {
            return 0;
        }
        let occupied = occupancy_after(pos, mv);
        immediate_gain(pos, mv)
            - exchange(
                pos,
                mv.to(),
                occupied,
                !pos.stm(),
                value(arriving_piece(pos, mv)),
            )
    }

    fn check(pos: &Position, ci: &CastlingInfo, mv: Move) {
        let exact = see_value(pos, mv);
        let fen = pos.fen(ci);
        assert!(pos.see_ge(mv, exact), "{fen} {mv:?}: {exact} not reached");
        assert!(
            !pos.see_ge(mv, exact + 1),
            "{fen} {mv:?}: more than {exact}"
        );
        for threshold in (-1300..=1300).step_by(100) {
            assert_eq!(
                pos.see_ge(mv, threshold),
                exact >= threshold,
                "{fen} {mv:?}: exact {exact}, threshold {threshold}"
            );
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn agrees_with_exhaustive_exchange_on_random_positions() {
        let mut moves = 0;
        crate::movegen::tests::random_positions(6, 100, |pos, ci| {
            let mut list = MoveList::new();
            gen_all(pos, ci, &mut list);
            for mv in list.iter() {
                check(pos, ci, mv);
                moves += 1;
            }
        });
        assert!(moves > 50_000, "{moves}");
    }

    /// `fen; move; exchange value`, each worked out by hand.
    const CASES: &str = include_str!("see/cases.epd");

    #[test]
    fn hand_worked_cases() {
        let mut count = 0;
        for line in CASES.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split(';').map(str::trim);
            let fen = parts.next().unwrap();
            let text = parts.next().unwrap_or_else(|| panic!("no move: {line}"));
            let expected: i32 = parts
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("no value: {line}"));
            let (pos, ci) = Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen}: {e}"));
            let mv = find_uci_move(&pos, &ci, text).unwrap_or_else(|| panic!("{fen}: no {text}"));
            assert_eq!(see_value(&pos, mv), expected, "{line}");
            check(&pos, &ci, mv);
            count += 1;
        }
        assert!(count >= 50, "{count}");
    }
}
