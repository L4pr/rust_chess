use crate::{Board, Piece};

// --- Phase Weights ---
const KNIGHT_PHASE: i32 = 1;
const BISHOP_PHASE: i32 = 1;
const ROOK_PHASE: i32 = 2;
const QUEEN_PHASE: i32 = 4;
const TOTAL_PHASE: i32 = 24;

// --- Material Values ---
const PAWN_VAL: i32 = 100;
const KNIGHT_VAL: i32 = 320;
const BISHOP_VAL: i32 = 330;
const ROOK_VAL: i32 = 500;
const QUEEN_VAL: i32 = 900;
const KING_VAL: i32 = 20000;

// --- Specialized Bonuses & Penalties ---
const BISHOP_PAIR_BONUS: i32 = 30;
const PASSED_PAWN_BONUS: [i32; 8] = [0, 10, 20, 40, 70, 120, 200, 0];
const ISOLATED_PAWN_PENALTY: i32 = -15;
const DOUBLED_PAWN_PENALTY: i32 = -15;

// --- File Masks ---
const FILE_A: u64 = 0x0101010101010101;
const FILE_MASKS: [u64; 8] = [
    FILE_A, FILE_A << 1, FILE_A << 2, FILE_A << 3,
    FILE_A << 4, FILE_A << 5, FILE_A << 6, FILE_A << 7,
];

const ADJACENT_FILES: [u64; 8] = [
    FILE_MASKS[1],
    FILE_MASKS[0] | FILE_MASKS[2],
    FILE_MASKS[1] | FILE_MASKS[3],
    FILE_MASKS[2] | FILE_MASKS[4],
    FILE_MASKS[3] | FILE_MASKS[5],
    FILE_MASKS[4] | FILE_MASKS[6],
    FILE_MASKS[5] | FILE_MASKS[7],
    FILE_MASKS[6],
];

// --- Compile-Time Passed Pawn Masks ---
const PASSED_PAWN_MASKS_WHITE: [u64; 64] = calculate_passed_masks(true);
const PASSED_PAWN_MASKS_BLACK: [u64; 64] = calculate_passed_masks(false);

const fn calculate_passed_masks(is_white: bool) -> [u64; 64] {
    let mut masks = [0u64; 64];
    let mut i = 0;
    while i < 64 {
        let file = i % 8;
        let rank = i / 8;
        let mut mask = 0u64;

        let mut r = 0;
        while r < 8 {
            let mut f = 0;
            while f < 8 {
                // Check if file f is same or adjacent to pawn file
                let dist = if f > file { f - file } else { file - f };
                if dist <= 1 {
                    let square_rank = r;
                    if is_white && square_rank > rank {
                        mask |= 1 << (r * 8 + f);
                    } else if !is_white && square_rank < rank {
                        mask |= 1 << (r * 8 + f);
                    }
                }
                f += 1;
            }
            r += 1;
        }
        masks[i] = mask;
        i += 1;
    }
    masks
}

// --- Piece Square Tables (Middlegame: MG, Endgame: EG) ---

#[rustfmt::skip]
const PAWN_PST_MG: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0
];

#[rustfmt::skip]
const KNIGHT_PST: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50
];

#[rustfmt::skip]
const BISHOP_PST: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20
];

#[rustfmt::skip]
const ROOK_PST_MG: [i32; 64] = [
      0,  0,  0,  0,  0,  0,  0,  0,
      5, 10, 10, 10, 10, 10, 10,  5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
      0,  0,  0,  5,  5,  0,  0,  0
];

#[rustfmt::skip]
const ROOK_PST_EG: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    10, 10, 10, 10, 10, 10, 10, 10,
     0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0
];

#[rustfmt::skip]
const QUEEN_PST_MG: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

#[rustfmt::skip]
const QUEEN_PST_EG: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  5,  5,  5,  5,  0,-10,
    -10,  5, 10, 10, 10, 10,  5,-10,
     -5,  5, 10, 20, 20, 10,  5, -5,
     -5,  5, 10, 20, 20, 10,  5, -5,
    -10,  5, 10, 10, 10, 10,  5,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

#[rustfmt::skip]
const KING_MG_PST: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

#[rustfmt::skip]
const KING_EG_PST: [i32; 64] = [
    -50,-40,-30,-20,-20,-30,-40,-50,
    -30,-20,-10,  0,  0,-10,-20,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-30,  0,  0,  0,  0,-30,-30,
    -50,-30,-30,-30,-30,-30,-30,-50
];

