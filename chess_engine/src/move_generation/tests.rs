// src/move_generation/tests.rs
#[cfg(test)]
mod tests {
    use crate::{Board, Move, generate_all_moves};

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

    pub fn perft(board: &mut Board, depth: usize) -> u64 {
        let mut nodes = 0;

        let mut move_storage = [Move(0); 218];
        let count = generate_all_moves(board, &mut move_storage);
        let moves = &move_storage[..count];

        // 2. Base Case: If depth is 1, return the count of moves
        if depth == 1 {
            return count as u64;
        }

        // 3. Recursive Step: Make each move and call perft again
        for &m in moves {
            let mut new_board = *board; // Copy the board
            new_board.make_move(m);    // Update the copy
            nodes += perft(&mut new_board, depth - 1);
        }

        nodes
    }

    #[test]
    fn test_fen() {
        let initial_fen = "rnbqkbnr/pppp1ppp/4p3/4P3/8/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1";
        let board = Board::from_fen(initial_fen);

        assert_eq!(initial_fen, board.get_fen());
    }
}