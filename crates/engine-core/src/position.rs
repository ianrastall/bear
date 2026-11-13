//! The board, and the state that changes with every move.
//!
//! A [`Position`] is 192 bytes in three cache lines and is copied to make a move
//! (copy-make); nothing is ever unmade.

mod fen;

pub use fen::{FenError, START_FEN};

use crate::attacks::{
    bishop_attacks, king_attacks, knight_attacks, line, pawn_attacks, rook_attacks,
};
use crate::castling::{CastleSide, CastlingInfo};
use crate::zobrist::KEYS;
use crate::{Bitboard, CastlingRights, Color, Move, Piece, PieceType, Square};

/// Everything about a position that is not the placement of the pieces.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct BoardState {
    /// Zobrist key of the whole position.
    pub key: u64,
    /// Zobrist key of the pawns alone.
    pub pawn_key: u64,
    /// Zobrist key of each colour's pieces other than pawns.
    pub non_pawn_key: [u64; 2],
    /// Enemy pieces giving check to the side to move.
    pub checkers: Bitboard,
    /// Pieces of the side to move that are pinned to their own king.
    pub pinned: Bitboard,
    /// Squares attacked by the opponent, computed with the king of the side to move
    /// taken off the board so that squares behind it on a checking ray are included.
    pub threats: Bitboard,
    /// Set only when a pawn of the side to move attacks the square.
    pub ep_square: Option<Square>,
    pub castling: CastlingRights,
    /// Plies since the last capture or pawn move.
    pub halfmove: u8,
    /// The piece captured by the move that produced this position.
    pub captured: Option<Piece>,
    /// Side to move.
    pub stm: Color,
    /// Plies since the last null move; bounds the repetition scan.
    pub plies_from_null: u8,
    pub fullmove: u16,
}

#[derive(Clone, PartialEq, Eq, Debug)]
#[repr(C, align(64))]
pub struct Position {
    pieces: [Bitboard; 6],
    colors: [Bitboard; 2],
    mailbox: [Option<Piece>; 64],
    state: BoardState,
}

const _: () = assert!(size_of::<BoardState>() == 64);
const _: () = assert!(size_of::<Position>() == 192 && align_of::<Position>() == 64);

impl Position {
    fn empty() -> Position {
        Position {
            pieces: [Bitboard::EMPTY; 6],
            colors: [Bitboard::EMPTY; 2],
            mailbox: [None; 64],
            state: BoardState {
                key: 0,
                pawn_key: 0,
                non_pawn_key: [0; 2],
                checkers: Bitboard::EMPTY,
                pinned: Bitboard::EMPTY,
                threats: Bitboard::EMPTY,
                ep_square: None,
                castling: CastlingRights::NONE,
                halfmove: 0,
                captured: None,
                stm: Color::White,
                plies_from_null: 0,
                fullmove: 1,
            },
        }
    }

    /// All pieces of one type, both colours.
    #[inline]
    pub fn pieces(&self, piece_type: PieceType) -> Bitboard {
        self.pieces[piece_type.index()]
    }

    /// All pieces of one colour.
    #[inline]
    pub fn colors(&self, color: Color) -> Bitboard {
        self.colors[color.index()]
    }

    #[inline]
    pub fn piece_bb(&self, color: Color, piece_type: PieceType) -> Bitboard {
        self.colors[color.index()] & self.pieces[piece_type.index()]
    }

    #[inline]
    pub fn occupied(&self) -> Bitboard {
        self.colors[0] | self.colors[1]
    }

    #[inline]
    pub fn piece_on(&self, sq: Square) -> Option<Piece> {
        self.mailbox[sq.index()]
    }

    #[inline]
    pub fn king_square(&self, color: Color) -> Square {
        self.piece_bb(color, PieceType::King).first()
    }

    #[inline]
    pub fn state(&self) -> &BoardState {
        &self.state
    }

    /// Side to move.
    #[inline]
    pub fn stm(&self) -> Color {
        self.state.stm
    }

    #[inline]
    pub fn key(&self) -> u64 {
        self.state.key
    }

    #[inline]
    pub fn checkers(&self) -> Bitboard {
        self.state.checkers
    }

