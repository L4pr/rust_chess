// src/move_generation/tests.rs
use crate::{Board, Move, generate_all_moves, is_square_attacked, Piece};

/// Perft (Performance Test) function to count all legal moves at a given depth
/// Used for move generation testing and benchmarking
pub fn perft(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut nodes = 0;
    let mut move_storage = [Move(0); 218];
    let count = generate_all_moves(board, &mut move_storage);

    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;

    for i in 0..count {
        let m = move_storage[i];
        let mut new_board = *board; // Copy the board

        new_board.make_move(m);    // Update the copy

        let king_sq = new_board.pieces[(us | Piece::KING) as usize].trailing_zeros() as u8;

        if !is_square_attacked(&new_board, king_sq, enemy) {
            nodes += perft(&mut new_board, depth - 1);
        }
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_moves_report() {
        let board = Board::starting_position();
        let expected = [
            (1, 20),
            (2, 400),
            (3, 8902),
            (4, 197281),
            (5, 4865609),
            (6, 119060324),
        ];

        let mut failures = Vec::new();

        for (depth, target) in expected {
            let result = perft(&mut board.clone(), depth);
            println!("Depth {}: Result {}, Expected {}", depth, result, target);
            if result != target {
                failures.push((depth, result, target));
            }
        }

        if !failures.is_empty() {
            for (d, res, exp) in failures {
                eprintln!("FAILED: Depth {} got {} (expected {})", d, res, exp);
            }
            panic!("Perft tests failed!");
        }
    }

    #[test]
    fn test_fen() {
        let initial_fen = "rnbqkbnr/pppp1ppp/4p3/4P3/8/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1";
        let board = Board::from_fen(initial_fen);

        assert_eq!(initial_fen, board.get_fen());
    }
}