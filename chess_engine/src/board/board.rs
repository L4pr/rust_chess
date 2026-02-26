use crate::{Move, Piece, CastlingRights, generate_all_moves, is_square_attacked, evaluate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board {
    pub pieces: [u64; 16],
    pub white_to_move: bool,
    pub en_passant_square: Option<u8>,
    pub castling_rights: CastlingRights,
    pub halfmove_clock: u32,
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
            halfmove_clock: 0,
        };

        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
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

        board.pieces[0] = board.pieces[7] | board.pieces[15];
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

        if parts.len() > 4 {
            board.halfmove_clock = parts[4].parse().unwrap_or(0);
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

        fen.push_str(&format!(" {} 1", self.halfmove_clock));

        fen
    }

    pub fn to_book_fen(&self) -> String {
        let full_fen = self.get_fen();
        let parts: Vec<&str> = full_fen.split_whitespace().collect();

        if parts.len() >= 3 {
            format!("{} {} {} -", parts[0], parts[1], parts[2])
        } else {
            full_fen
        }
    }

    pub fn parse_uci_to_move(&self, uci: &str) -> Option<Move> {
        let mut move_storage = [Move(0); 218];
        let count = generate_all_moves(self, &mut move_storage);
        let legal_moves = &move_storage[..count];

        for &m in legal_moves {
            if m.to_uci() == uci {
                return Some(m);
            }
        }

        None
    }

    pub fn make_move(&mut self, m: Move) {
        let from = m.from_sq();
        let to = m.to_sq();
        let flags = m.flags();
        let from_bit = 1u64 << from;
        let to_bit = 1u64 << to;
        let move_mask = from_bit | to_bit;

        let us = if self.white_to_move { Piece::WHITE } else { Piece::BLACK };
        let them = us ^ 8;

        let mut moved_piece_type = Piece::PAWN;
        for pt in [Piece::PAWN, Piece::KNIGHT, Piece::BISHOP, Piece::ROOK, Piece::QUEEN, Piece::KING] {
            if (self.pieces[(us | pt) as usize] & from_bit) != 0 {
                moved_piece_type = pt;
                break;
            }
        }

        if m.is_capture() || moved_piece_type == Piece::PAWN {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        if m.is_capture() && flags != Move::EN_PASSANT {
            for pt in [Piece::PAWN, Piece::KNIGHT, Piece::BISHOP, Piece::ROOK, Piece::QUEEN, Piece::KING] {
                if (self.pieces[(them | pt) as usize] & to_bit) != 0 {
                    self.pieces[(them | pt) as usize] ^= to_bit;
                    self.pieces[(them | Piece::ALL) as usize] ^= to_bit;
                    break;
                }
            }
        }

        self.pieces[(us | moved_piece_type) as usize] ^= move_mask;
        self.pieces[(us | Piece::ALL) as usize] ^= move_mask;

        if flags == Move::EN_PASSANT {
            let capture_sq = if self.white_to_move { to - 8 } else { to + 8 };
            let cap_bit = 1u64 << capture_sq;
            self.pieces[(them | Piece::PAWN) as usize] ^= cap_bit;
            self.pieces[(them | Piece::ALL) as usize] ^= cap_bit;
        }

        if m.is_promotion() {
            self.pieces[(us | Piece::PAWN) as usize] ^= to_bit;
            let promo_type = match flags {
                Move::PR_KNIGHT | Move::PC_KNIGHT => Piece::KNIGHT,
                Move::PR_BISHOP | Move::PC_BISHOP => Piece::BISHOP,
                Move::PR_ROOK | Move::PC_ROOK => Piece::ROOK,
                _ => Piece::QUEEN,
            };
            self.pieces[(us | promo_type) as usize] ^= to_bit;
        }

        if flags == Move::KING_CASTLE {
            let (r_from, r_to) = if self.white_to_move { (7, 5) } else { (63, 61) };
            let r_mask = (1u64 << r_from) | (1u64 << r_to);
            self.pieces[(us | Piece::ROOK) as usize] ^= r_mask;
            self.pieces[(us | Piece::ALL) as usize] ^= r_mask;
        } else if flags == Move::QUEEN_CASTLE {
            let (r_from, r_to) = if self.white_to_move { (0, 3) } else { (56, 59) };
            let r_mask = (1u64 << r_from) | (1u64 << r_to);
            self.pieces[(us | Piece::ROOK) as usize] ^= r_mask;
            self.pieces[(us | Piece::ALL) as usize] ^= r_mask;
        }

        self.en_passant_square = if flags == Move::DOUBLE_PAWN_PUSH {
            Some(if self.white_to_move { from + 8 } else { from - 8 })
        } else {
            None
        };

        self.update_castling_rights(from, to);
        self.white_to_move = !self.white_to_move;
        self.pieces[0] = self.pieces[7] | self.pieces[15];
    }

    pub fn update_castling_rights(&mut self, from: u8, to: u8) {
        self.castling_rights.0 &= CASTLING_MASKS[from as usize];
        self.castling_rights.0 &= CASTLING_MASKS[to as usize];
    }

    pub fn is_in_check(&self) -> bool {
        let us = if self.white_to_move { Piece::WHITE } else { Piece::BLACK };
        let king_bitboard = self.pieces[(us | Piece::KING) as usize];

        let king_sq = king_bitboard.trailing_zeros() as u8;
        is_square_attacked(self, king_sq, us ^ 8)
    }

    pub fn evaluate_board(&self) -> f64 {
        evaluate(self)
    }
}

const CASTLING_MASKS: [u8; 64] = [
    0b1101, 0b1111, 0b1111, 0b1111, 0b1100, 0b1111, 0b1111, 0b1110,
    0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111,
    0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111,
    0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111,
    0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111,
    0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111,
    0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111, 0b1111,
    0b0111, 0b1111, 0b1111, 0b1111, 0b0011, 0b1111, 0b1111, 0b1011,
];

pub fn is_draw_by_repetition(halfmove_clock: u32, history: &[u64], current_hash: u64) -> bool {
    // Only look back as far as the halfmove clock
    let start = history.len().saturating_sub(halfmove_clock as usize);

    for &h in history[start..].iter().rev() {
        if h == current_hash {
            return true;
        }
    }
    false
}