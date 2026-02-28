use crate::{Board, Piece};
use std::sync::OnceLock;

pub struct ZobristKeys {
    pub pieces: [[u64; 64]; 12], // 12 piece types (6 white, 6 black) across 64 squares
    pub black_to_move: u64,
    pub castling: [u64; 16],     // 16 possible castling right combinations
    pub en_passant: [u64; 8],    // 8 possible files for en passant
}

/// Global Zobrist keys, initialized once and accessible from anywhere.
static ZOBRIST: OnceLock<ZobristKeys> = OnceLock::new();

/// Initialize the global Zobrist keys. Call once at startup.
pub fn init_zobrist() {
    ZOBRIST.get_or_init(ZobristKeys::new);
}

/// Get a reference to the global Zobrist keys.
#[inline]
pub fn zobrist() -> &'static ZobristKeys {
    ZOBRIST.get().expect("Zobrist keys not initialized")
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

    /// Map a piece bitboard index (like WHITE|PAWN = 1, BLACK|KNIGHT = 10)
    /// to the Zobrist piece index (0..11).
    #[inline]
    pub fn piece_index(color: u8, piece_type: u8) -> usize {
        // WHITE pieces: color=0, types 1-6 → indices 0-5
        // BLACK pieces: color=8, types 1-6 → indices 6-11
        let color_offset = if color == Piece::BLACK { 6 } else { 0 };
        (piece_type as usize - 1) + color_offset
    }

    /// Compute a full hash from scratch (used for initialization / verification).
    pub fn hash(&self, board: &Board) -> u64 {
        let mut hash = 0u64;

        let piece_types = [
            Piece::WHITE | Piece::PAWN, Piece::WHITE | Piece::KNIGHT, Piece::WHITE | Piece::BISHOP,
            Piece::WHITE | Piece::ROOK, Piece::WHITE | Piece::QUEEN, Piece::WHITE | Piece::KING,
            Piece::BLACK | Piece::PAWN, Piece::BLACK | Piece::KNIGHT, Piece::BLACK | Piece::BISHOP,
            Piece::BLACK | Piece::ROOK, Piece::BLACK | Piece::QUEEN, Piece::BLACK | Piece::KING,
        ];

        for (i, &pt) in piece_types.iter().enumerate() {
            let mut bb = board.pieces[pt as usize];
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                hash ^= self.pieces[i][sq];
                bb &= bb - 1;
            }
        }

        if !board.white_to_move { hash ^= self.black_to_move; }
        hash ^= self.castling[(board.castling_rights.0 & 0b1111) as usize];

        if let Some(ep_sq) = board.en_passant_square {
            hash ^= self.en_passant[(ep_sq % 8) as usize];
        }

        hash
    }
}