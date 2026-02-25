use crate::{Board, Piece};

// --- Phase Weights ---
const KNIGHT_PHASE: i32 = 1;
const BISHOP_PHASE: i32 = 1;
const ROOK_PHASE: i32 = 2;
const QUEEN_PHASE: i32 = 4;
const TOTAL_PHASE: i32 = 24;

// --- Material Values ---
const PAWN_VAL: f64 = 100.0;
const KNIGHT_VAL: f64 = 320.0;
const BISHOP_VAL: f64 = 330.0;
const ROOK_VAL: f64 = 500.0;
const QUEEN_VAL: f64 = 900.0;
const KING_VAL: f64 = 20000.0;

// --- Specialized Bonuses & Penalties ---
const BISHOP_PAIR_BONUS: f64 = 30.0;
const PASSED_PAWN_BONUS: [f64; 8] = [0.0, 10.0, 20.0, 40.0, 70.0, 120.0, 200.0, 0.0];
const ISOLATED_PAWN_PENALTY: f64 = -15.0;
const DOUBLED_PAWN_PENALTY: f64 = -15.0;

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
const PAWN_PST_MG: [f64; 64] = [
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
    50.0, 50.0, 50.0, 50.0, 50.0, 50.0, 50.0, 50.0,
    10.0, 10.0, 20.0, 30.0, 30.0, 20.0, 10.0, 10.0,
     5.0,  5.0, 10.0, 25.0, 25.0, 10.0,  5.0,  5.0,
     0.0,  0.0,  0.0, 20.0, 20.0,  0.0,  0.0,  0.0,
     5.0, -5.0,-10.0,  0.0,  0.0,-10.0, -5.0,  5.0,
     5.0, 10.0, 10.0,-20.0,-20.0, 10.0, 10.0,  5.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0
];

#[rustfmt::skip]
const KNIGHT_PST: [f64; 64] = [
    -50.0,-40.0,-30.0,-30.0,-30.0,-30.0,-40.0,-50.0,
    -40.0,-20.0,  0.0,  0.0,  0.0,  0.0,-20.0,-40.0,
    -30.0,  0.0, 10.0, 15.0, 15.0, 10.0,  0.0,-30.0,
    -30.0,  5.0, 15.0, 20.0, 20.0, 15.0,  5.0,-30.0,
    -30.0,  0.0, 15.0, 20.0, 20.0, 15.0,  0.0,-30.0,
    -30.0,  5.0, 10.0, 15.0, 15.0, 10.0,  5.0,-30.0,
    -40.0,-20.0,  0.0,  5.0,  5.0,  0.0,-20.0,-40.0,
    -50.0,-40.0,-30.0,-30.0,-30.0,-30.0,-40.0,-50.0
];

#[rustfmt::skip]
const BISHOP_PST: [f64; 64] = [
    -20.0,-10.0,-10.0,-10.0,-10.0,-10.0,-10.0,-20.0,
    -10.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,-10.0,
    -10.0,  0.0,  5.0, 10.0, 10.0,  5.0,  0.0,-10.0,
    -10.0,  5.0,  5.0, 10.0, 10.0,  5.0,  5.0,-10.0,
    -10.0,  0.0, 10.0, 10.0, 10.0, 10.0,  0.0,-10.0,
    -10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0,-10.0,
    -10.0,  5.0,  0.0,  0.0,  0.0,  0.0,  5.0,-10.0,
    -20.0,-10.0,-10.0,-10.0,-10.0,-10.0,-10.0,-20.0
];

#[rustfmt::skip]
const ROOK_PST_MG: [f64; 64] = [
      0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
      5.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0,  5.0,
     -5.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, -5.0,
     -5.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, -5.0,
     -5.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, -5.0,
     -5.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, -5.0,
     -5.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, -5.0,
      0.0,  0.0,  0.0,  5.0,  5.0,  0.0,  0.0,  0.0
];

#[rustfmt::skip]
const ROOK_PST_EG: [f64; 64] = [
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
    10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,
     0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0
];

#[rustfmt::skip]
const QUEEN_PST_MG: [f64; 64] = [
    -20.0,-10.0,-10.0, -5.0, -5.0,-10.0,-10.0,-20.0,
    -10.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,-10.0,
    -10.0,  0.0,  5.0,  5.0,  5.0,  5.0,  0.0,-10.0,
     -5.0,  0.0,  5.0,  5.0,  5.0,  5.0,  0.0, -5.0,
      0.0,  0.0,  5.0,  5.0,  5.0,  5.0,  0.0, -5.0,
    -10.0,  5.0,  5.0,  5.0,  5.0,  5.0,  0.0,-10.0,
    -10.0,  0.0,  5.0,  0.0,  0.0,  0.0,  0.0,-10.0,
    -20.0,-10.0,-10.0, -5.0, -5.0,-10.0,-10.0,-20.0
];

