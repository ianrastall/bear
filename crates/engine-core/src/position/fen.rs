//! Reading and writing positions in Forsyth-Edwards Notation.
//!
//! Castling rights are accepted as `KQkq` (the outermost rook on that side of the king)
//! or as rook file letters (`HAha`, Shredder-FEN), which covers Chess960.

use core::fmt;

use super::Position;
use crate::attacks::pawn_attacks;
use crate::castling::{CastleSide, CastlingInfo};
use crate::{Bitboard, CastlingRights, Color, Piece, PieceType, Square};

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FenError {
    /// The piece placement field is missing or malformed.
    Board,
    /// The side to move is missing or is neither `w` nor `b`.
    SideToMove,
    /// The castling field has a character that is not a right.
    Castling,
    /// The en passant field is neither `-` nor a square.
    EnPassant,
    /// A move counter is not a number.
    Counter,
    /// A side does not have exactly one king.
    Kings,
    /// A pawn stands on the first or eighth rank.
    PawnOnBackRank,
    /// A side has more pieces than promotion could ever produce.
    Material,
    /// The side that just moved left its own king in check.
    OpponentInCheck,
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FenError::Board => "piece placement is missing or malformed",
            FenError::SideToMove => "side to move must be w or b",
            FenError::Castling => "castling rights are malformed",
            FenError::EnPassant => "en passant square is malformed",
            FenError::Counter => "move counters must be numbers",
            FenError::Kings => "each side needs exactly one king",
            FenError::PawnOnBackRank => "pawns cannot stand on the first or eighth rank",
            FenError::Material => "a side has more pieces than a game can produce",
            FenError::OpponentInCheck => "the side not to move is in check",
        })
    }
}

impl core::error::Error for FenError {}

impl Position {
    /// The standard starting position.
    pub fn startpos() -> (Position, CastlingInfo) {
        Position::from_fen(START_FEN).expect("the start position is valid")
    }

    /// Parses a FEN string. The two move counters may be omitted.
    ///
    /// Castling rights with no matching king and rook are dropped, and an en passant
    /// square is kept only if a pawn can capture on it, so that equal positions compare
    /// and hash equal.
    pub fn from_fen(fen: &str) -> Result<(Position, CastlingInfo), FenError> {
        let mut fields = fen.split_whitespace();
        let board = fields.next().ok_or(FenError::Board)?;
        let side = fields.next().ok_or(FenError::SideToMove)?;
        let castling = fields.next().unwrap_or("-");
        let en_passant = fields.next().unwrap_or("-");
        let halfmove = fields.next().unwrap_or("0");
        let fullmove = fields.next().unwrap_or("1");

        let mut pos = Position::empty();
        pos.parse_board(board)?;
        pos.check_material()?;

        pos.state.stm = match side {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return Err(FenError::SideToMove),
        };

        let mut ci = CastlingInfo::none();
        pos.parse_castling(castling, &mut ci)?;

        if en_passant != "-" {
            let sq: Square = en_passant.parse().map_err(|_| FenError::EnPassant)?;
            if pos.en_passant_is_usable(sq) {
                pos.state.ep_square = Some(sq);
            }
        }

        let halfmove: u32 = halfmove.parse().map_err(|_| FenError::Counter)?;
        let fullmove: u32 = fullmove.parse().map_err(|_| FenError::Counter)?;
        pos.state.halfmove = halfmove.min(u8::MAX as u32) as u8;
        pos.state.fullmove = fullmove.clamp(1, u16::MAX as u32) as u16;

        (pos.state.key, pos.state.pawn_key, pos.state.non_pawn_key) = pos.compute_keys();
        pos.refresh_check_info();

        let us = pos.state.stm;
        let their_king = pos.king_square(!us);
        if (pos.attackers_to(their_king, pos.occupied()) & pos.colors(us)).any() {
            return Err(FenError::OpponentInCheck);
        }
        debug_assert_eq!(pos.validate(), Ok(()));
        Ok((pos, ci))
    }

    fn parse_board(&mut self, board: &str) -> Result<(), FenError> {
        let mut ranks = 0;
        for (i, row) in board.split('/').enumerate() {
            if i >= 8 {
                return Err(FenError::Board);
            }
            let rank = 7 - i as u8;
            let mut file = 0u8;
            for c in row.chars() {
                if let Some(skip) = c.to_digit(10) {
                    if skip == 0 || skip > 8 {
                        return Err(FenError::Board);
                    }
                    file += skip as u8;
                } else {
                    let piece = Piece::from_char(c).ok_or(FenError::Board)?;
                    if file >= 8 {
                        return Err(FenError::Board);
                    }
                    self.put(piece, Square::from_file_rank(file, rank));
                    file += 1;
                }
                if file > 8 {
                    return Err(FenError::Board);
                }
            }
            if file != 8 {
                return Err(FenError::Board);
            }
            ranks = i + 1;
        }
        if ranks == 8 {
            Ok(())
        } else {
            Err(FenError::Board)
        }
    }

