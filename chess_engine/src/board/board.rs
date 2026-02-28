use crate::{Move, Piece, CastlingRights, generate_all_moves, is_square_attacked, evaluate};
use crate::board::zobrist::{zobrist, ZobristKeys};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Board {
    pub pieces: [u64; 16],
    /// Mailbox: for each square, stores the piece index (color|type), or EMPTY (0xFF)
    pub mailbox: [u8; 64],
    pub white_to_move: bool,
    pub en_passant_square: Option<u8>,
    pub castling_rights: CastlingRights,
    pub halfmove_clock: u32,
    pub zobrist_hash: u64,
}

const EMPTY: u8 = 0xFF;

impl Board {
    pub fn starting_position() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    }

    pub fn from_fen(fen: &str) -> Self {
        let mut board = Board {
            pieces: [0; 16],
            mailbox: [EMPTY; 64],
            white_to_move: true,
            en_passant_square: None,
            castling_rights: CastlingRights::new(0),
            halfmove_clock: 0,
            zobrist_hash: 0,
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
                board.mailbox[square] = color | piece_type;

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

        // Compute full zobrist hash from scratch
        board.zobrist_hash = zobrist().hash(&board);

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
        let bytes = uci.as_bytes();
        if bytes.len() < 4 { return None; }

        let from_file = bytes[0] - b'a';
        let from_rank = bytes[1] - b'1';
        let to_file = bytes[2] - b'a';
        let to_rank = bytes[3] - b'1';
        let from_sq = from_rank * 8 + from_file;
        let to_sq = to_rank * 8 + to_file;

        let promo_char = if bytes.len() > 4 { Some(bytes[4]) } else { None };

        let mut move_storage = [Move(0); 218];
        let count = generate_all_moves(self, &mut move_storage);

        for i in 0..count {
            let m = move_storage[i];
            if m.from_sq() == from_sq && m.to_sq() == to_sq {
                // Check promotion match
                if m.is_promotion() {
                    let m_promo = match m.flags() {
                        Move::PR_KNIGHT | Move::PC_KNIGHT => b'n',
                        Move::PR_BISHOP | Move::PC_BISHOP => b'b',
                        Move::PR_ROOK | Move::PC_ROOK => b'r',
                        _ => b'q',
                    };
                    if promo_char == Some(m_promo) {
                        return Some(m);
                    }
                } else if promo_char.is_none() {
                    return Some(m);
                }
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

        let z = zobrist();

        // XOR out old castling rights and en passant
        self.zobrist_hash ^= z.castling[(self.castling_rights.0 & 0b1111) as usize];
        if let Some(ep_sq) = self.en_passant_square {
            self.zobrist_hash ^= z.en_passant[(ep_sq % 8) as usize];
        }

        // O(1) piece lookup via mailbox
        let moved_piece = self.mailbox[from as usize];
        let moved_piece_type = moved_piece & 0x07; // mask out color bits

        if m.is_capture() || moved_piece_type == Piece::PAWN {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }

        // Remove captured piece (O(1) via mailbox)
        if m.is_capture() && flags != Move::EN_PASSANT {
            let captured = self.mailbox[to as usize];
            debug_assert_ne!(captured, EMPTY);
            let cap_type = captured & 0x07;
            self.pieces[(them | cap_type) as usize] ^= to_bit;
            self.pieces[(them | Piece::ALL) as usize] ^= to_bit;
            self.zobrist_hash ^= z.pieces[ZobristKeys::piece_index(them, cap_type)][to as usize];
            // mailbox[to] will be overwritten below
        }

        // Move the piece
        let zi = ZobristKeys::piece_index(us, moved_piece_type);
        self.zobrist_hash ^= z.pieces[zi][from as usize];
        self.zobrist_hash ^= z.pieces[zi][to as usize];
        self.pieces[(us | moved_piece_type) as usize] ^= move_mask;
        self.pieces[(us | Piece::ALL) as usize] ^= move_mask;
        self.mailbox[to as usize] = moved_piece;
        self.mailbox[from as usize] = EMPTY;

        // En passant capture
        if flags == Move::EN_PASSANT {
            let capture_sq = if self.white_to_move { to - 8 } else { to + 8 };
            let cap_bit = 1u64 << capture_sq;
            self.pieces[(them | Piece::PAWN) as usize] ^= cap_bit;
            self.pieces[(them | Piece::ALL) as usize] ^= cap_bit;
            self.zobrist_hash ^= z.pieces[ZobristKeys::piece_index(them, Piece::PAWN)][capture_sq as usize];
            self.mailbox[capture_sq as usize] = EMPTY;
        }

        // Promotion: remove pawn, add promoted piece
        if m.is_promotion() {
            self.pieces[(us | Piece::PAWN) as usize] ^= to_bit;
            self.zobrist_hash ^= z.pieces[ZobristKeys::piece_index(us, Piece::PAWN)][to as usize];

            let promo_type = match flags {
                Move::PR_KNIGHT | Move::PC_KNIGHT => Piece::KNIGHT,
                Move::PR_BISHOP | Move::PC_BISHOP => Piece::BISHOP,
                Move::PR_ROOK | Move::PC_ROOK => Piece::ROOK,
                _ => Piece::QUEEN,
            };
            self.pieces[(us | promo_type) as usize] ^= to_bit;
            self.zobrist_hash ^= z.pieces[ZobristKeys::piece_index(us, promo_type)][to as usize];
            self.mailbox[to as usize] = us | promo_type;
        }

        // Castling rook movement
        if flags == Move::KING_CASTLE {
            let (r_from, r_to) = if self.white_to_move { (7u8, 5u8) } else { (63u8, 61u8) };
            let r_mask = (1u64 << r_from) | (1u64 << r_to);
            self.pieces[(us | Piece::ROOK) as usize] ^= r_mask;
            self.pieces[(us | Piece::ALL) as usize] ^= r_mask;
            let ri = ZobristKeys::piece_index(us, Piece::ROOK);
            self.zobrist_hash ^= z.pieces[ri][r_from as usize];
            self.zobrist_hash ^= z.pieces[ri][r_to as usize];
            self.mailbox[r_to as usize] = us | Piece::ROOK;
            self.mailbox[r_from as usize] = EMPTY;
        } else if flags == Move::QUEEN_CASTLE {
            let (r_from, r_to) = if self.white_to_move { (0u8, 3u8) } else { (56u8, 59u8) };
            let r_mask = (1u64 << r_from) | (1u64 << r_to);
            self.pieces[(us | Piece::ROOK) as usize] ^= r_mask;
            self.pieces[(us | Piece::ALL) as usize] ^= r_mask;
            let ri = ZobristKeys::piece_index(us, Piece::ROOK);
            self.zobrist_hash ^= z.pieces[ri][r_from as usize];
            self.zobrist_hash ^= z.pieces[ri][r_to as usize];
            self.mailbox[r_to as usize] = us | Piece::ROOK;
            self.mailbox[r_from as usize] = EMPTY;
        }

        // En passant square
        self.en_passant_square = if flags == Move::DOUBLE_PAWN_PUSH {
            let ep = if self.white_to_move { from + 8 } else { from - 8 };
            self.zobrist_hash ^= z.en_passant[(ep % 8) as usize];
            Some(ep)
        } else {
            None
        };

        self.update_castling_rights(from, to);

        // XOR in new castling rights
        self.zobrist_hash ^= z.castling[(self.castling_rights.0 & 0b1111) as usize];

        // Flip side to move
        self.white_to_move = !self.white_to_move;
        self.zobrist_hash ^= z.black_to_move;

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

    pub fn evaluate_board(&self) -> i32 {
        evaluate(self)
    }

    /// Returns true if the side to move has at least one non-pawn, non-king piece.
    /// Used to avoid null-move pruning in endgames with only pawns.
    pub fn has_non_pawn_material(&self) -> bool {
        let us = if self.white_to_move { Piece::WHITE } else { Piece::BLACK };
        (self.pieces[(us | Piece::KNIGHT) as usize]
            | self.pieces[(us | Piece::BISHOP) as usize]
            | self.pieces[(us | Piece::ROOK) as usize]
            | self.pieces[(us | Piece::QUEEN) as usize]) != 0
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
    // Positions can only repeat with the same side to move, so step by 2
    let len = history.len();
    let lookback = (halfmove_clock as usize).min(len);
    if lookback < 2 { return false; }

    let mut i = len - 2;
    let stop = len - lookback;
    loop {
        if history[i] == current_hash {
            return true;
        }
        if i < stop + 2 { break; }
        i -= 2;
    }
    false
}