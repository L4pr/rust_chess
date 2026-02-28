use crate::{Board, Piece};
use crate::move_generation::generate_moves::KNIGHT_MOVES;
use crate::move_generation::magic_bitboards::{get_bishop_attacks, get_rook_attacks};

// --- Phase Weights ---
const KNIGHT_PHASE: i32 = 1;
const BISHOP_PHASE: i32 = 1;
const ROOK_PHASE: i32 = 2;
const QUEEN_PHASE: i32 = 4;
const TOTAL_PHASE: i32 = 24;

// --- Material Values ---
const PAWN_VAL_MG: i32 = 82;
const PAWN_VAL_EG: i32 = 94;
const KNIGHT_VAL_MG: i32 = 337;
const KNIGHT_VAL_EG: i32 = 281;
const BISHOP_VAL_MG: i32 = 365;
const BISHOP_VAL_EG: i32 = 297;
const ROOK_VAL_MG: i32 = 477;
const ROOK_VAL_EG: i32 = 512;
const QUEEN_VAL_MG: i32 = 1025;
const QUEEN_VAL_EG: i32 = 936;
const KING_VAL: i32 = 20000;

// --- Specialized Bonuses & Penalties ---
const BISHOP_PAIR_BONUS_MG: i32 = 30;
const BISHOP_PAIR_BONUS_EG: i32 = 50;
const PASSED_PAWN_BONUS_MG: [i32; 8] = [0,  5, 10, 20, 35, 60, 100, 0];
const PASSED_PAWN_BONUS_EG: [i32; 8] = [0, 10, 20, 40, 70, 120, 200, 0];
const CONNECTED_PASSED_BONUS: i32 = 25;
const ISOLATED_PAWN_PENALTY_MG: i32 = -10;
const ISOLATED_PAWN_PENALTY_EG: i32 = -20;
const DOUBLED_PAWN_PENALTY: i32 = -15;
const BACKWARD_PAWN_PENALTY_MG: i32 = -8;
const BACKWARD_PAWN_PENALTY_EG: i32 = -10;

// --- Mobility Weights (centipawns per available square) ---
const KNIGHT_MOBILITY_MG: i32 = 4;
const KNIGHT_MOBILITY_EG: i32 = 4;
const BISHOP_MOBILITY_MG: i32 = 5;
const BISHOP_MOBILITY_EG: i32 = 5;
const ROOK_MOBILITY_MG: i32 = 2;
const ROOK_MOBILITY_EG: i32 = 4;
const QUEEN_MOBILITY_MG: i32 = 1;
const QUEEN_MOBILITY_EG: i32 = 2;

// --- Rook File Bonuses ---
const ROOK_OPEN_FILE_MG: i32 = 25;
const ROOK_OPEN_FILE_EG: i32 = 10;
const ROOK_SEMI_OPEN_FILE_MG: i32 = 12;
const ROOK_SEMI_OPEN_FILE_EG: i32 = 8;
const ROOK_ON_7TH_MG: i32 = 20;
const ROOK_ON_7TH_EG: i32 = 30;

// --- King Safety ---
const PAWN_SHIELD_BONUS: i32 = 10; // per shielding pawn
const PAWN_STORM_BONUS: i32 = 5;   // per enemy pawn near our king
const KING_OPEN_FILE_PENALTY: i32 = -25;

// --- Knight Outpost ---
const KNIGHT_OUTPOST_BONUS_MG: i32 = 20;
const KNIGHT_OUTPOST_BONUS_EG: i32 = 10;

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

