//! Colours, piece types and coloured pieces.

use core::ops::Not;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub const COUNT: usize = 2;
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl Not for Color {
    type Output = Color;

    #[inline]
    fn not(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    pub const COUNT: usize = 6;
    pub const ALL: [PieceType; 6] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Lower-case letter used in FEN and UCI promotion suffixes.
    #[inline]
    pub const fn to_char(self) -> char {
        match self {
            PieceType::Pawn => 'p',
            PieceType::Knight => 'n',
            PieceType::Bishop => 'b',
            PieceType::Rook => 'r',
            PieceType::Queen => 'q',
            PieceType::King => 'k',
        }
    }
}

/// A coloured piece. The discriminant is `(piece_type << 1) | color`.
///
/// "No piece" is `Option<Piece>`, which is one byte because of the enum niche.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Piece {
    WhitePawn = 0,
    BlackPawn = 1,
    WhiteKnight = 2,
    BlackKnight = 3,
    WhiteBishop = 4,
    BlackBishop = 5,
    WhiteRook = 6,
    BlackRook = 7,
    WhiteQueen = 8,
    BlackQueen = 9,
    WhiteKing = 10,
    BlackKing = 11,
}

const _: () = assert!(size_of::<Option<Piece>>() == 1);

impl Piece {
    pub const COUNT: usize = 12;
    pub const ALL: [Piece; 12] = [
        Piece::WhitePawn,
        Piece::BlackPawn,
        Piece::WhiteKnight,
        Piece::BlackKnight,
        Piece::WhiteBishop,
        Piece::BlackBishop,
        Piece::WhiteRook,
        Piece::BlackRook,
        Piece::WhiteQueen,
        Piece::BlackQueen,
        Piece::WhiteKing,
        Piece::BlackKing,
    ];

    #[inline]
    pub const fn new(color: Color, piece_type: PieceType) -> Piece {
        Piece::ALL[((piece_type as usize) << 1) | color as usize]
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn color(self) -> Color {
        if self as u8 & 1 == 0 {
            Color::White
        } else {
            Color::Black
        }
    }

    #[inline]
    pub const fn piece_type(self) -> PieceType {
        PieceType::ALL[self as usize >> 1]
    }

    /// FEN letter: upper case for White, lower case for Black.
    #[inline]
    pub const fn to_char(self) -> char {
        let c = self.piece_type().to_char();
        match self.color() {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }

    /// The piece for a FEN letter, or `None` if the letter is not one.
    pub const fn from_char(c: char) -> Option<Piece> {
        let piece_type = match c.to_ascii_lowercase() {
            'p' => PieceType::Pawn,
            'n' => PieceType::Knight,
            'b' => PieceType::Bishop,
            'r' => PieceType::Rook,
            'q' => PieceType::Queen,
            'k' => PieceType::King,
            _ => return None,
        };
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        Some(Piece::new(color, piece_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_flip() {
        assert_eq!(!Color::White, Color::Black);
        assert_eq!(!Color::Black, Color::White);
    }

    #[test]
    fn tables_match_discriminants() {
        for (i, c) in Color::ALL.into_iter().enumerate() {
            assert_eq!(c.index(), i);
        }
        for (i, pt) in PieceType::ALL.into_iter().enumerate() {
            assert_eq!(pt.index(), i);
        }
        for (i, p) in Piece::ALL.into_iter().enumerate() {
            assert_eq!(p.index(), i);
        }
    }

    #[test]
    fn piece_composition() {
        for color in Color::ALL {
            for piece_type in PieceType::ALL {
                let piece = Piece::new(color, piece_type);
                assert_eq!(piece.color(), color);
                assert_eq!(piece.piece_type(), piece_type);
                assert_eq!(piece.index(), (piece_type.index() << 1) | color.index());
            }
        }
        assert_eq!(Piece::new(Color::White, PieceType::King), Piece::WhiteKing);
        assert_eq!(Piece::new(Color::Black, PieceType::Pawn), Piece::BlackPawn);
    }

    #[test]
    fn fen_letters() {
        assert_eq!(Piece::WhiteKnight.to_char(), 'N');
        assert_eq!(Piece::BlackQueen.to_char(), 'q');
        for piece in Piece::ALL {
            assert_eq!(Piece::from_char(piece.to_char()), Some(piece));
        }
        for bad in ['x', '1', ' ', '/', 'é'] {
            assert_eq!(Piece::from_char(bad), None);
        }
    }
}
