use crate::board::board::Board;
use crate::board::move_struct::Move;
use crate::board::pieces::*;
pub fn generate_all_moves(board: &Board, moves: &mut [Move]) -> usize {
    let mut curr_move_index = 0;

    generate_pawn_moves(board, moves, &mut curr_move_index);
    generate_knight_moves(board, moves, &mut curr_move_index);
    generate_bishop_moves(board, moves, &mut curr_move_index);
    generate_rook_moves(board, moves, &mut curr_move_index);
    generate_queen_moves(board, moves, &mut curr_move_index);
    generate_king_moves(board, moves, &mut curr_move_index);

    curr_move_index
}


fn generate_pawn_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let mut pawns = board.pieces[Piece::new(us, Piece::PAWN).0 as usize];
    while pawns != 0 {
        let from = pawns.trailing_zeros() as u8;
        pawns &= pawns - 1; // Clear the bit for the current pawn

        // Single Push
        let to = if board.white_to_move { from + 8 } else { from - 8 };

        if (board.pieces[0] & (1u64 << to)) == 0 {
            if to >= 56 || to <= 7 {
                push_promotions(moves, curr_move_index, from, to, false);
            } else {
                moves[*curr_move_index] = Move::new_with_flags(from, to, Move::QUIET);
                *curr_move_index += 1;

                // DOUBLE PUSH
                // Only check if single push was successful and we are on the starting rank
                let start_rank = if board.white_to_move { from / 8 == 1 } else { from / 8 == 6 };
                if start_rank {
                    let double_to = if board.white_to_move { from + 16 } else { from - 16 };
                    if (board.pieces[0] & (1u64 << double_to)) == 0 {
                        moves[*curr_move_index] = Move::new_with_flags(from, double_to, Move::DOUBLE_PAWN_PUSH);
                        *curr_move_index += 1;
                    }
                }
            }
        }

        // Captures
        let capture_offsets = if board.white_to_move { [7i8, 9i8] } else { [-7i8, -9i8] };

        for &offset in &capture_offsets {
            let target_sq = (from as i8 + offset) as u8;

            // Prevent wrapping (e.g., a-file pawn capturing left to h-file)
            let from_file = from & 7;
            let target_file = target_sq & 7;
            if (from_file as i8 - target_file as i8).abs() > 1 { continue; }
            if target_sq > 63 { continue; } // Safety check for board boundaries

            let target_bit = 1u64 << target_sq;

            // Standard Captures
            if (target_bit & board.pieces[((us ^ 8) | Piece::ALL) as usize]) != 0 {
                if target_sq >= 56 || target_sq <= 7 {
                    push_promotions(moves, curr_move_index, from, target_sq, true);
                } else {
                    moves[*curr_move_index] = Move::new_with_flags(from, target_sq, Move::CAPTURE);
                    *curr_move_index += 1;
                }
            }

            // EN PASSANT
            if let Some(ep_sq) = board.en_passant_square {
                if target_sq == ep_sq {
                    moves[*curr_move_index] = Move::new_with_flags(from, target_sq, Move::EN_PASSANT);
                    *curr_move_index += 1;
                }
            }
        }

    }
}

fn push_promotions(moves: &mut [Move], index: &mut usize, from: u8, to: u8, capture: bool) {
    let flags = if capture {
        [Move::PC_QUEEN, Move::PC_ROOK, Move::PC_BISHOP, Move::PC_KNIGHT]
    } else {
        [Move::PR_QUEEN, Move::PR_ROOK, Move::PR_BISHOP, Move::PR_KNIGHT]
    };

    for &flag in &flags {
        moves[*index] = Move::new_with_flags(from, to, flag);
        *index += 1;
    }
}

fn generate_knight_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let mut knights = board.pieces[(us | Piece::KNIGHT) as usize];

    while knights != 0 {
        let from = knights.trailing_zeros() as u8;

        // 1. Get all potential moves for this square
        // 2. Filter out squares occupied by our own pieces (& !friendly_occ)
        let mut attacks = KNIGHT_MOVES[from as usize] & !board.pieces[(us | Piece::ALL) as usize];

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            let to_bit = 1u64 << to;

            // Determine if it's a capture for the Move flag
            let enemy_occ = board.pieces[((us ^ 8) | Piece::ALL) as usize];
            let flag = if (to_bit & enemy_occ) != 0 {
                Move::CAPTURE
            } else {
                Move::QUIET
            };

            moves[*curr_move_index] = Move::new_with_flags(from, to, flag);
            *curr_move_index += 1;

            attacks &= attacks - 1; // Clear processed bit
        }
        knights &= knights - 1; // Clear processed knight
    }
}

