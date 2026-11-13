//! Pseudo-legal move generation.
//!
//! Generated moves obey how the pieces move but may leave the mover's king attacked;
//! [`Position::is_legal`] settles that when a move is about to be played. When the side
//! to move is in check, only moves that could answer the check are generated.

use core::mem::MaybeUninit;

use crate::attacks::{
    between, bishop_attacks, king_attacks, knight_attacks, pawn_attacks, queen_attacks,
    rook_attacks,
};
use crate::castling::{CastleSide, CastlingInfo};
use crate::{Bitboard, CastlingRights, Color, Move, MoveFlag, PieceType, Position, Square};

/// A move with the ordering score the search assigns to it.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// A fixed-capacity list of moves that is never zeroed.
pub struct MoveList {
    len: usize,
    moves: [MaybeUninit<ScoredMove>; MoveList::CAPACITY],
}

impl MoveList {
    /// More than any position accepted by [`Position::from_fen`] or reached from one
    /// can have; the known maximum of legal moves is 218.
    pub const CAPACITY: usize = 256;

    #[inline]
    pub fn new() -> MoveList {
        MoveList {
            len: 0,
            moves: [const { MaybeUninit::uninit() }; MoveList::CAPACITY],
        }
    }

    /// Appends a move with a score of zero. Panics if the list is full.
    #[inline]
    pub fn push(&mut self, mv: Move) {
        self.moves[self.len].write(ScoredMove { mv, score: 0 });
        self.len += 1;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    #[allow(unsafe_code)]
    pub fn as_slice(&self) -> &[ScoredMove] {
        // SAFETY: `push` is the only way `len` grows, and it initialises element
        // `len` through a bounds-checked index before incrementing, so the first `len`
        // elements are initialised and `len <= CAPACITY`. `MaybeUninit<T>` has the
        // layout of `T`.
        unsafe { core::slice::from_raw_parts(self.moves.as_ptr().cast::<ScoredMove>(), self.len) }
    }

    #[inline]
    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [ScoredMove] {
        // SAFETY: as in `as_slice`; the exclusive borrow of `self` covers the elements.
        unsafe {
            core::slice::from_raw_parts_mut(self.moves.as_mut_ptr().cast::<ScoredMove>(), self.len)
        }
    }

    /// The moves in the order they were added.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Move> + '_ {
        self.as_slice().iter().map(|scored| scored.mv)
    }

    pub fn contains(&self, mv: Move) -> bool {
        self.iter().any(|m| m == mv)
    }
}