    #[inline]
    pub fn in_check(&self) -> bool {
        self.state.checkers.any()
    }

    /// Pieces of both colours that attack `sq`, given the occupied squares `occ`.
    pub fn attackers_to(&self, sq: Square, occ: Bitboard) -> Bitboard {
        let queens = self.pieces(PieceType::Queen);
        (pawn_attacks(Color::White, sq) & self.piece_bb(Color::Black, PieceType::Pawn))
            | (pawn_attacks(Color::Black, sq) & self.piece_bb(Color::White, PieceType::Pawn))
            | (knight_attacks(sq) & self.pieces(PieceType::Knight))
            | (bishop_attacks(sq, occ) & (self.pieces(PieceType::Bishop) | queens))
            | (rook_attacks(sq, occ) & (self.pieces(PieceType::Rook) | queens))
            | (king_attacks(sq) & self.pieces(PieceType::King))
    }

    #[inline]
    fn put(&mut self, piece: Piece, sq: Square) {
        debug_assert!(self.mailbox[sq.index()].is_none());
        let bb = sq.bb();
        let color = piece.color();
        self.pieces[piece.piece_type().index()] ^= bb;
        self.colors[color.index()] ^= bb;
        self.mailbox[sq.index()] = Some(piece);

        let key = KEYS.piece_square[piece.index()][sq.index()];
        self.state.key ^= key;
        if piece.piece_type() == PieceType::Pawn {
            self.state.pawn_key ^= key;
        } else {
            self.state.non_pawn_key[color.index()] ^= key;
        }
    }

    #[inline]
    fn remove(&mut self, sq: Square) -> Piece {
        let piece = self.mailbox[sq.index()]
            .take()
            .expect("a piece stands on the square");
        let bb = sq.bb();
        let color = piece.color();
        self.pieces[piece.piece_type().index()] ^= bb;
        self.colors[color.index()] ^= bb;

        let key = KEYS.piece_square[piece.index()][sq.index()];
        self.state.key ^= key;
        if piece.piece_type() == PieceType::Pawn {
            self.state.pawn_key ^= key;
        } else {
            self.state.non_pawn_key[color.index()] ^= key;
        }
        piece
    }

    /// The position after `mv`, which must be pseudo-legal and legal here.
    #[inline]
    pub fn make(&self, mv: Move, ci: &CastlingInfo) -> Position {
        let mut child = self.clone();
        child.apply(mv, ci);
        child
    }

    /// Writes the position after `mv` into `out`. `mv` must be pseudo-legal and legal here.
    #[inline]
    pub fn make_into(&self, mv: Move, ci: &CastlingInfo, out: &mut Position) {
        out.clone_from(self);
        out.apply(mv, ci);
    }

    fn apply(&mut self, mv: Move, ci: &CastlingInfo) {
        let us = self.state.stm;
        let them = !us;
        let from = mv.from();
        let to = mv.to();

        self.state.halfmove = self.state.halfmove.saturating_add(1);
        self.state.plies_from_null = self.state.plies_from_null.saturating_add(1);
        self.state.captured = None;
        if let Some(ep) = self.state.ep_square.take() {
            self.state.key ^= KEYS.ep_file[ep.file() as usize];
        }

        if mv.is_castle() {
            // King takes own rook: `to` is the rook's square. Both are lifted before
            // either is placed because in Chess960 their paths can overlap.
            let side = if mv.is_king_castle() {
                CastleSide::King
            } else {
                CastleSide::Queen
            };
            let king = self.remove(from);
            let rook = self.remove(to);
            self.put(
                king,
                Square::from_file_rank(side.king_to_file(), from.rank()),
            );
            self.put(
                rook,
                Square::from_file_rank(side.rook_to_file(), from.rank()),
            );
        } else {
            if mv.is_capture() {
                let captured_on = if mv.is_en_passant() {
                    Square::from_file_rank(to.file(), from.rank())
                } else {
                    to
                };
                self.state.captured = Some(self.remove(captured_on));
                self.state.halfmove = 0;
            }
            let piece = self.remove(from);
            let placed = match mv.promotion() {
                Some(piece_type) => Piece::new(us, piece_type),
                None => piece,
            };
            self.put(placed, to);

            if piece.piece_type() == PieceType::Pawn {
                self.state.halfmove = 0;
                if mv.is_double_push() {
                    let skipped = Square::from_index((from as u8 + to as u8) / 2);
                    let capturers =
                        pawn_attacks(us, skipped) & self.piece_bb(them, PieceType::Pawn);
                    if capturers.any() {
                        self.state.ep_square = Some(skipped);
                        self.state.key ^= KEYS.ep_file[skipped.file() as usize];
                    }
                }
            }
        }

        let old_rights = self.state.castling;
        let new_rights = old_rights & ci.rights_mask(from) & ci.rights_mask(to);
        if new_rights != old_rights {
            self.state.castling = new_rights;
            self.state.key ^= KEYS.castling[old_rights.index()] ^ KEYS.castling[new_rights.index()];
        }

        self.state.stm = them;
        self.state.key ^= KEYS.side;
        if us == Color::Black {
            self.state.fullmove = self.state.fullmove.saturating_add(1);
        }

        self.refresh_check_info();

        #[cfg(debug_assertions)]
        if let Err(problem) = self.validate() {
            panic!("invalid position after {mv:?}: {problem}");
        }
    }