pub fn evaluate(board: &Board) -> i32 {
    let mut mg_score: i32 = 0;
    let mut eg_score: i32 = 0;
    let mut game_phase: i32 = 0;

    let w_pawns = board.pieces[(Piece::WHITE | Piece::PAWN) as usize];
    let b_pawns = board.pieces[(Piece::BLACK | Piece::PAWN) as usize];

    // 1. Piece & PST Evaluation
    let piece_types: [(u8, i32, &[i32; 64], &[i32; 64], i32); 6] = [
        (Piece::PAWN, PAWN_VAL, &PAWN_PST_MG, &PAWN_PST_MG, 0),
        (Piece::KNIGHT, KNIGHT_VAL, &KNIGHT_PST, &KNIGHT_PST, KNIGHT_PHASE),
        (Piece::BISHOP, BISHOP_VAL, &BISHOP_PST, &BISHOP_PST, BISHOP_PHASE),
        (Piece::ROOK, ROOK_VAL, &ROOK_PST_MG, &ROOK_PST_EG, ROOK_PHASE),
        (Piece::QUEEN, QUEEN_VAL, &QUEEN_PST_MG, &QUEEN_PST_EG, QUEEN_PHASE),
        (Piece::KING, KING_VAL, &KING_MG_PST, &KING_EG_PST, 0),
    ];

    for (pt, val, mg_pst, eg_pst, ph_val) in piece_types.iter() {
        let mut white_bb = board.pieces[(Piece::WHITE | pt) as usize];
        let white_count = white_bb.count_ones();
        while white_bb != 0 {
            let sq = white_bb.trailing_zeros() as usize;
            mg_score += val + mg_pst[sq];
            eg_score += val + eg_pst[sq];
            game_phase += ph_val;
            white_bb &= white_bb - 1;
        }

        let mut black_bb = board.pieces[(Piece::BLACK | pt) as usize];
        let black_count = black_bb.count_ones();
        while black_bb != 0 {
            let sq = black_bb.trailing_zeros() as usize;
            mg_score -= val + mg_pst[sq ^ 56];
            eg_score -= val + eg_pst[sq ^ 56];
            game_phase += ph_val;
            black_bb &= black_bb - 1;
        }

        if *pt == Piece::BISHOP {
            if white_count >= 2 { mg_score += BISHOP_PAIR_BONUS; eg_score += BISHOP_PAIR_BONUS; }
            if black_count >= 2 { mg_score -= BISHOP_PAIR_BONUS; eg_score -= BISHOP_PAIR_BONUS; }
        }
    }

    // 2. Optimized Pawn Structure
    let (w_p_mg, w_p_eg) = evaluate_pawn_structure(w_pawns, b_pawns, true);
    let (b_p_mg, b_p_eg) = evaluate_pawn_structure(b_pawns, w_pawns, false);

    mg_score += w_p_mg - b_p_mg;
    eg_score += w_p_eg - b_p_eg;

    // 3. Tapering Logic (integer)
    let phase = game_phase.min(TOTAL_PHASE);
    let final_score = (mg_score * phase + eg_score * (TOTAL_PHASE - phase)) / TOTAL_PHASE;

    if board.white_to_move { final_score } else { -final_score }
}

fn evaluate_pawn_structure(our_pawns: u64, enemy_pawns: u64, is_white: bool) -> (i32, i32) {
    let mut mg: i32 = 0;
    let mut eg: i32 = 0;

    for f in 0..8 {
        let file_pawns = our_pawns & FILE_MASKS[f];
        if file_pawns != 0 {
            // Doubled Pawns
            let count = file_pawns.count_ones() as i32;
            if count > 1 {
                let penalty = (count - 1) * DOUBLED_PAWN_PENALTY;
                mg += penalty; eg += penalty;
            }
            // Isolated Pawns
            if (our_pawns & ADJACENT_FILES[f]) == 0 {
                let penalty = count * ISOLATED_PAWN_PENALTY;
                mg += penalty; eg += penalty;
            }
        }
    }

    let mut pawns = our_pawns;
    while pawns != 0 {
        let sq = pawns.trailing_zeros() as usize;
        let mask = if is_white { PASSED_PAWN_MASKS_WHITE[sq] } else { PASSED_PAWN_MASKS_BLACK[sq] };

        if (mask & enemy_pawns) == 0 {
            let rank = sq / 8;
            let rel_rank = if is_white { rank } else { 7 - rank };
            let bonus = PASSED_PAWN_BONUS[rel_rank];
            mg += bonus / 2;
            eg += bonus;
        }
        pawns &= pawns - 1;
    }

    (mg, eg)
}