#[rustfmt::skip]
const QUEEN_PST_EG: [f64; 64] = [
    -20.0,-10.0,-10.0, -5.0, -5.0,-10.0,-10.0,-20.0,
    -10.0,  0.0,  5.0,  5.0,  5.0,  5.0,  0.0,-10.0,
    -10.0,  5.0, 10.0, 10.0, 10.0, 10.0,  5.0,-10.0,
     -5.0,  5.0, 10.0, 20.0, 20.0, 10.0,  5.0, -5.0,
     -5.0,  5.0, 10.0, 20.0, 20.0, 10.0,  5.0, -5.0,
    -10.0,  5.0, 10.0, 10.0, 10.0, 10.0,  5.0,-10.0,
    -10.0,  0.0,  5.0,  5.0,  5.0,  5.0,  0.0,-10.0,
    -20.0,-10.0,-10.0, -5.0, -5.0,-10.0,-10.0,-20.0
];

#[rustfmt::skip]
const KING_MG_PST: [f64; 64] = [
    -30.0,-40.0,-40.0,-50.0,-50.0,-40.0,-40.0,-30.0,
    -30.0,-40.0,-40.0,-50.0,-50.0,-40.0,-40.0,-30.0,
    -30.0,-40.0,-40.0,-50.0,-50.0,-40.0,-40.0,-30.0,
    -30.0,-40.0,-40.0,-50.0,-50.0,-40.0,-40.0,-30.0,
    -20.0,-30.0,-30.0,-40.0,-40.0,-30.0,-30.0,-20.0,
    -10.0,-20.0,-20.0,-20.0,-20.0,-20.0,-20.0,-10.0,
     20.0, 20.0,  0.0,  0.0,  0.0,  0.0, 20.0, 20.0,
     20.0, 30.0, 10.0,  0.0,  0.0, 10.0, 30.0, 20.0
];

#[rustfmt::skip]
const KING_EG_PST: [f64; 64] = [
    -50.0,-40.0,-30.0,-20.0,-20.0,-30.0,-40.0,-50.0,
    -30.0,-20.0,-10.0,  0.0,  0.0,-10.0,-20.0,-30.0,
    -30.0,-10.0, 20.0, 30.0, 30.0, 20.0,-10.0,-30.0,
    -30.0,-10.0, 30.0, 40.0, 40.0, 30.0,-10.0,-30.0,
    -30.0,-10.0, 30.0, 40.0, 40.0, 30.0,-10.0,-30.0,
    -30.0,-10.0, 20.0, 30.0, 30.0, 20.0,-10.0,-30.0,
    -30.0,-30.0,  0.0,  0.0,  0.0,  0.0,-30.0,-30.0,
    -50.0,-30.0,-30.0,-30.0,-30.0,-30.0,-30.0,-50.0
];

pub fn evaluate(board: &Board) -> f64 {
    let mut mg_score = 0.0;
    let mut eg_score = 0.0;
    let mut game_phase = 0;

    let w_pawns = board.pieces[(Piece::WHITE | Piece::PAWN) as usize];
    let b_pawns = board.pieces[(Piece::BLACK | Piece::PAWN) as usize];

    // 1. Piece & PST Evaluation
    let piece_types = [
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

    // 3. Tapering Logic
    let phase = (game_phase.min(TOTAL_PHASE) as f64) / (TOTAL_PHASE as f64);
    let final_score = (mg_score * phase) + (eg_score * (1.0 - phase));

    if board.white_to_move { final_score } else { -final_score }
}

fn evaluate_pawn_structure(our_pawns: u64, enemy_pawns: u64, is_white: bool) -> (f64, f64) {
    let mut mg = 0.0;
    let mut eg = 0.0;

    for f in 0..8 {
        let file_pawns = our_pawns & FILE_MASKS[f];
        if file_pawns != 0 {
            // Doubled Pawns
            let count = file_pawns.count_ones();
            if count > 1 {
                let penalty = (count - 1) as f64 * DOUBLED_PAWN_PENALTY;
                mg += penalty; eg += penalty;
            }
            // Isolated Pawns
            if (our_pawns & ADJACENT_FILES[f]) == 0 {
                let penalty = count as f64 * ISOLATED_PAWN_PENALTY;
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
            mg += bonus * 0.5;
            eg += bonus;
        }
        pawns &= pawns - 1;
    }

    (mg, eg)
}