    /// Checkers, pinned pieces and threatened squares for the side to move.
    fn check_info(&self) -> (Bitboard, Bitboard, Bitboard) {
        let us = self.state.stm;
        let them = !us;
        let king = self.king_square(us);
        let occ = self.occupied();
        let ours = self.colors(us);
        let theirs = self.colors(them);

        let queens = self.pieces(PieceType::Queen);
        let pawns = self.pieces(PieceType::Pawn) & theirs;
        let knights = self.pieces(PieceType::Knight) & theirs;
        let diagonal = (self.pieces(PieceType::Bishop) | queens) & theirs;
        let straight = (self.pieces(PieceType::Rook) | queens) & theirs;

        let checkers = (pawn_attacks(us, king) & pawns)
            | (knight_attacks(king) & knights)
            | (bishop_attacks(king, occ) & diagonal)
            | (rook_attacks(king, occ) & straight);

        // Looking from the king through our own pieces finds every enemy slider with
        // only our pieces in the way. Exactly one piece in the way is a pin.
        let mut pinned = Bitboard::EMPTY;
        let snipers =
            (bishop_attacks(king, theirs) & diagonal) | (rook_attacks(king, theirs) & straight);
        for sniper in snipers {
            let blockers = crate::attacks::between(king, sniper) & ours;
            if blockers.any() && !blockers.more_than_one() {
                pinned |= blockers;
            }
        }

        let without_king = occ ^ king.bb();
        let mut threats = match them {
            Color::White => pawns.north().east() | pawns.north().west(),
            Color::Black => pawns.south().east() | pawns.south().west(),
        };
        for sq in knights {
            threats |= knight_attacks(sq);
        }
        for sq in diagonal {
            threats |= bishop_attacks(sq, without_king);
        }
        for sq in straight {
            threats |= rook_attacks(sq, without_king);
        }
        threats |= king_attacks(self.king_square(them));

        (checkers, pinned, threats)
    }

    fn refresh_check_info(&mut self) {
        let (checkers, pinned, threats) = self.check_info();
        self.state.checkers = checkers;
        self.state.pinned = pinned;
        self.state.threats = threats;
    }

    /// Zobrist keys computed from the board alone: whole position, pawns, non-pawns by colour.
    fn compute_keys(&self) -> (u64, u64, [u64; 2]) {
        let mut key = 0;
        let mut pawn_key = 0;
        let mut non_pawn_key = [0; 2];
        for sq in self.occupied() {
            let piece = self.mailbox[sq.index()].expect("occupied squares hold a piece");
            let piece_key = KEYS.piece_square[piece.index()][sq.index()];
            key ^= piece_key;
            if piece.piece_type() == PieceType::Pawn {
                pawn_key ^= piece_key;
            } else {
                non_pawn_key[piece.color().index()] ^= piece_key;
            }
        }
        key ^= KEYS.castling[self.state.castling.index()];
        if let Some(ep) = self.state.ep_square {
            key ^= KEYS.ep_file[ep.file() as usize];
        }
        if self.state.stm == Color::Black {
            key ^= KEYS.side;
        }
        (key, pawn_key, non_pawn_key)
    }