    /// Rejects boards no game can reach by promotion. This also keeps the number of
    /// moves in any accepted position within the move list's capacity.
    fn check_material(&self) -> Result<(), FenError> {
        if (self.pieces(PieceType::Pawn) & (Bitboard::RANK_1 | Bitboard::RANK_8)).any() {
            return Err(FenError::PawnOnBackRank);
        }
        for color in Color::ALL {
            let count = |piece_type| self.piece_bb(color, piece_type).count();
            if count(PieceType::King) != 1 {
                return Err(FenError::Kings);
            }
            let pawns = count(PieceType::Pawn);
            let promoted = count(PieceType::Knight).saturating_sub(2)
                + count(PieceType::Bishop).saturating_sub(2)
                + count(PieceType::Rook).saturating_sub(2)
                + count(PieceType::Queen).saturating_sub(1);
            if pawns > 8 || promoted > 8 - pawns {
                return Err(FenError::Material);
            }
        }
        Ok(())
    }

    fn parse_castling(&mut self, field: &str, ci: &mut CastlingInfo) -> Result<(), FenError> {
        if field == "-" {
            return Ok(());
        }
        for c in field.chars() {
            let color = if c.is_ascii_uppercase() {
                Color::White
            } else {
                Color::Black
            };
            let king = self.king_square(color);
            let back_rank = match color {
                Color::White => 0,
                Color::Black => 7,
            };
            let rooks = self.piece_bb(color, PieceType::Rook) & Bitboard::rank(back_rank);

            let rook = match c.to_ascii_lowercase() {
                // The outermost rook on the king's side.
                'k' => rooks
                    .into_iter()
                    .filter(|rook| rook.file() > king.file())
                    .last(),
                // The outermost rook on the queen's side.
                'q' => rooks.into_iter().find(|rook| rook.file() < king.file()),
                file @ 'a'..='h' => {
                    let sq = Square::from_file_rank(file as u8 - b'a', back_rank);
                    rooks.contains(sq).then_some(sq)
                }
                _ => return Err(FenError::Castling),
            };

            // A right with no king on the back rank or no such rook is dropped.
            if king.rank() == back_rank
                && let Some(rook) = rook
            {
                let side = ci.add(color, king, rook);
                self.state.castling = self.state.castling | CastlingRights::of(color, side);
            }
        }
        Ok(())
    }

    /// Whether `sq` is a real en passant square: a pawn of the side not to move has just
    /// passed over it, and a pawn of the side to move attacks it.
    fn en_passant_is_usable(&self, sq: Square) -> bool {
        let us = self.state.stm;
        let them = !us;
        let (ep_rank, pushed_rank, origin_rank) = match us {
            Color::White => (5, 4, 6),
            Color::Black => (2, 3, 1),
        };
        if sq.rank() != ep_rank {
            return false;
        }
        let pushed = Square::from_file_rank(sq.file(), pushed_rank);
        let origin = Square::from_file_rank(sq.file(), origin_rank);
        self.piece_on(pushed) == Some(Piece::new(them, PieceType::Pawn))
            && self.piece_on(sq).is_none()
            && self.piece_on(origin).is_none()
            && (pawn_attacks(them, sq) & self.piece_bb(us, PieceType::Pawn)).any()
    }

