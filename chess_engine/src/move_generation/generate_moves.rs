use crate::board::board::Board;
use crate::board::move_struct::Move;
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