impl Default for MoveList {
    fn default() -> MoveList {
        MoveList::new()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Stage {
    Noisy,
    Quiet,
    Evasion,
}

const PROMOTIONS: [MoveFlag; 4] = [
    MoveFlag::PromoQueen,
    MoveFlag::PromoKnight,
    MoveFlag::PromoRook,
    MoveFlag::PromoBishop,
];
const PROMOTION_CAPTURES: [MoveFlag; 4] = [
    MoveFlag::PromoCaptureQueen,
    MoveFlag::PromoCaptureKnight,
    MoveFlag::PromoCaptureRook,
    MoveFlag::PromoCaptureBishop,
];

/// The square `delta` index steps from `sq`. The caller guarantees it is on the board.
#[inline]
fn offset(sq: Square, delta: i8) -> Square {
    Square::from_index((sq as i8 + delta) as u8)
}

/// Every pseudo-legal move: evasions when in check, otherwise noisy then quiet moves.
pub fn gen_all(pos: &Position, ci: &CastlingInfo, list: &mut MoveList) {
    if pos.in_check() {
        gen_evasions(pos, list);
    } else {
        gen_noisy(pos, list);
        gen_quiet(pos, ci, list);
    }
}

/// Captures, en passant and queen promotions. The side to move must not be in check.
pub fn gen_noisy(pos: &Position, list: &mut MoveList) {
    debug_assert!(!pos.in_check());
    let theirs = pos.colors(!pos.stm());
    gen_pawns(pos, list, Bitboard::ALL, Stage::Noisy);
    gen_pieces(pos, list, theirs);
    gen_king(pos, list, theirs);
}

/// Moves that are not captures, including castling and promotions to less than a
/// queen without capture. The side to move must not be in check.
pub fn gen_quiet(pos: &Position, ci: &CastlingInfo, list: &mut MoveList) {
    debug_assert!(!pos.in_check());
    let empty = !pos.occupied();
    gen_pawns(pos, list, Bitboard::ALL, Stage::Quiet);
    gen_pieces(pos, list, empty);
    gen_king(pos, list, empty);
    gen_castling(pos, ci, list);
}

/// Moves that could answer a check: king moves, and against a single checker, captures
/// of it and interpositions. The side to move must be in check.
pub fn gen_evasions(pos: &Position, list: &mut MoveList) {
    let us = pos.stm();
    let checkers = pos.checkers();
    debug_assert!(checkers.any());

    gen_king(pos, list, !pos.colors(us));
    if checkers.more_than_one() {
        return;
    }
    let mask = between(pos.king_square(us), checkers.first()) | checkers;
    gen_pawns(pos, list, mask, Stage::Evasion);
    gen_pieces(pos, list, mask);
}

/// All legal moves, for callers that do not need staged generation.
pub fn gen_legal(pos: &Position, ci: &CastlingInfo, list: &mut MoveList) {
    let mut all = MoveList::new();
    gen_all(pos, ci, &mut all);
    for mv in all.iter() {
        if pos.is_legal(mv, ci) {
            list.push(mv);
        }
    }
}

#[inline]
fn push_moves(list: &mut MoveList, from: Square, targets: Bitboard, theirs: Bitboard) {
    for to in targets & theirs {
        list.push(Move::new(from, to, MoveFlag::Capture));
    }
    for to in targets & !theirs {
        list.push(Move::new(from, to, MoveFlag::Quiet));
    }
}

/// Knight, bishop, rook and queen moves onto `targets`.
fn gen_pieces(pos: &Position, list: &mut MoveList, targets: Bitboard) {
    let us = pos.stm();
    let occ = pos.occupied();
    let theirs = pos.colors(!us);

    for from in pos.piece_bb(us, PieceType::Knight) {
        push_moves(list, from, knight_attacks(from) & targets, theirs);
    }
    for from in pos.piece_bb(us, PieceType::Bishop) {
        push_moves(list, from, bishop_attacks(from, occ) & targets, theirs);
    }
    for from in pos.piece_bb(us, PieceType::Rook) {
        push_moves(list, from, rook_attacks(from, occ) & targets, theirs);
    }
    for from in pos.piece_bb(us, PieceType::Queen) {
        push_moves(list, from, queen_attacks(from, occ) & targets, theirs);
    }
}

fn gen_king(pos: &Position, list: &mut MoveList, targets: Bitboard) {
    let us = pos.stm();
    let from = pos.king_square(us);
    push_moves(list, from, king_attacks(from) & targets, pos.colors(!us));
}

fn gen_castling(pos: &Position, ci: &CastlingInfo, list: &mut MoveList) {
    let us = pos.stm();
    let rights = pos.state().castling;
    if !rights.intersects(CastlingRights::both(us)) {
        return;
    }
    let king = pos.king_square(us);
    let occ = pos.occupied();
    for side in CastleSide::ALL {
        if rights.contains(CastlingRights::of(us, side))
            && let Some(rook) = ci.rook_from(us, side)
            && (ci.empty_path(us, side) & occ).is_empty()
        {
            list.push(Move::new(king, rook, side.flag()));
        }
    }
}

/// Pawn moves. `mask` limits destinations (and, for en passant, the pawn captured);
/// it is everything except when evading a check.
fn gen_pawns(pos: &Position, list: &mut MoveList, mask: Bitboard, stage: Stage) {
    let us = pos.stm();
    let them = !us;
    let pawns = pos.piece_bb(us, PieceType::Pawn);
    let empty = !pos.occupied();
    let targets = pos.colors(them) & mask;

    // `up` is one rank forward; a capture towards the a-file is `up - 1`.
    let (up, seventh, third): (i8, Bitboard, Bitboard) = match us {
        Color::White => (8, Bitboard::rank(6), Bitboard::rank(2)),
        Color::Black => (-8, Bitboard::rank(1), Bitboard::rank(5)),
    };
    let forward = |bb: Bitboard| match us {
        Color::White => bb.north(),
        Color::Black => bb.south(),
    };
    let promoting = pawns & seventh;
    let others = pawns & !seventh;

    if stage != Stage::Noisy {
        let single = forward(others) & empty;
        let double = forward(single & third) & empty;
        for to in single & mask {
            list.push(Move::new(offset(to, -up), to, MoveFlag::Quiet));
        }
        for to in double & mask {
            list.push(Move::new(offset(to, -2 * up), to, MoveFlag::DoublePush));
        }
    }

    if stage != Stage::Quiet {
        for to in forward(others).west() & targets {
            list.push(Move::new(offset(to, -up + 1), to, MoveFlag::Capture));
        }
        for to in forward(others).east() & targets {
            list.push(Move::new(offset(to, -up - 1), to, MoveFlag::Capture));
        }
    }

    if promoting.any() {
        let flags: &[MoveFlag] = match stage {
            Stage::Noisy => &PROMOTIONS[..1],
            Stage::Quiet => &PROMOTIONS[1..],
            Stage::Evasion => &PROMOTIONS,
        };
        for to in forward(promoting) & empty & mask {
            for &flag in flags {
                list.push(Move::new(offset(to, -up), to, flag));
            }
        }
        if stage != Stage::Quiet {
            for to in forward(promoting).west() & targets {
                for flag in PROMOTION_CAPTURES {
                    list.push(Move::new(offset(to, -up + 1), to, flag));
                }
            }
            for to in forward(promoting).east() & targets {
                for flag in PROMOTION_CAPTURES {
                    list.push(Move::new(offset(to, -up - 1), to, flag));
                }
            }
        }
    }

    if stage != Stage::Quiet
        && let Some(ep) = pos.state().ep_square
    {
        // When evading, en passant helps only if it removes the checking pawn or lands
        // on the checking line.
        let captured_on = offset(ep, -up);
        if stage != Stage::Evasion || mask.contains(captured_on) || mask.contains(ep) {
            for from in pawn_attacks(them, ep) & pawns {
                list.push(Move::new(from, ep, MoveFlag::EnPassant));
            }
        }
    }
}

impl Position {
    /// Whether [`gen_all`] would produce `mv` here.
    ///
    /// Moves that come from outside the generator, such as one stored in the
    /// transposition table, must pass this before [`is_legal`](Position::is_legal) or
    /// [`make`](Position::make); any 16-bit pattern is safe to ask about.
    pub fn is_pseudo_legal(&self, mv: Move, ci: &CastlingInfo) -> bool {
        let us = self.stm();
        let them = !us;
        let from = mv.from();
        let to = mv.to();
        let Some(piece) = self.piece_on(from) else {
            return false;
        };
        if piece.color() != us {
            return false;
        }
        let piece_type = piece.piece_type();
        let occ = self.occupied();
        let theirs = self.colors(them);
        let checkers = self.checkers();

        if mv.is_castle() {
            let side = if mv.is_king_castle() {
                CastleSide::King
            } else {
                CastleSide::Queen
            };
            return piece_type == PieceType::King
                && checkers.is_empty()
                && self.state().castling.contains(CastlingRights::of(us, side))
                && ci.rook_from(us, side) == Some(to)
                && (ci.empty_path(us, side) & occ).is_empty();
        }
        if self.colors(us).contains(to) {
            return false;
        }

        const QUIET: u16 = MoveFlag::Quiet as u16;
        const DOUBLE_PUSH: u16 = MoveFlag::DoublePush as u16;
        const CAPTURE: u16 = MoveFlag::Capture as u16;
        const EN_PASSANT: u16 = MoveFlag::EnPassant as u16;
        let flag = mv.flag_bits();

        if piece_type == PieceType::Pawn {
            let (up, second, last): (i8, u8, u8) = match us {
                Color::White => (8, 1, 7),
                Color::Black => (-8, 6, 0),
            };
            let from_index = from as i8;
            let to_index = to as i8;
            let attacks = pawn_attacks(us, from);
            // Promotion flags reduce to the plain push or capture they are made with.
            let base = if mv.is_promotion() {
                (flag & 4) | (QUIET & 3)
            } else {
                flag
            };
            let reaches_last_rank = to.rank() == last;

            let shape_ok = match base {
                EN_PASSANT => self.state().ep_square == Some(to) && attacks.contains(to),
                _ if mv.is_promotion() != reaches_last_rank => false,
                QUIET => to_index == from_index + up && !occ.contains(to),
                DOUBLE_PUSH => {
                    from.rank() == second
                        && to_index == from_index + 2 * up
                        && !occ.contains(to)
                        && !occ.contains(offset(from, up))
                }
                CAPTURE => attacks.contains(to) && theirs.contains(to),
                _ => false,
            };
            if !shape_ok {
                return false;
            }
        } else {
            let flag_ok = match flag {
                QUIET => !theirs.contains(to),
                CAPTURE => theirs.contains(to),
                _ => false,
            };
            if !flag_ok {
                return false;
            }
            let attacks = match piece_type {
                PieceType::Knight => knight_attacks(from),
                PieceType::Bishop => bishop_attacks(from, occ),
                PieceType::Rook => rook_attacks(from, occ),
                PieceType::Queen => queen_attacks(from, occ),
                _ => king_attacks(from),
            };
            if !attacks.contains(to) {
                return false;
            }
        }

        // In check, the generator produces only evasions.
        if checkers.any() && piece_type != PieceType::King {
            if checkers.more_than_one() {
                return false;
            }
            let mask = between(self.king_square(us), checkers.first()) | checkers;
            if mv.is_en_passant() {
                let captured_on = Square::from_file_rank(to.file(), from.rank());
                return mask.contains(captured_on) || mask.contains(to);
            }
            return mask.contains(to);
        }
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::position::START_FEN;

    /// xorshift64, for reproducible random games in tests.
    pub(crate) struct Rng(pub u64);

    impl Rng {
        pub(crate) fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    pub(crate) const ROOTS: [&str; 8] = [
        START_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        // Chess960 starting positions with castling rights on unusual files.
        "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9",
        "rk2r3/pppppppp/8/8/8/8/PPPPPPPP/RK2R3 w EAea - 0 1",
    ];

    /// Calls `visit` on positions from random games started at each root.
    pub(crate) fn random_positions(
        games_per_root: usize,
        max_plies: usize,
        mut visit: impl FnMut(&Position, &CastlingInfo),
    ) {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for fen in ROOTS {
            for _ in 0..games_per_root {
                let (mut pos, ci) = Position::from_fen(fen).unwrap();
                for _ in 0..max_plies {
                    visit(&pos, &ci);
                    let mut legal = MoveList::new();
                    gen_legal(&pos, &ci, &mut legal);
                    if legal.is_empty() || pos.state().halfmove >= 100 {
                        break;
                    }
                    let pick = legal.as_slice()[(rng.next() % legal.len() as u64) as usize].mv;
                    pos = pos.make(pick, &ci);
                }
            }
        }
    }

    fn generated(pos: &Position, ci: &CastlingInfo) -> MoveList {
        let mut list = MoveList::new();
        gen_all(pos, ci, &mut list);
        list
    }

    #[test]
    fn start_position_moves() {
        let (pos, ci) = Position::startpos();
        let list = generated(&pos, &ci);
        assert_eq!(list.len(), 20);
        assert!(list.iter().all(|mv| pos.is_legal(mv, &ci)));
        assert!(list.contains(Move::new(Square::E2, Square::E4, MoveFlag::DoublePush)));
        assert!(list.contains(Move::new(Square::G1, Square::F3, MoveFlag::Quiet)));

        let mut noisy = MoveList::new();
        gen_noisy(&pos, &mut noisy);
        assert!(noisy.is_empty());
    }

    #[test]
    fn noisy_and_quiet_partition_the_moves() {
        let (pos, ci) = Position::from_fen(ROOTS[1]).unwrap();
        let mut noisy = MoveList::new();
        let mut quiet = MoveList::new();
        gen_noisy(&pos, &mut noisy);
        gen_quiet(&pos, &ci, &mut quiet);
        assert!(noisy.iter().all(|mv| mv.is_capture()));
        assert!(quiet.iter().all(|mv| !mv.is_capture()));
        assert_eq!(noisy.len(), 8);
        assert_eq!(noisy.len() + quiet.len(), 48);
        assert_eq!(quiet.iter().filter(|mv| mv.is_castle()).count(), 2);
    }

    #[test]
    fn promotions_split_between_stages() {
        // One pawn can push to a8, and nothing else on the board can capture.
        let (pos, ci) = Position::from_fen("7k/P7/8/8/8/8/8/K7 w - - 0 1").unwrap();
        let mut noisy = MoveList::new();
        let mut quiet = MoveList::new();
        gen_noisy(&pos, &mut noisy);
        gen_quiet(&pos, &ci, &mut quiet);
        assert_eq!(noisy.len(), 1);
        assert_eq!(noisy.as_slice()[0].mv.promotion(), Some(PieceType::Queen));
        assert_eq!(quiet.iter().filter(|mv| mv.is_promotion()).count(), 3);
    }

    #[test]
    fn double_check_allows_only_king_moves() {
        // Knight on f6 and rook on e1 both check the king on e8.
        let (pos, ci) = Position::from_fen("4k3/3p4/5N2/8/8/8/8/4R2K b - - 0 1").unwrap();
        assert!(pos.checkers().more_than_one());
        let list = generated(&pos, &ci);
        assert!(!list.is_empty());
        assert!(list.iter().all(|mv| mv.from() == Square::E8));
    }

    #[test]
    fn single_check_allows_blocks_and_captures() {
        // Rook on e1 checks the king on e8; the bishop can block on e7 or e6... only on
        // squares of the e-file, and the knight can capture nothing.
        let (pos, ci) = Position::from_fen("4k3/8/8/2b5/8/8/8/4R2K b - - 0 1").unwrap();
        let list = generated(&pos, &ci);
        let bishop: Vec<Move> = list.iter().filter(|mv| mv.from() == Square::C5).collect();
        assert_eq!(bishop.len(), 2);
        assert!(bishop.iter().all(|mv| mv.to().file() == 4));
    }

    #[test]
    fn en_passant_can_capture_a_checking_pawn() {
        // White has just played d2-d4, checking the king on e5.
        let (pos, ci) = Position::from_fen("8/8/8/4k3/3Pp3/8/8/4K3 b - d3 0 1").unwrap();
        assert_eq!(pos.checkers(), Square::D4.bb());
        let list = generated(&pos, &ci);
        let ep = Move::new(Square::E4, Square::D3, MoveFlag::EnPassant);
        assert!(list.contains(ep));
        assert!(pos.is_legal(ep, &ci));
    }

    #[test]
    fn no_duplicates_and_generated_moves_keep_positions_valid() {
        let games = if cfg!(miri) { 0 } else { 6 };
        random_positions(games, 120, |pos, ci| {
            let list = generated(pos, ci);
            let mut raws: Vec<u16> = list.iter().map(|mv| mv.raw()).collect();
            raws.sort_unstable();
            raws.dedup();
            assert_eq!(raws.len(), list.len(), "duplicate move in {}", pos.fen(ci));
            for mv in list.iter() {
                if pos.is_legal(mv, ci) {
                    // Debug builds validate the child inside `make`.
                    let child = pos.make(mv, ci);
                    assert_eq!(child.validate(), Ok(()), "{mv:?} in {}", pos.fen(ci));
                }
            }
        });
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn is_pseudo_legal_agrees_with_the_generator_for_every_bit_pattern() {
        let mut checked = 0u32;
        random_positions(4, 60, |pos, ci| {
            let list = generated(pos, ci);
            let mut expected = vec![false; 1 << 16];
            for mv in list.iter() {
                expected[mv.raw() as usize] = true;
            }
            for raw in 1..=u16::MAX {
                let mv = Move::from_raw(raw).unwrap();
                assert_eq!(
                    pos.is_pseudo_legal(mv, ci),
                    expected[raw as usize],
                    "{mv:?} (raw {raw:#06x}) in {}",
                    pos.fen(ci)
                );
            }
            checked += 1;
        });
        assert!(checked > 1000, "only {checked} positions were checked");
    }
}