pub fn generate_bishop_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let bishops = board.pieces[(us | Piece::BISHOP) as usize];
    generate_sliding_moves(board, moves, curr_move_index, bishops, &BISHOP_SHIFTS);
}

pub fn generate_rook_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let rooks = board.pieces[(us | Piece::ROOK) as usize];
    generate_sliding_moves(board, moves, curr_move_index, rooks, &ROOK_SHIFTS);
}

pub fn generate_queen_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let queens = board.pieces[(us | Piece::QUEEN) as usize];

    // A Queen moves like a Bishop AND a Rook
    generate_sliding_moves(board, moves, curr_move_index, queens, &BISHOP_SHIFTS);
    generate_sliding_moves(board, moves, curr_move_index, queens, &ROOK_SHIFTS);
}

fn generate_sliding_moves(
    board: &Board,
    moves: &mut [Move],
    curr_move_index: &mut usize,
    mut piece_bb: u64, // The bitboard of pieces to generate moves for
    shifts: &[(i8, u64)], // The directions [(offset, mask)]
) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy_occ = board.pieces[((us ^ 8) | Piece::ALL) as usize];
    let friendly_occ = board.pieces[(us | Piece::ALL) as usize];

    while piece_bb != 0 {
        let from = piece_bb.trailing_zeros() as u8;
        let from_bit = 1u64 << from;

        for &(shift, mask) in shifts {
            let mut current_sq_mask = from_bit;
            loop {
                if shift > 0 {
                    current_sq_mask = (current_sq_mask << shift) & mask;
                } else {
                    current_sq_mask = (current_sq_mask >> shift.abs()) & mask;
                }

                if current_sq_mask == 0 { break; }

                let to = current_sq_mask.trailing_zeros() as u8;
                if (current_sq_mask & friendly_occ) != 0 { break; }

                if (current_sq_mask & enemy_occ) != 0 {
                    moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
                    *curr_move_index += 1;
                    break;
                }

                moves[*curr_move_index] = Move::new_with_flags(from, to, Move::QUIET);
                *curr_move_index += 1;
            }
        }
        piece_bb &= piece_bb - 1;
    }
}

fn generate_king_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {


}

pub const KNIGHT_MOVES: [u64; 64] = {
    let mut table = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let bit = 1u64 << sq;
        let mut moves = 0u64;
        let start_file = (sq % 8) as i8;

        // The 8 possible bit-shift offsets for a Knight
        // Positive = Up/Right, Negative = Down/Left
        let offsets: [i8; 8] = [17, 15, 10, 6, -17, -15, -10, -6];

        let mut i = 0;
        while i < 8 {
            let offset = offsets[i];
            let target_sq = sq as i8 + offset;

            // 1. Check if the square is even on the 0-63 board
            if target_sq >= 0 && target_sq < 64 {
                let target_file = (target_sq % 8) as i8;

                // A knight move can never change more than 2 files.
                // If it changed 7 files (e.g. from A to H), it wrapped.
                if (target_file - start_file).abs() <= 2 {
                    moves |= 1u64 << target_sq as u8;
                }
            }
            i += 1;
        }

        table[sq as usize] = moves;
        sq += 1;
    }
    table
};

// 0xFE = 11111110 (A-file is 0)
pub const NOT_A_FILE: u64 = 0xfefefefefefefefe;

// 0x7F = 01111111 (H-file is 0)
pub const NOT_H_FILE: u64 = 0x7f7f7f7f7f7f7f7f;

const BISHOP_SHIFTS: [(i8, u64); 4] = [(9, NOT_A_FILE), (7, NOT_H_FILE), (-7, NOT_A_FILE), (-9, NOT_H_FILE)];
const ROOK_SHIFTS: [(i8, u64); 4] = [(8, !0), (-8, !0), (1, NOT_A_FILE), (-1, NOT_H_FILE)];