    /// Checks every internal invariant against a from-scratch recomputation.
    ///
    /// Debug builds run this after every move.
    pub fn validate(&self) -> Result<(), &'static str> {
        for sq in Bitboard::ALL {
            let mut found = None;
            for piece in Piece::ALL {
                if self
                    .piece_bb(piece.color(), piece.piece_type())
                    .contains(sq)
                {
                    if found.is_some() {
                        return Err("two pieces share a square");
                    }
                    found = Some(piece);
                }
            }
            if found != self.mailbox[sq.index()] {
                return Err("mailbox and bitboards disagree");
            }
        }
        let by_type = self
            .pieces
            .iter()
            .fold(Bitboard::EMPTY, |all, &bb| all | bb);
        if by_type != self.occupied() || (self.colors[0] & self.colors[1]).any() {
            return Err("piece and colour bitboards disagree");
        }
        for color in Color::ALL {
            if self.piece_bb(color, PieceType::King).count() != 1 {
                return Err("each side needs exactly one king");
            }
        }

        let (key, pawn_key, non_pawn_key) = self.compute_keys();
        if key != self.state.key {
            return Err("incremental key differs from recomputation");
        }
        if pawn_key != self.state.pawn_key {
            return Err("incremental pawn key differs from recomputation");
        }
        if non_pawn_key != self.state.non_pawn_key {
            return Err("incremental non-pawn key differs from recomputation");
        }
        if self.check_info() != (self.state.checkers, self.state.pinned, self.state.threats) {
            return Err("checkers, pins or threats are stale");
        }