    /// The position as a FEN string.
    ///
    /// Castling rights are written as `KQkq` when the king is on the e-file and the rook
    /// in its standard corner, and as the rook's file letter otherwise.
    pub fn fen(&self, ci: &CastlingInfo) -> String {
        let mut out = String::with_capacity(90);

        for rank in (0..8u8).rev() {
            let mut empty = 0u8;
            for file in 0..8u8 {
                match self.piece_on(Square::from_file_rank(file, rank)) {
                    Some(piece) => {
                        if empty > 0 {
                            out.push((b'0' + empty) as char);
                            empty = 0;
                        }
                        out.push(piece.to_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                out.push((b'0' + empty) as char);
            }
            if rank > 0 {
                out.push('/');
            }
        }

        out.push(' ');
        out.push(match self.state.stm {
            Color::White => 'w',
            Color::Black => 'b',
        });

        out.push(' ');
        let mut any_right = false;
        for color in Color::ALL {
            for side in CastleSide::ALL {
                if !self
                    .state
                    .castling
                    .contains(CastlingRights::of(color, side))
                {
                    continue;
                }
                let Some(rook) = ci.rook_from(color, side) else {
                    continue;
                };
                let corner_file = match side {
                    CastleSide::King => 7,
                    CastleSide::Queen => 0,
                };
                let letter = if self.king_square(color).file() == 4 && rook.file() == corner_file {
                    match side {
                        CastleSide::King => 'k',
                        CastleSide::Queen => 'q',
                    }
                } else {
                    (b'a' + rook.file()) as char
                };
                out.push(match color {
                    Color::White => letter.to_ascii_uppercase(),
                    Color::Black => letter,
                });
                any_right = true;
            }
        }
        if !any_right {
            out.push('-');
        }

        match self.state.ep_square {
            Some(sq) => out.push_str(&format!(" {sq}")),
            None => out.push_str(" -"),
        }
        out.push_str(&format!(" {} {}", self.state.halfmove, self.state.fullmove));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUND_TRIP: [&str; 8] = [
        START_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        "rnbqkbnr/pppp1ppp/8/8/3Pp3/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 3",
        "4k3/8/8/8/8/8/8/4K3 b - - 99 150",
    ];

    #[test]
    fn round_trip() {
        for fen in ROUND_TRIP {
            let (pos, ci) = Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen}: {e}"));
            assert_eq!(pos.validate(), Ok(()), "{fen}");
            assert_eq!(pos.fen(&ci), fen);
        }
    }

    #[test]
    fn counters_are_optional() {
        let (short, _) =
            Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -").unwrap();
        let (full, _) = Position::startpos();
        assert_eq!(short, full);
    }

    #[test]
    fn unusable_en_passant_square_is_dropped() {
        // After 1. e4 no black pawn can capture on e3.
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        let (pos, ci) = Position::from_fen(fen).unwrap();
        assert_eq!(pos.state().ep_square, None);
        assert_eq!(
            pos.fen(&ci),
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
        );

        // A square on the wrong rank, or with no pawn in front of it, is not one either.
        let (pos, _) = Position::from_fen("4k3/8/8/8/3Pp3/8/8/4K3 b - e3 0 1").unwrap();
        assert_eq!(pos.state().ep_square, None);
        let (pos, _) = Position::from_fen("4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 1").unwrap();
        assert_eq!(pos.state().ep_square, Some(Square::D3));
    }

    #[test]
    fn castling_rights_without_pieces_are_dropped() {
        let (pos, _) = Position::from_fen("4k3/8/8/8/8/8/8/4K2R w KQkq - 0 1").unwrap();
        assert_eq!(pos.state().castling, CastlingRights::WHITE_KING);
        let (pos, _) = Position::from_fen("4k3/8/8/8/8/4K3/8/R6R w KQ - 0 1").unwrap();
        assert_eq!(pos.state().castling, CastlingRights::NONE);
    }

    #[test]
    fn chess960_castling_fields() {
        // King on b1 between rooks on a1 and g1, written with file letters.
        let fen = "rk4r1/pppppppp/8/8/8/8/PPPPPPPP/RK4R1 w GAga - 0 1";
        let (pos, ci) = Position::from_fen(fen).unwrap();
        assert_eq!(pos.state().castling, CastlingRights::ALL);
        assert_eq!(
            ci.rook_from(Color::White, CastleSide::King),
            Some(Square::G1)
        );
        assert_eq!(
            ci.rook_from(Color::White, CastleSide::Queen),
            Some(Square::A1)
        );
        assert_eq!(
            ci.rook_from(Color::Black, CastleSide::King),
            Some(Square::G8)
        );
        assert_eq!(pos.fen(&ci), fen);

        // The same rights written as KQkq mean the outermost rooks.
        let (same, same_ci) =
            Position::from_fen("rk4r1/pppppppp/8/8/8/8/PPPPPPPP/RK4R1 w KQkq - 0 1").unwrap();
        assert_eq!(same, pos);
        assert_eq!(same_ci, ci);
    }

    #[test]
    fn malformed_input_is_rejected() {
        let cases = [
            ("", FenError::Board),
            ("8/8/8/8/8/8/8 w - - 0 1", FenError::Board),
            ("8/8/8/8/8/8/8/8/8 w - - 0 1", FenError::Board),
            (
                "rnbqkbnr/pppppppp/9/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                FenError::Board,
            ),
            (
                "rnbqkbnr/pppppppp/7/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                FenError::Board,
            ),
            (
                "rnbqkbnr/ppppxppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                FenError::Board,
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
                FenError::SideToMove,
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1",
                FenError::SideToMove,
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQxq - 0 1",
                FenError::Castling,
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e9 0 1",
                FenError::EnPassant,
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - x 1",
                FenError::Counter,
            ),
            ("8/8/8/8/8/8/8/4K3 w - - 0 1", FenError::Kings),
            ("4k3/8/8/8/8/8/8/3KK3 w - - 0 1", FenError::Kings),
            ("P3k3/8/8/8/8/8/8/4K3 w - - 0 1", FenError::PawnOnBackRank),
            ("4k3/8/8/8/8/8/PPPPPPPP/QQ2K3 w - - 0 1", FenError::Material),
            ("4k3/8/8/8/8/8/8/4K2r b - - 0 1", FenError::OpponentInCheck),
        ];
        for (fen, expected) in cases {
            assert_eq!(Position::from_fen(fen).err(), Some(expected), "{fen:?}");
        }
    }
}
