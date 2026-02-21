use super::pieces::Piece;
use super::castling_rights::CastlingRights;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board {
    // 16 bitboards: 6 for each piece type (white and black) + 2 for all pieces of each color
    // we have extra bitboards for all pieces of each color to speed up move generation and checks, those are saved right after the piece type bitboards (WHITE_ALL = 7, BLACK_ALL = 15)
    // we also added an all pieces board, this is saved at index 0 and is used for move generation and checks, it is the bitwise OR of all piece type bitboards (WHITE_ALL | BLACK_ALL)
    pieces: [u64; 16],
    white_to_move: bool,
    en_passant_square: Option<u8>,
    castling_rights: CastlingRights,
}


impl Board {
    pub fn starting_position() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    }

    pub fn from_fen(fen: &str) -> Self {
        let mut board = Board {
            pieces: [0; 16],
            white_to_move: true,
            en_passant_square: None,
            castling_rights: CastlingRights::new(0),
        };

        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            panic!("Invalid FEN string!");
        }

        let mut row = 7;
        let mut col = 0;

        for ch in parts[0].chars() {
            if ch == '/' {
                row -= 1;
                col = 0;
            } else if ch.is_ascii_digit() {
                col += ch.to_digit(10).unwrap() as usize;
            } else {
                let color = if ch.is_uppercase() { Piece::WHITE } else { Piece::BLACK };
                let piece_type = match ch.to_ascii_lowercase() {
                    'p' => Piece::PAWN,
                    'n' => Piece::KNIGHT,
                    'b' => Piece::BISHOP,
                    'r' => Piece::ROOK,
                    'q' => Piece::QUEEN,
                    'k' => Piece::KING,
                    _ => panic!("Invalid piece character in FEN"),
                };

                let square = row * 8 + col;
                board.pieces[Piece::new(color, piece_type).0 as usize] |= 1u64 << square;
                board.pieces[(color | Piece::ALL) as usize] |= 1u64 << square;

                col += 1;
            }
        }

        board.pieces[0] = board.pieces[7] | board.pieces[15]; // Update the all pieces bitboard

        board.white_to_move = parts[1] == "w";

        let mut cr_val = 0;
        for char in parts[2].chars() {
            match char {
                'K' => cr_val |= CastlingRights::WHITE_KINGSIDE,
                'Q' => cr_val |= CastlingRights::WHITE_QUEENSIDE,
                'k' => cr_val |= CastlingRights::BLACK_KINGSIDE,
                'q' => cr_val |= CastlingRights::BLACK_QUEENSIDE,
                '-' => break,
                _ => (),
            }
        }
        board.castling_rights = CastlingRights::new(cr_val);

        if parts[3] != "-" {
            let mut chars = parts[3].chars();
            let file = chars.next().unwrap() as u8 - b'a';
            let rank = chars.next().unwrap() as u8 - b'1';
            board.en_passant_square = Some(rank * 8 + file);
        }

        board
    }

    pub fn get_fen(&self) -> String {
        let mut fen = String::new();

        for row in (0..8).rev() {
            let mut empty_count = 0;
            for col in 0..8 {
                let square = row * 8 + col;
                let bit = 1u64 << square;

                let mut piece_char = None;

                let piece_types = [
                    (Piece::PAWN, 'P'),
                    (Piece::KNIGHT, 'N'),
                    (Piece::BISHOP, 'B'),
                    (Piece::ROOK, 'R'),
                    (Piece::QUEEN, 'Q'),
                    (Piece::KING, 'K'),
                ];

                for &(pt, c) in &piece_types {
                    if (self.pieces[(Piece::WHITE | pt) as usize] & bit) != 0 {
                        piece_char = Some(c);
                        break;
                    }
                    if (self.pieces[(Piece::BLACK | pt) as usize] & bit) != 0 {
                        piece_char = Some(c.to_ascii_lowercase());
                        break;
                    }
                }

                if let Some(c) = piece_char {
                    if empty_count > 0 {
                        fen.push_str(&empty_count.to_string());
                        empty_count = 0;
                    }
                    fen.push(c);
                } else {
                    empty_count += 1;
                }
            }

            if empty_count > 0 {
                fen.push_str(&empty_count.to_string());
            }

            if row > 0 {
                fen.push('/');
            }
        }

        fen.push(' ');
        fen.push(if self.white_to_move { 'w' } else { 'b' });

        fen.push(' ');
        let mut castling = String::new();
        if (self.castling_rights.0 & CastlingRights::WHITE_KINGSIDE) != 0 { castling.push('K'); }
        if (self.castling_rights.0 & CastlingRights::WHITE_QUEENSIDE) != 0 { castling.push('Q'); }
        if (self.castling_rights.0 & CastlingRights::BLACK_KINGSIDE) != 0 { castling.push('k'); }
        if (self.castling_rights.0 & CastlingRights::BLACK_QUEENSIDE) != 0 { castling.push('q'); }

        if castling.is_empty() {
            fen.push('-');
        } else {
            fen.push_str(&castling);
        }

        fen.push(' ');
        if let Some(sq) = self.en_passant_square {
            let file = (sq % 8) as u8 + b'a';
            let rank = (sq / 8) as u8 + b'1';
            fen.push(file as char);
            fen.push(rank as char);
        } else {
            fen.push('-');
        }

        fen.push_str(" 0 1");

        fen
    }
}
