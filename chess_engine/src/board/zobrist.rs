use crate::{Board, Piece};

pub struct ZobristKeys {
    pub pieces: [[u64; 64]; 12], // 12 piece types (6 white, 6 black) across 64 squares
    pub black_to_move: u64,
    pub castling: [u64; 16],     // 16 possible castling right combinations
    pub en_passant: [u64; 8],    // 8 possible files for en passant
}

impl ZobristKeys {
    pub fn new() -> Self {
        let mut seed = 1070372_u64; // Arbitrary starting seed

        // Simple fast PRNG (Xorshift)
        let mut rand = || -> u64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        let mut keys = ZobristKeys {
            pieces: [[0; 64]; 12],
            black_to_move: rand(),
            castling: [0; 16],
            en_passant: [0; 8],
        };

        for piece in 0..12 {
            for sq in 0..64 {
                keys.pieces[piece][sq] = rand();
            }
        }
        for i in 0..16 { keys.castling[i] = rand(); }
        for i in 0..8 { keys.en_passant[i] = rand(); }

        keys
    }

    pub fn hash(&self, board: &Board) -> u64 {
        let mut hash = 0;

        // 1. Hash the pieces
        for sq in 0..64 {
            let bit = 1u64 << sq;
            // Iterate through White (0-5) and Black (6-11) pieces
            // Assuming your Piece types are indexed easily. For your bitboards:
            let piece_types = [
                Piece::WHITE | Piece::PAWN, Piece::WHITE | Piece::KNIGHT, Piece::WHITE | Piece::BISHOP,
                Piece::WHITE | Piece::ROOK, Piece::WHITE | Piece::QUEEN, Piece::WHITE | Piece::KING,
                Piece::BLACK | Piece::PAWN, Piece::BLACK | Piece::KNIGHT, Piece::BLACK | Piece::BISHOP,
                Piece::BLACK | Piece::ROOK, Piece::BLACK | Piece::QUEEN, Piece::BLACK | Piece::KING,
            ];

            for (i, &pt) in piece_types.iter().enumerate() {
                if (board.pieces[pt as usize] & bit) != 0 {
                    hash ^= self.pieces[i][sq];
                    break;
                }
            }
        }

        // 2. Hash side to move
        if !board.white_to_move { hash ^= self.black_to_move; }

        // 3. Hash castling rights
        hash ^= self.castling[(board.castling_rights.0 & 0b1111) as usize];

        // 4. Hash en passant
        if let Some(ep_sq) = board.en_passant_square {
            let file = (ep_sq % 8) as usize;
            hash ^= self.en_passant[file];
        }

        hash
    }
}