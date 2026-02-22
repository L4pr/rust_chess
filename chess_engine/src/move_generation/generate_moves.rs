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


}

fn generate_bishop_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {


}

fn generate_rook_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {


}

fn generate_queen_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {


}

fn generate_king_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {


}