const RANK_MASKS: [u64; 8] = [
    0xFF, 0xFF00, 0xFF0000, 0xFF000000,
    0xFF00000000, 0xFF0000000000, 0xFF000000000000, 0xFF00000000000000,
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
                let dist = if f > file { f - file } else { file - f };
                if dist <= 1 {
                    if is_white && r > rank {
                        mask |= 1 << (r * 8 + f);
                    } else if !is_white && r < rank {
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
// Values from PeSTO / Rofchade, widely used and tuned.

#[rustfmt::skip]
const PAWN_PST_MG: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    98, 134, 61, 95, 68, 126, 34, -11,
    -6,   7, 26, 31, 65,  56, 25, -20,
   -14,  13,  6, 21, 23,  12, 17, -23,
   -27,  -2, -5, 12, 17,   6, 10, -25,
   -26,  -4, -4,-10,  3,   3, 33, -12,
   -35,  -1,-20,-23,-15,  24, 38, -22,
     0,   0,  0,  0,  0,   0,  0,   0,
];

#[rustfmt::skip]
const PAWN_PST_EG: [i32; 64] = [
     0,   0,  0,  0,  0,  0,  0,  0,
   178, 173,158,134,147,132,165,187,
    94, 100, 85, 67, 56, 53, 82, 84,
    32,  24, 13,  5, -2,  4, 17, 17,
    13,   9, -3, -7, -7, -8,  3, -1,
     4,   7, -6,  1,  0, -5, -1, -8,
    13,   8,  8,-10, -6, -3, -4, -14,
     0,   0,  0,  0,  0,  0,  0,  0,
];

#[rustfmt::skip]
const KNIGHT_PST_MG: [i32; 64] = [
   -167, -89, -34, -49,  61, -97, -15,-107,
    -73, -41,  72,  36,  23,  62,   7, -17,
    -47,  60,  37,  65,  84, 129,  73,  44,
     -9,  17,  19,  53,  37,  69,  18,  22,
    -13,   4,  16,  13,  28,  19,  21,  -8,
    -23,  -9,  12,  10,  19,  17,  25, -16,
    -29, -53, -12,  -3,  -1,  18, -14, -19,
   -105, -21, -58, -33, -17, -28, -19, -23,
];

#[rustfmt::skip]
const KNIGHT_PST_EG: [i32; 64] = [
    -58, -38, -13, -28, -31, -27, -63, -99,
    -25,  -8, -25,  -2,  -9, -25, -24, -52,
    -24, -20,  10,   9,  -1,  -9, -19, -41,
    -17,   3,  22,  22,  22,  11,   8, -18,
    -18,  -6,  16,  25,  16,  17,   4, -18,
    -23,  -3,  -1,  15,  10,  -3, -20, -22,
    -42, -20, -10,  -5,  -2, -20, -23, -44,
    -29, -51, -23, -15, -22, -18, -50, -64,
];

#[rustfmt::skip]
const BISHOP_PST_MG: [i32; 64] = [
    -29,   4, -82, -37, -25, -42,   7,  -8,
    -26,  16, -18, -13,  30,  59,  18, -47,
    -16,  37,  43,  40,  35,  50,  37,  -2,
     -4,   5,  19,  50,  37,  37,   7,  -2,
     -6,  13,  13,  26,  34,  12,  10,   4,
      0,  15,  15,  15,  14,  27,  18,  10,
      4,  15,  16,   0,   7,  21,  33,   1,
    -33,  -3, -14, -21, -13, -12, -39, -21,
];

#[rustfmt::skip]
const BISHOP_PST_EG: [i32; 64] = [
    -14, -21, -11,  -8,  -7,  -9, -17, -24,
     -8,  -4,   7, -12,  -3, -13,  -4, -14,
      2,  -8,   0,  -1,  -2,   6,   0,   4,
     -3,   9,  12,   9,  14,  10,   3,   2,
     -6,   3,  13,  19,   7,  10,  -3,  -9,
    -12,  -3,   8,  10,  13,   3,  -7, -15,
    -14, -18,  -7,  -1,   4,  -9, -15, -27,
    -23,  -9, -23,  -5,  -9, -16,  -5, -17,
];

#[rustfmt::skip]
const ROOK_PST_MG: [i32; 64] = [
     32,  42,  32,  51,  63,   9,  31,  43,
     27,  32,  58,  62,  80,  67,  26,  44,
     -5,  19,  26,  36,  17,  45,  61,  16,
    -24, -11,   7,  26,  24,  35,  -8, -20,
    -36, -26, -12,  -1,   9,  -7,   6, -23,
    -45, -25, -16, -17,   3,   0,  -5, -33,
    -44, -16, -20,  -9,  -1,  11,  -6, -71,
    -19, -13,   1,  17,  16,   7, -37, -26,
];

#[rustfmt::skip]
const ROOK_PST_EG: [i32; 64] = [
    13, 10, 18, 15, 12,  12,   8,   5,
    11, 13, 13, 11, -3,   3,   8,   3,
     7,  7,  7,  5,  4,  -3,  -5,  -3,
     4,  3, 13,  1,  2,   1,  -1,   2,
     3,  5,  8,  4, -5,  -6,  -8, -11,
    -4,  0, -5, -1, -7, -12,  -8, -16,
    -6, -6,  0,  2, -9,  -9, -11,  -3,
    -9,  2,  3, -1, -5, -13,   4, -20,
];

#[rustfmt::skip]
const QUEEN_PST_MG: [i32; 64] = [
    -28,   0,  29,  12,  59,  44,  43,  45,
    -24, -39,  -5,   1, -16,  57,  28,  54,
    -13, -17,   7,   8,  29,  56,  47,  57,
    -27, -27, -16, -16,  -1,  17,  -2,   1,
     -9, -26,  -9, -10,  -2,  -4,   3,  -3,
    -14,   2, -11,  -2,  -5,   2,  14,   5,
    -35,  -8,  11,   2,   8,  15,  -3,   1,
     -1, -18,  -9,  10, -15, -25, -31, -50,
];

#[rustfmt::skip]
const QUEEN_PST_EG: [i32; 64] = [
     -9,  22,  22,  27,  27,  19,  10,  20,
    -17,  20,  32,  41,  58,  25,  30,   0,
    -20,   6,   9,  49,  47,  35,  19,   9,
      3,  22,  24,  45,  57,  40,  57,  36,
    -18,  28,  19,  47,  31,  34,  39,  23,
    -16, -27,  15,   6,   9,  17,  10,   5,
    -22, -23, -30, -16, -16, -23, -36, -32,
    -33, -28, -22, -43,  -5, -32, -20, -41,
];

#[rustfmt::skip]
const KING_PST_MG: [i32; 64] = [
    -65,  23,  16, -15, -56, -34,   2,  13,
     29,  -1, -20,  -7,  -8,  -4, -38, -29,
     -9,  24,   2, -16, -20,   6,  22, -22,
    -17, -20, -12, -27, -30, -25,  -14, -36,
    -49,  -1, -27, -39, -46, -44,  -33, -51,
    -14, -14, -22, -46, -44, -30,  -15, -27,
      1,   7,  -8, -64, -43, -16,   9,   8,
    -15,  36,  12, -54,   8, -28,  24,  14,
];

#[rustfmt::skip]
const KING_PST_EG: [i32; 64] = [
    -74, -35, -18, -18, -11,  15,   4, -17,
    -12,  17,  14,  17,  17,  38,  23,  11,
     10,  17,  23,  15,  20,  45,  44,  13,
     -8,  22,  24,  27,  26,  33,  26,   3,
    -18,  -4,  21,  24,  27,  23,   9, -11,
    -19,  -3,  11,  21,  23,  16,   7,  -9,
    -27, -11,   4,  13,  14,   4,  -5, -17,
    -53, -34, -21, -11, -28, -14, -24, -43,
];

pub fn evaluate(board: &Board) -> i32 {
    let mut mg: i32 = 0;
    let mut eg: i32 = 0;
    let mut game_phase: i32 = 0;

    let w_pawns = board.pieces[(Piece::WHITE | Piece::PAWN) as usize];
    let b_pawns = board.pieces[(Piece::BLACK | Piece::PAWN) as usize];
    let all_pawns = w_pawns | b_pawns;
    let occupancy = board.pieces[0];
    let w_occ = board.pieces[(Piece::WHITE | Piece::ALL) as usize];
    let b_occ = board.pieces[(Piece::BLACK | Piece::ALL) as usize];

    // =============================================
    // 1. Material + PST (with separate MG/EG PSTs)
    // =============================================
    // Pawns
    eval_piece_pst(board, Piece::PAWN, PAWN_VAL_MG, PAWN_VAL_EG, &PAWN_PST_MG, &PAWN_PST_EG, 0, &mut mg, &mut eg, &mut game_phase);
    // Knights
    eval_piece_pst(board, Piece::KNIGHT, KNIGHT_VAL_MG, KNIGHT_VAL_EG, &KNIGHT_PST_MG, &KNIGHT_PST_EG, KNIGHT_PHASE, &mut mg, &mut eg, &mut game_phase);
    // Bishops
    eval_piece_pst(board, Piece::BISHOP, BISHOP_VAL_MG, BISHOP_VAL_EG, &BISHOP_PST_MG, &BISHOP_PST_EG, BISHOP_PHASE, &mut mg, &mut eg, &mut game_phase);
    // Rooks
    eval_piece_pst(board, Piece::ROOK, ROOK_VAL_MG, ROOK_VAL_EG, &ROOK_PST_MG, &ROOK_PST_EG, ROOK_PHASE, &mut mg, &mut eg, &mut game_phase);
    // Queens
    eval_piece_pst(board, Piece::QUEEN, QUEEN_VAL_MG, QUEEN_VAL_EG, &QUEEN_PST_MG, &QUEEN_PST_EG, QUEEN_PHASE, &mut mg, &mut eg, &mut game_phase);
    // Kings (no material value in phase, but PST matters)
    eval_piece_pst(board, Piece::KING, KING_VAL, KING_VAL, &KING_PST_MG, &KING_PST_EG, 0, &mut mg, &mut eg, &mut game_phase);

    // =============================================
    // 2. Bishop Pair
    // =============================================
    let w_bishop_count = board.pieces[(Piece::WHITE | Piece::BISHOP) as usize].count_ones();
    let b_bishop_count = board.pieces[(Piece::BLACK | Piece::BISHOP) as usize].count_ones();
    if w_bishop_count >= 2 { mg += BISHOP_PAIR_BONUS_MG; eg += BISHOP_PAIR_BONUS_EG; }
    if b_bishop_count >= 2 { mg -= BISHOP_PAIR_BONUS_MG; eg -= BISHOP_PAIR_BONUS_EG; }

    // =============================================
    // 3. Pawn Structure
    // =============================================
    let (wp_mg, wp_eg) = evaluate_pawn_structure(w_pawns, b_pawns, true);
    let (bp_mg, bp_eg) = evaluate_pawn_structure(b_pawns, w_pawns, false);
    mg += wp_mg - bp_mg;
    eg += wp_eg - bp_eg;

    // =============================================
    // 4. Piece Mobility
    // =============================================
    // Knights
    eval_knight_mobility(board, Piece::WHITE, w_occ, &mut mg, &mut eg);
    eval_knight_mobility(board, Piece::BLACK, b_occ, &mut mg, &mut eg);
    // Bishops
    eval_slider_mobility(board, Piece::BISHOP, Piece::WHITE, w_occ, occupancy, BISHOP_MOBILITY_MG, BISHOP_MOBILITY_EG, &mut mg, &mut eg);
    eval_slider_mobility(board, Piece::BISHOP, Piece::BLACK, b_occ, occupancy, BISHOP_MOBILITY_MG, BISHOP_MOBILITY_EG, &mut mg, &mut eg);
    // Rooks
    eval_slider_mobility(board, Piece::ROOK, Piece::WHITE, w_occ, occupancy, ROOK_MOBILITY_MG, ROOK_MOBILITY_EG, &mut mg, &mut eg);
    eval_slider_mobility(board, Piece::ROOK, Piece::BLACK, b_occ, occupancy, ROOK_MOBILITY_MG, ROOK_MOBILITY_EG, &mut mg, &mut eg);
    // Queens
    eval_queen_mobility(board, Piece::WHITE, w_occ, occupancy, &mut mg, &mut eg);
    eval_queen_mobility(board, Piece::BLACK, b_occ, occupancy, &mut mg, &mut eg);

    // =============================================
    // 5. Rook on Open / Semi-Open Files + 7th Rank
    // =============================================
    eval_rook_files(board, Piece::WHITE, w_pawns, b_pawns, &mut mg, &mut eg);
    eval_rook_files(board, Piece::BLACK, b_pawns, w_pawns, &mut mg, &mut eg);

    // =============================================
    // 6. King Safety (MG only — less relevant in EG)
    // =============================================
    eval_king_safety(board, Piece::WHITE, w_pawns, b_pawns, all_pawns, &mut mg);
    eval_king_safety(board, Piece::BLACK, b_pawns, w_pawns, all_pawns, &mut mg);

    // =============================================
    // 7. Knight Outposts
    // =============================================
    eval_knight_outposts(board, Piece::WHITE, w_pawns, b_pawns, &mut mg, &mut eg);
    eval_knight_outposts(board, Piece::BLACK, b_pawns, w_pawns, &mut mg, &mut eg);

    // =============================================
    // 8. Tapering
    // =============================================
    let phase = game_phase.min(TOTAL_PHASE);
    let score = (mg * phase + eg * (TOTAL_PHASE - phase)) / TOTAL_PHASE;

    if board.white_to_move { score } else { -score }
}

// =======================================================
// Helper: Material + PST for one piece type
// =======================================================
#[inline]
fn eval_piece_pst(
    board: &Board, pt: u8, val_mg: i32, val_eg: i32,
    pst_mg: &[i32; 64], pst_eg: &[i32; 64],
    phase_val: i32,
    mg: &mut i32, eg: &mut i32, game_phase: &mut i32,
) {
    let mut white_bb = board.pieces[(Piece::WHITE | pt) as usize];
    while white_bb != 0 {
        let sq = white_bb.trailing_zeros() as usize;
        *mg += val_mg + pst_mg[sq];
        *eg += val_eg + pst_eg[sq];
        *game_phase += phase_val;
        white_bb &= white_bb - 1;
    }

    let mut black_bb = board.pieces[(Piece::BLACK | pt) as usize];
    while black_bb != 0 {
        let sq = black_bb.trailing_zeros() as usize;
        *mg -= val_mg + pst_mg[sq ^ 56]; // mirror vertically for black
        *eg -= val_eg + pst_eg[sq ^ 56];
        *game_phase += phase_val;
        black_bb &= black_bb - 1;
    }
}

// =======================================================
// Helper: Knight Mobility
// =======================================================
#[inline]
fn eval_knight_mobility(board: &Board, color: u8, friendly_occ: u64, mg: &mut i32, eg: &mut i32) {
    let sign = if color == Piece::WHITE { 1 } else { -1 };
    let mut knights = board.pieces[(color | Piece::KNIGHT) as usize];
    while knights != 0 {
        let sq = knights.trailing_zeros() as usize;
        let moves = (KNIGHT_MOVES[sq] & !friendly_occ).count_ones() as i32;
        *mg += sign * KNIGHT_MOBILITY_MG * (moves - 4); // center around 4 squares
        *eg += sign * KNIGHT_MOBILITY_EG * (moves - 4);
        knights &= knights - 1;
    }
}

// =======================================================
// Helper: Slider Mobility (Bishop / Rook)
// =======================================================
#[inline]
fn eval_slider_mobility(
    board: &Board, pt: u8, color: u8, friendly_occ: u64, occupancy: u64,
    mob_mg: i32, mob_eg: i32, mg: &mut i32, eg: &mut i32,
) {
    let sign = if color == Piece::WHITE { 1 } else { -1 };
    let mut pieces = board.pieces[(color | pt) as usize];
    while pieces != 0 {
        let sq = pieces.trailing_zeros() as u8;
        let attacks = if pt == Piece::BISHOP {
            get_bishop_attacks(sq, occupancy)
        } else {
            get_rook_attacks(sq, occupancy)
        };
        let moves = (attacks & !friendly_occ).count_ones() as i32;
        let center = if pt == Piece::BISHOP { 7 } else { 7 };
        *mg += sign * mob_mg * (moves - center);
        *eg += sign * mob_eg * (moves - center);
        pieces &= pieces - 1;
    }
}

// =======================================================
// Helper: Queen Mobility (combined bishop + rook attacks)
// =======================================================
#[inline]
fn eval_queen_mobility(board: &Board, color: u8, friendly_occ: u64, occupancy: u64, mg: &mut i32, eg: &mut i32) {
    let sign = if color == Piece::WHITE { 1 } else { -1 };
    let mut queens = board.pieces[(color | Piece::QUEEN) as usize];
    while queens != 0 {
        let sq = queens.trailing_zeros() as u8;
        let attacks = get_bishop_attacks(sq, occupancy) | get_rook_attacks(sq, occupancy);
        let moves = (attacks & !friendly_occ).count_ones() as i32;
        *mg += sign * QUEEN_MOBILITY_MG * (moves - 14);
        *eg += sign * QUEEN_MOBILITY_EG * (moves - 14);
        queens &= queens - 1;
    }
}

// =======================================================
// Helper: Rook on Open / Semi-Open Files + 7th Rank
// =======================================================
#[inline]
fn eval_rook_files(board: &Board, color: u8, our_pawns: u64, enemy_pawns: u64, mg: &mut i32, eg: &mut i32) {
    let sign = if color == Piece::WHITE { 1 } else { -1 };
    let seventh_rank = if color == Piece::WHITE { RANK_MASKS[6] } else { RANK_MASKS[1] };
    let mut rooks = board.pieces[(color | Piece::ROOK) as usize];
    while rooks != 0 {
        let sq = rooks.trailing_zeros() as usize;
        let file = sq % 8;
        let file_mask = FILE_MASKS[file];

        if (our_pawns & file_mask) == 0 {
            if (enemy_pawns & file_mask) == 0 {
                // Fully open file
                *mg += sign * ROOK_OPEN_FILE_MG;
                *eg += sign * ROOK_OPEN_FILE_EG;
            } else {
                // Semi-open file
                *mg += sign * ROOK_SEMI_OPEN_FILE_MG;
                *eg += sign * ROOK_SEMI_OPEN_FILE_EG;
            }
        }

        // Rook on 7th rank (2nd for black)
        if (1u64 << sq) & seventh_rank != 0 {
            *mg += sign * ROOK_ON_7TH_MG;
            *eg += sign * ROOK_ON_7TH_EG;
        }

        rooks &= rooks - 1;
    }
}

// =======================================================
// Helper: King Safety (MG weighted)
// =======================================================
#[inline]
fn eval_king_safety(board: &Board, color: u8, our_pawns: u64, enemy_pawns: u64, all_pawns: u64, mg: &mut i32) {
    let sign = if color == Piece::WHITE { 1 } else { -1 };
    let king_bb = board.pieces[(color | Piece::KING) as usize];
    if king_bb == 0 { return; }
    let king_sq = king_bb.trailing_zeros() as usize;
    let king_file = king_sq % 8;

    // Pawn shield: count friendly pawns on king's file and adjacent files, one or two ranks ahead
    let shield_mask = {
        let mut m = 0u64;
        let start_f = if king_file > 0 { king_file - 1 } else { 0 };
        let end_f = if king_file < 7 { king_file + 1 } else { 7 };
        for f in start_f..=end_f {
            m |= FILE_MASKS[f];
        }
        // Only consider ranks just ahead of the king
        if color == Piece::WHITE {
            let king_rank = king_sq / 8;
            if king_rank < 6 {
                m &= RANK_MASKS[king_rank + 1] | RANK_MASKS[king_rank + 2];
            } else {
                m &= RANK_MASKS[7];
            }
        } else {
            let king_rank = king_sq / 8;
            if king_rank > 1 {
                m &= RANK_MASKS[king_rank - 1] | RANK_MASKS[king_rank - 2];
            } else {
                m &= RANK_MASKS[0];
            }
        }
        m
    };

    let shield_count = (our_pawns & shield_mask).count_ones() as i32;
    *mg += sign * PAWN_SHIELD_BONUS * shield_count;

    // Pawn storm: enemy pawns near our king
    let storm_count = (enemy_pawns & shield_mask).count_ones() as i32;
    *mg -= sign * PAWN_STORM_BONUS * storm_count;

    // Open file near king penalty
    let king_file_mask = FILE_MASKS[king_file];
    if (all_pawns & king_file_mask) == 0 {
        *mg += sign * KING_OPEN_FILE_PENALTY;
    }
}

// =======================================================
// Helper: Knight Outposts
// =======================================================
#[inline]
fn eval_knight_outposts(board: &Board, color: u8, our_pawns: u64, enemy_pawns: u64, mg: &mut i32, eg: &mut i32) {
    let sign = if color == Piece::WHITE { 1 } else { -1 };
    let mut knights = board.pieces[(color | Piece::KNIGHT) as usize];

    // Outpost: knight on rank 4-6 (white) / 3-5 (black), supported by our pawn,
    // and cannot be attacked by an enemy pawn
    let outpost_ranks = if color == Piece::WHITE {
        RANK_MASKS[3] | RANK_MASKS[4] | RANK_MASKS[5]
    } else {
        RANK_MASKS[2] | RANK_MASKS[3] | RANK_MASKS[4]
    };

    while knights != 0 {
        let sq = knights.trailing_zeros() as usize;
        let sq_bit = 1u64 << sq;

        if sq_bit & outpost_ranks != 0 {
            let file = sq % 8;
            // Check no enemy pawns can attack this square
            let enemy_pawn_attack_files = ADJACENT_FILES[file];
            let enemy_pawn_mask = if color == Piece::WHITE {
                PASSED_PAWN_MASKS_WHITE[sq] & enemy_pawn_attack_files
            } else {
                PASSED_PAWN_MASKS_BLACK[sq] & enemy_pawn_attack_files
            };

            if (enemy_pawns & enemy_pawn_mask) == 0 {
                // Supported by our pawn?
                let pawn_support = if color == Piece::WHITE {
                    let left = if file > 0 { sq_bit >> 9 } else { 0 };
                    let right = if file < 7 { sq_bit >> 7 } else { 0 };
                    left | right
                } else {
                    let left = if file > 0 { sq_bit << 7 } else { 0 };
                    let right = if file < 7 { sq_bit << 9 } else { 0 };
                    left | right
                };

                if (our_pawns & pawn_support) != 0 {
                    *mg += sign * KNIGHT_OUTPOST_BONUS_MG;
                    *eg += sign * KNIGHT_OUTPOST_BONUS_EG;
                }
            }
        }
        knights &= knights - 1;
    }
}

// =======================================================
// Pawn Structure Evaluation
// =======================================================
fn evaluate_pawn_structure(our_pawns: u64, enemy_pawns: u64, is_white: bool) -> (i32, i32) {
    let mut mg: i32 = 0;
    let mut eg: i32 = 0;

    for f in 0..8usize {
        let file_pawns = our_pawns & FILE_MASKS[f];
        if file_pawns != 0 {
            let count = file_pawns.count_ones() as i32;
            // Doubled pawns
            if count > 1 {
                let penalty = (count - 1) * DOUBLED_PAWN_PENALTY;
                mg += penalty;
                eg += penalty;
            }
            // Isolated pawns
            if (our_pawns & ADJACENT_FILES[f]) == 0 {
                mg += count * ISOLATED_PAWN_PENALTY_MG;
                eg += count * ISOLATED_PAWN_PENALTY_EG;
            }
        }
    }

    // Per-pawn evaluation: passed pawns, backward pawns, connected passed pawns
    let mut pawns = our_pawns;
    while pawns != 0 {
        let sq = pawns.trailing_zeros() as usize;
        let file = sq % 8;
        let rank = sq / 8;
        let rel_rank = if is_white { rank } else { 7 - rank };
        let passed_mask = if is_white { PASSED_PAWN_MASKS_WHITE[sq] } else { PASSED_PAWN_MASKS_BLACK[sq] };

        let is_passed = (passed_mask & enemy_pawns) == 0;

        if is_passed {
            mg += PASSED_PAWN_BONUS_MG[rel_rank];
            eg += PASSED_PAWN_BONUS_EG[rel_rank];

            // Connected passed pawn: adjacent file pawn on same or adjacent rank
            if file > 0 {
                let adj_file = FILE_MASKS[file - 1];
                let near_ranks = if rank > 0 && rank < 7 {
                    RANK_MASKS[rank - 1] | RANK_MASKS[rank] | RANK_MASKS[rank + 1]
                } else if rank == 0 {
                    RANK_MASKS[0] | RANK_MASKS[1]
                } else {
                    RANK_MASKS[6] | RANK_MASKS[7]
                };
                if (our_pawns & adj_file & near_ranks) != 0 {
                    mg += CONNECTED_PASSED_BONUS;
                    eg += CONNECTED_PASSED_BONUS;
                }
            }
            if file < 7 {
                let adj_file = FILE_MASKS[file + 1];
                let near_ranks = if rank > 0 && rank < 7 {
                    RANK_MASKS[rank - 1] | RANK_MASKS[rank] | RANK_MASKS[rank + 1]
                } else if rank == 0 {
                    RANK_MASKS[0] | RANK_MASKS[1]
                } else {
                    RANK_MASKS[6] | RANK_MASKS[7]
                };
                if (our_pawns & adj_file & near_ranks) != 0 {
                    // Don't double-count — only count once per pair
                    // We already counted from the left, so skip
                }
            }
        } else {
            // Backward pawn: not passed, no friendly pawn on adjacent files that is behind or equal rank
            let behind_mask = if is_white {
                // All squares on adjacent files at same rank or behind
                let mut m = 0u64;
                for r in 0..=rank {
                    m |= RANK_MASKS[r];
                }
                m & ADJACENT_FILES[file]
            } else {
                let mut m = 0u64;
                for r in rank..8 {
                    m |= RANK_MASKS[r];
                }
                m & ADJACENT_FILES[file]
            };

            if (our_pawns & behind_mask) == 0 {
                // It's backward — no support from adjacent pawns
                // Also check the square ahead is controlled by enemy pawn
                let ahead_sq = if is_white { sq + 8 } else { sq.wrapping_sub(8) };
                if ahead_sq < 64 {
                    let ahead_bit = 1u64 << ahead_sq;
                    let enemy_attacks = if is_white {
                        ((ahead_bit >> 7) & 0xfefefefefefefefe) | ((ahead_bit >> 9) & 0x7f7f7f7f7f7f7f7f)
                    } else {
                        ((ahead_bit << 7) & 0x7f7f7f7f7f7f7f7f) | ((ahead_bit << 9) & 0xfefefefefefefefe)
                    };
                    if (enemy_pawns & enemy_attacks) != 0 {
                        mg += BACKWARD_PAWN_PENALTY_MG;
                        eg += BACKWARD_PAWN_PENALTY_EG;
                    }
                }
            }
        }

        pawns &= pawns - 1;
    }

    (mg, eg)
}
