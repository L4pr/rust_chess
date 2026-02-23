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

    /// Helper function to run perft tests cleanly
    fn run_perft_test(fen: &str, expected: &[(usize, u64)]) {
        let board = Board::from_fen(fen);
        for &(depth, target) in expected {
            // We clone the board so the original isn't mutated during tests
            let result = perft(&mut board.clone(), depth);
            assert_eq!(
                result, target,
                "PERFT FAILED! FEN: {} | Depth: {} | Expected: {} | Got: {}",
                fen, depth, target, result
            );
        }
    }

    #[test]
    fn test_perft_start_position() {
        // Position 1: Standard Start
        run_perft_test(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &[
                (1, 20),
                (2, 400),
                (3, 8902),
                (4, 197281),
                (5, 4865609),
                // (6, 119060324), // Uncomment if you want to wait!
            ],
        );
    }

    #[test]
    fn test_perft_kiwipete() {
        // Position 2: "Kiwipete"
        // Tests complex castling rights, pinned pieces, and late-game pawn pushes.
        run_perft_test(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &[
                (1, 48),
                (2, 2039),
                (3, 97862),
                (4, 4085603),
                // (5, 193690690),
            ],
        );
    }

    #[test]
    fn test_perft_position_3() {
        // Position 3: Silver Suite
        // Brutal test for en passant, discovered checks, and pawn captures.
        run_perft_test(
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            &[
                (1, 14),
                (2, 191),
                (3, 2812),
                (4, 43238),
                (5, 674624),
                (6, 11030083),
            ],
        );
    }

    #[test]
    fn test_perft_position_4() {
        // Position 4: Asymmetric
        // Excellent for finding bugs in White vs Black specific logic (mirrored boards).
        run_perft_test(
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            &[
                (1, 6),
                (2, 264),
                (3, 9467),
                (4, 422333),
                (5, 15833292),
            ],
        );
    }

    #[test]
    fn test_perft_position_5() {
        // Position 5: Promotion City
        // Tests edge cases where pawns promote while giving check or capturing.
        run_perft_test(
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            &[
                (1, 44),
                (2, 1486),
                (3, 62379),
                (4, 2103487),
                // (5, 89941194),
            ],
        );
    }
}