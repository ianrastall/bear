use crate::attacks::rook_attacks;
use crate::{Bitboard, Color, PieceType, Position, Square};

use super::{
    Accumulate, BACKWARD, CONNECTED, DOUBLED, ISOLATED, PASSED, PASSED_BLOCKED, PASSED_CLEAR,
    PASSED_KINGS, PASSED_ROOK, PHALANX,
};

#[derive(Clone, Copy, Default)]
pub(super) struct Pawns {
    pub attacks: [Bitboard; 2],
    passed: [Bitboard; 2],
}

fn attacks(color: Color, pawns: Bitboard) -> Bitboard {
    let sides = pawns.east() | pawns.west();
    match color {
        Color::White => sides.north(),
        Color::Black => sides.south(),
    }
}

fn ahead(color: Color, rank: u8) -> Bitboard {
    match color {
        Color::White => Bitboard(u64::MAX.checked_shl(8 * (u32::from(rank) + 1)).unwrap_or(0)),
        Color::Black => Bitboard((1u64 << (8 * u32::from(rank))) - 1),
    }
}

fn relative_rank(color: Color, square: Square) -> usize {
    usize::from(if color == Color::White {
        square.rank()
    } else {
        7 - square.rank()
    })
}

pub(super) fn analyze(pos: &Position, score: &mut impl Accumulate) -> Pawns {
    let pawns = Color::ALL.map(|color| pos.piece_bb(color, PieceType::Pawn));
    let mut result = Pawns {
        attacks: [
            attacks(Color::White, pawns[0]),
            attacks(Color::Black, pawns[1]),
        ],
        ..Pawns::default()
    };
    for color in Color::ALL {
        let us = color.index();
        let them = (!color).index();
        let sign = super::sign(color);
        for sq in pawns[us] {
            let file = Bitboard::file(sq.file());
            let adjacent = file.east() | file.west();
            let front = ahead(color, sq.rank());
            let neighbors = adjacent & pawns[us];
            let rank = relative_rank(color, sq);
            let doubled = (file & front & pawns[us]).any();
            let connected = result.attacks[us].contains(sq);
            let phalanx = (neighbors & Bitboard::rank(sq.rank())).any();
            score.add(ISOLATED, sign * i32::from(neighbors.is_empty()));
            score.add(DOUBLED, sign * i32::from(doubled));
            score.add(CONNECTED + rank, sign * i32::from(connected));
            score.add(PHALANX + rank, sign * i32::from(phalanx));

            // A pawn with no neighboring pawn able to support its advance.
            let stop = super::forward(color, sq, 1);
            if neighbors.any()
                && (neighbors & !front).is_empty()
                && stop.is_some_and(|target| result.attacks[them].contains(target))
            {
                score.add(BACKWARD, sign);
            }
            if !doubled && (front & (file | adjacent) & pawns[them]).is_empty() {
                score.add(PASSED + rank, sign);
                result.passed[us] |= Bitboard::from(sq);
            }
        }
    }
    result
}

fn distance(a: Square, b: Square) -> i32 {
    i32::from(a.file().abs_diff(b.file()).max(a.rank().abs_diff(b.rank())))
}

impl Pawns {
    pub(super) fn passed_extras(self, pos: &Position, score: &mut impl Accumulate) {
        let occupied = pos.occupied();
        for color in Color::ALL {
            let enemy = !color;
            let sign = super::sign(color);
            for sq in self.passed[color.index()] {
                let Some(stop) = super::forward(color, sq, 1) else {
                    continue;
                };
                let rank = relative_rank(color, sq) as i32;
                let front = ahead(color, sq.rank()) & Bitboard::file(sq.file());
                let clear = (front & occupied).is_empty();
                score.add(PASSED_CLEAR, sign * rank * i32::from(clear));
                score.add(PASSED_BLOCKED, sign * i32::from(occupied.contains(stop)));
                score.add(
                    PASSED_KINGS,
                    sign * rank
                        * (distance(pos.king_square(enemy), stop)
                            - distance(pos.king_square(color), stop)),
                );
                let behind = rook_attacks(sq, occupied) & !front & Bitboard::file(sq.file());
                score.add(
                    PASSED_ROOK,
                    sign * ((behind & pos.piece_bb(color, PieceType::Rook)).count() as i32
                        - (behind & pos.piece_bb(enemy, PieceType::Rook)).count() as i32),
                );
            }
        }
    }
}