        let us = self.state.stm;
        let them = !us;
        if (self.attackers_to(self.king_square(them), self.occupied()) & self.colors(us)).any() {
            return Err("the side not to move is in check");
        }
        if let Some(ep) = self.state.ep_square
            && (pawn_attacks(them, ep) & self.piece_bb(us, PieceType::Pawn)).is_empty()
        {
            return Err("en passant square that no pawn can capture on");
        }
        Ok(())
    }

    /// Whether the pseudo-legal move `mv` leaves the mover's king safe.
    ///
    /// `mv` must have come from the move generator or passed
    /// [`is_pseudo_legal`](Position::is_pseudo_legal) for this position.
    #[inline]
    pub fn is_legal(&self, mv: Move, ci: &CastlingInfo) -> bool {
        let us = self.state.stm;
        let from = mv.from();
        let to = mv.to();
        let king = self.king_square(us);

        if mv.is_castle() {
            // `to` is the rook. A rook shielding the king from an attacker on the back
            // rank is pinned, and moving it away would expose the king (Chess960 only).
            let side = if mv.is_king_castle() {
                CastleSide::King
            } else {
                CastleSide::Queen
            };
            return (ci.king_path(us, side) & self.state.threats).is_empty()
                && !self.state.pinned.contains(to);
        }
        if from == king {
            return !self.state.threats.contains(to);
        }
        if mv.is_en_passant() {
            // Two pawns leave the fourth or fifth rank at once, which the pin mask does
            // not model, so look along the king's lines on the board as it will be.
            let them = !us;
            let captured_on = Square::from_file_rank(to.file(), from.rank());
            let occ = (self.occupied() ^ from.bb() ^ captured_on.bb()) | to.bb();
            let queens = self.pieces(PieceType::Queen);
            let diagonal = (self.pieces(PieceType::Bishop) | queens) & self.colors(them);
            let straight = (self.pieces(PieceType::Rook) | queens) & self.colors(them);
            return (bishop_attacks(king, occ) & diagonal).is_empty()
                && (rook_attacks(king, occ) & straight).is_empty();
        }
        !self.state.pinned.contains(from) || line(king, from).contains(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MoveFlag;

    fn play(
        pos: &Position,
        ci: &CastlingInfo,
        from: Square,
        to: Square,
        flag: MoveFlag,
    ) -> Position {
        let mv = Move::new(from, to, flag);
        assert!(pos.is_pseudo_legal(mv, ci), "{mv:?} should be pseudo-legal");
        assert!(pos.is_legal(mv, ci), "{mv:?} should be legal");
        pos.make(mv, ci)
    }

    #[test]
    fn start_position_state() {
        let (pos, _) = Position::startpos();
        assert_eq!(pos.validate(), Ok(()));
        assert_eq!(pos.occupied().count(), 32);
        assert_eq!(pos.stm(), Color::White);
        assert_eq!(pos.king_square(Color::White), Square::E1);
        assert_eq!(pos.king_square(Color::Black), Square::E8);
        assert_eq!(pos.piece_on(Square::D8), Some(Piece::BlackQueen));
        assert_eq!(pos.piece_on(Square::E4), None);
        assert!(!pos.in_check());
        assert_eq!(pos.state().pinned, Bitboard::EMPTY);
        assert_eq!(pos.state().castling, CastlingRights::ALL);
        // Black attacks its own third rank and nothing beyond it.
        assert!(pos.state().threats.contains(Square::A6));
        assert!(!pos.state().threats.contains(Square::A5));
    }

    #[test]
    fn transpositions_share_a_key() {
        let (start, ci) = Position::startpos();
        let a = play(&start, &ci, Square::G1, Square::F3, MoveFlag::Quiet);
        let a = play(&a, &ci, Square::G8, Square::F6, MoveFlag::Quiet);
        let a = play(&a, &ci, Square::B1, Square::C3, MoveFlag::Quiet);

        let b = play(&start, &ci, Square::B1, Square::C3, MoveFlag::Quiet);
        let b = play(&b, &ci, Square::G8, Square::F6, MoveFlag::Quiet);
        let b = play(&b, &ci, Square::G1, Square::F3, MoveFlag::Quiet);

        assert_eq!(a.key(), b.key());
        assert_ne!(a.key(), start.key());

        // Knights out and back: the same placement and side to move as the start.
        let back = play(&a, &ci, Square::F6, Square::G8, MoveFlag::Quiet);
        let back = play(&back, &ci, Square::F3, Square::G1, MoveFlag::Quiet);
        let back = play(&back, &ci, Square::B8, Square::C6, MoveFlag::Quiet);
        let back = play(&back, &ci, Square::C3, Square::B1, MoveFlag::Quiet);
        let back = play(&back, &ci, Square::C6, Square::B8, MoveFlag::Quiet);
        assert_eq!(back.stm(), Color::White);
        assert_eq!(back.key(), start.key());
        assert_eq!(back.state().pawn_key, start.state().pawn_key);
        assert_eq!(back.state().non_pawn_key, start.state().non_pawn_key);
    }

    #[test]
    fn en_passant_square_only_when_capturable() {
        let (start, ci) = Position::startpos();
        let after_e4 = play(&start, &ci, Square::E2, Square::E4, MoveFlag::DoublePush);
        assert_eq!(after_e4.state().ep_square, None);

        let (pos, ci) =
            Position::from_fen("rnbqkbnr/pppp1ppp/8/8/4p3/8/PPPPPPPP/RNBQKBNR w KQkq - 0 3")
                .unwrap();
        let after_d4 = play(&pos, &ci, Square::D2, Square::D4, MoveFlag::DoublePush);
        assert_eq!(after_d4.state().ep_square, Some(Square::D3));

        let taken = play(&after_d4, &ci, Square::E4, Square::D3, MoveFlag::EnPassant);
        assert_eq!(taken.piece_on(Square::D4), None);
        assert_eq!(taken.piece_on(Square::D3), Some(Piece::BlackPawn));
        assert_eq!(taken.state().captured, Some(Piece::WhitePawn));
        assert_eq!(taken.state().ep_square, None);
        assert_eq!(taken.state().halfmove, 0);
    }

    #[test]
    fn castling_moves_both_pieces_and_clears_rights() {
        let (pos, ci) = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();

        let short = play(&pos, &ci, Square::E1, Square::H1, MoveFlag::KingCastle);
        assert_eq!(short.piece_on(Square::G1), Some(Piece::WhiteKing));
        assert_eq!(short.piece_on(Square::F1), Some(Piece::WhiteRook));
        assert_eq!(short.piece_on(Square::E1), None);
        assert_eq!(short.piece_on(Square::H1), None);
        assert_eq!(short.state().castling, CastlingRights::both(Color::Black));

        let long = play(&pos, &ci, Square::E1, Square::A1, MoveFlag::QueenCastle);
        assert_eq!(long.piece_on(Square::C1), Some(Piece::WhiteKing));
        assert_eq!(long.piece_on(Square::D1), Some(Piece::WhiteRook));
        assert_eq!(long.state().castling, CastlingRights::both(Color::Black));

        // Capturing a rook on its home square removes the opponent's right.
        let rook_takes = play(&pos, &ci, Square::A1, Square::A8, MoveFlag::Capture);
        assert_eq!(
            rook_takes.state().castling,
            CastlingRights::WHITE_KING | CastlingRights::BLACK_KING
        );
    }

    #[test]
    fn castling_through_or_into_attack_is_illegal() {
        // The black rook on f8 attacks f1, which the white king must cross.
        let (pos, ci) = Position::from_fen("5rk1/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
        let short = Move::new(Square::E1, Square::H1, MoveFlag::KingCastle);
        let long = Move::new(Square::E1, Square::A1, MoveFlag::QueenCastle);
        assert!(pos.is_pseudo_legal(short, &ci));
        assert!(!pos.is_legal(short, &ci));
        assert!(pos.is_legal(long, &ci));

        // An attack on b1 does not matter: only the rook crosses it.
        let (pos, ci) = Position::from_fen("1r4k1/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
        assert!(pos.is_legal(long, &ci));
    }

    #[test]
    fn pins_and_checks_are_tracked() {
        // The knight on e2 is pinned by the rook on e8.
        let (pos, ci) = Position::from_fen("4r2k/8/8/8/8/8/4N3/4K3 w - - 0 1").unwrap();
        assert_eq!(pos.state().pinned, Square::E2.bb());
        let hop = Move::new(Square::E2, Square::C3, MoveFlag::Quiet);
        assert!(pos.is_pseudo_legal(hop, &ci));
        assert!(!pos.is_legal(hop, &ci));

        // A pinned rook may move along the pin.
        let (pos, ci) = Position::from_fen("4r2k/8/8/8/8/8/4R3/4K3 w - - 0 1").unwrap();
        assert!(pos.is_legal(Move::new(Square::E2, Square::E5, MoveFlag::Quiet), &ci));
        assert!(pos.is_legal(Move::new(Square::E2, Square::E8, MoveFlag::Capture), &ci));
        assert!(!pos.is_legal(Move::new(Square::E2, Square::D2, MoveFlag::Quiet), &ci));

        // In check from the rook, the king may not retreat along the rank.
        let (pos, ci) = Position::from_fen("7k/8/8/8/8/8/8/r3K3 w - - 0 1").unwrap();
        assert_eq!(pos.checkers(), Square::A1.bb());
        assert!(!pos.is_legal(Move::new(Square::E1, Square::F1, MoveFlag::Quiet), &ci));
        assert!(pos.is_legal(Move::new(Square::E1, Square::E2, MoveFlag::Quiet), &ci));
    }

    #[test]
    fn en_passant_that_exposes_the_king_is_illegal() {
        // Both pawns leave the fifth rank, opening the rook's line to the king.
        let (pos, ci) = Position::from_fen("7k/8/8/K2pP2r/8/8/8/8 w - d6 0 1").unwrap();
        let capture = Move::new(Square::E5, Square::D6, MoveFlag::EnPassant);
        assert!(pos.is_pseudo_legal(capture, &ci));
        assert!(!pos.is_legal(capture, &ci));
    }

    #[test]
    fn make_into_matches_make() {
        let (start, ci) = Position::startpos();
        let mv = Move::new(Square::E2, Square::E4, MoveFlag::DoublePush);
        let mut out = start.clone();
        start.make_into(mv, &ci, &mut out);
        assert_eq!(out, start.make(mv, &ci));
        assert_eq!(out.state().fullmove, 1);
        let reply = Move::new(Square::E7, Square::E5, MoveFlag::DoublePush);
        assert_eq!(out.make(reply, &ci).state().fullmove, 2);
    }
}
