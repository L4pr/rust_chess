use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{Board, Move, generate_all_moves, is_square_attacked, Piece, ZobristKeys, OpeningBook};

pub struct Engine {
    board: Board,
    zobrist: ZobristKeys,
    tt: TranspositionTable,
    book: OpeningBook,
}

impl Engine {
    pub fn new() -> Self {
        let book = OpeningBook::load_from_file();

        Engine {
            board: Board::starting_position(),
            zobrist: ZobristKeys::new(),
            tt: TranspositionTable::new(64), // 64 MB transposition table
            book,
        }
    }

    pub fn clear_tt(&mut self) {
        self.tt = TranspositionTable::new(64); // Clear the TT by creating a new one
    }

    pub fn set_board(&mut self, new_board: Board) {
        self.board = new_board;
    }

    pub fn think(&mut self, abort: Arc<AtomicBool>) -> Option<Move> {
        let book_fen = self.board.to_book_fen();

        if let Some(book_move_str) = self.book.get_book_move(&book_fen) {
            println!("info string Playing book move!");
            if !abort.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            return self.board.parse_uci_to_move(&book_move_str);
        }

        let mut move_storage = [Move::new(0, 0); 218];
        let count = generate_all_moves(&self.board, &mut move_storage);

        let us = if self.board.white_to_move { Piece::WHITE } else { Piece::BLACK };
        let enemy = us ^ 8;

        let mut root_moves = Vec::new();

        for i in 0..count {
            let m = move_storage[i];
            let mut test_board = self.board;
            test_board.make_move(m);

            // Legality Check
            let king_bit = test_board.pieces[(us | Piece::KING) as usize];
            if king_bit != 0 {
                let king_sq = king_bit.trailing_zeros() as u8;
                if !is_square_attacked(&test_board, king_sq, enemy) {
                    root_moves.push(RootMove { m, score: score_move(m, &self.board, None) as f64 });
                }
            }
        }

        let mut nodes = 0;

        let mut absolute_best_move = root_moves[0].m;
        let mut absolute_best_score = f64::NEG_INFINITY;

        let mut depth_searched = 0;

        for depth in 1..25 { // Iterative Deepening

            // 2. SORT THE ENTIRE ARRAY based on the previous depth's scores!
            // This puts the best move 1st, second best 2nd, etc.
            root_moves.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            let mut alpha = f64::NEG_INFINITY;
            let beta = f64::INFINITY;

            // We need to track if we should break completely out of the depth
            let mut search_aborted = false;

            let mut current_depth_best_move = root_moves[0].m;
            let mut current_depth_best_score = f64::NEG_INFINITY;

            for i in 0..root_moves.len() {
                let m = root_moves[i].m;
                let mut new_board = self.board;
                new_board.make_move(m);

                // --- Legality Check ---
                let us = if self.board.white_to_move { Piece::WHITE } else { Piece::BLACK };
                let enemy = us ^ 8;
                let king_bit = new_board.pieces[(us | Piece::KING) as usize];
                if king_bit == 0 { continue; }
                let king_sq = king_bit.trailing_zeros() as u8;
                if is_square_attacked(&new_board, king_sq, enemy) { continue; }

                // 3. Search the move (Now passing TT and Zobrist!)
                let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1, 1, &abort, &mut nodes, &mut self.tt, &self.zobrist);

                if abort.load(Ordering::Relaxed) {
                    search_aborted = true;
                    break;
                }

                // 4. Update the exact score for THIS specific move so we can sort it next depth
                root_moves[i].score = score;

                if score > current_depth_best_score {
                    current_depth_best_score = score;
                    current_depth_best_move = m;
                }

                if score > alpha {
                    alpha = score;
                }
            }

            if search_aborted {
                // If this partial depth just found a move that is STRICTLY BETTER
                // than the best score from the last fully completed depth, it's safe to harvest it!
                if current_depth_best_score > absolute_best_score {
                    absolute_best_move = current_depth_best_move;
                }

                break;
            }

            // Because we sorted the array at the start, and updated scores,
            // the best move of this depth will be sorted to index 0 on the next loop!
            // We can just print the best one we found so far:

            // (We have to re-find the max here just for the print, because the scores
            // just updated and aren't sorted again until the next depth starts)
            absolute_best_move = current_depth_best_move;
            absolute_best_score = current_depth_best_score;

            depth_searched = depth;

            if absolute_best_score > 9000.0 {
                break;
            }
        }

        let score_to_print = if absolute_best_score > 9000.0 {
            format!("mate {}", ((10000.0 - absolute_best_score) / 2.0).ceil())
        } else if absolute_best_score < -9000.0 {
            format!("mate -{}", ((10000.0 + absolute_best_score) / 2.0).ceil())
        } else {
            format!("cp {}", absolute_best_score as i32)
        };

        println!("info depth {} score {} nodes {} pv {}",
                 depth_searched,
                 score_to_print,
                 nodes,
                 absolute_best_move.to_uci()
        );

        Some(absolute_best_move)
    }
}

pub fn alpha_beta(
    board: &Board,
    mut alpha: f64,
    mut beta: f64, // Needs to be mut now so we can update it from the TT
    depth: u32,
    ply: u32,
    abort: &Arc<AtomicBool>,
    nodes: &mut u64,
    tt: &mut TranspositionTable,
    zobrist: &ZobristKeys,
) -> f64 {
    // 1. Check for abort every few nodes
    *nodes += 1;
    if *nodes & 2047 == 0 && abort.load(Ordering::Relaxed) {
        return 0.0;
    }

    if ply >= 90 {
        println!("info string Reached ply {}, something is probably wrong. Aborting search.", ply);
        return board.evaluate_board();
    }

    // --- TT PROBE START ---
    let original_alpha = alpha;
    let hash_key = zobrist.hash(board);

    let mut tt_move = None;

    if let Some(entry) = tt.probe(hash_key) {
        tt_move = entry.best_move;

        if entry.depth >= depth {
            let mut tt_score = entry.score;
            if tt_score > 9000.0 {
                tt_score -= ply as f64;
            } else if tt_score < -9000.0 {
                tt_score += ply as f64;
            }

            match entry.flag {
                TTFlag::Exact => return tt_score,
                TTFlag::LowerBound => alpha = alpha.max(tt_score),
                TTFlag::UpperBound => beta = beta.min(tt_score),
            }
            if alpha >= beta {
                return tt_score;
            }
        }
    }
    // --- TT PROBE END ---

    // 2. Base Case: Leaf Node
    if depth == 0 {
        return board.evaluate_board();
    }

    // 3. Generate and Sort Moves
    let mut move_storage = [Move(0); 218];
    let count = generate_all_moves(board, &mut move_storage);

    let mut scores = [0i32; 218];
    for i in 0..count {
        scores[i] = score_move(move_storage[i], board, tt_move);
    }

    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;

    let mut legal_moves = 0;
    let mut best_score = f64::NEG_INFINITY;
    let mut best_move = None;

    for i in 0..count {
        let mut best_idx = i;
        for j in (i + 1)..count {
            if scores[j] > scores[best_idx] {
                best_idx = j;
            }
        }
        move_storage.swap(i, best_idx);
        scores.swap(i, best_idx);

        let m = move_storage[i];
        let mut new_board = *board;
        new_board.make_move(m);

        // Check legality (Did we leave our king in check?)
        let king_bit = new_board.pieces[(us | Piece::KING) as usize];
        let king_sq = king_bit.trailing_zeros() as u8;
        if is_square_attacked(&new_board, king_sq, enemy) {
            continue;
        }

        legal_moves += 1;

        let mut extension = 0;

        let enemy_king_bit = new_board.pieces[(enemy | Piece::KING) as usize];
        if enemy_king_bit != 0 {
            let enemy_king_sq = enemy_king_bit.trailing_zeros() as u8;
            if is_square_attacked(&new_board, enemy_king_sq, us) {
                extension = 1;
            } else if m.is_capture() {
                // extension = 1;
            }
        }

        // Recursively call with -beta and -alpha (Negamax style)
        // This flips the perspective for the other player
        let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, nodes, tt, zobrist);

        if abort.load(Ordering::Relaxed) {
            return 0.0;
        }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
            // STORE BEFORE RETURNING!
            let mut store_score = best_score;
            if store_score > 9000.0 {
                store_score += ply as f64;
            } else if store_score < -9000.0 {
                store_score -= ply as f64;
            }
            tt.store(hash_key, depth, store_score, TTFlag::LowerBound, best_move);
            return best_score;
        }
        if score > alpha {
            alpha = score; // This is our new best move
        }
    }

    // 4. Checkmate/Stalemate Detection
    if legal_moves == 0 {
        let king_bit = board.pieces[(us | Piece::KING) as usize];
        let king_sq = king_bit.trailing_zeros() as u8;
        best_score = if is_square_attacked(board, king_sq, enemy) {
            -10000.0 + (ply as f64) // Checkmate (prefer faster mates)
        } else {
            0.0 // Stalemate
        };

        let mut store_score = best_score;
        if store_score > 9000.0 {
            store_score += ply as f64;
        } else if store_score < -9000.0 {
            store_score -= ply as f64;
        }

        tt.store(hash_key, depth, store_score, TTFlag::Exact, None);
        return best_score;
    }

    // --- TT STORE START ---
    let tt_flag = if best_score <= original_alpha {
        TTFlag::UpperBound
    } else {
        TTFlag::Exact
    };

    tt.store(hash_key, depth, best_score, tt_flag, best_move);
    // --- TT STORE END ---

    alpha
}

fn score_move(m: Move, _board: &Board, tt_move: Option<Move>) -> i32 {
    // 1. Highest Priority: The move from the Transposition Table
    if Some(m) == tt_move {
        return 10_000;
    }

    let mut score = 0;

    // 2. High Priority: Captures
    // (Later you can upgrade this to MVV-LVA: Most Valuable Victim, Least Valuable Attacker) TODO
    if m.is_capture() {
        score += 1_000;
    }

    // 3. Medium Priority: Promotions
    if m.is_promotion() {
        score += 900;
    }

    // Quiet moves get a score of 0
    score
}

#[derive(Copy, Clone)]
struct RootMove {
    pub m: Move,
    pub score: f64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TTFlag {
    Exact,
    LowerBound, // We got a Beta cutoff (score >= beta)
    UpperBound, // We failed low (score <= alpha)
}

#[derive(Clone, Copy)]
pub struct TTEntry {
    pub key: u64,          // Zobrist hash of the position
    pub score: f64,        // Evaluation
    pub depth: u32,        // How deep we searched to get this score
    pub flag: TTFlag,      // Exact, Alpha, or Beta
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    mask: usize,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        // Calculate how many entries fit in the given Megabytes
        let entry_size = size_of::<TTEntry>();
        let num_entries = (size_mb * 1024 * 1024) / entry_size;

        // Find the next power of 2 for fast bitwise indexing
        let capacity = num_entries.next_power_of_two();

        Self {
            entries: vec![TTEntry {
                key: 0,
                score: 0.0,
                depth: 0,
                flag: TTFlag::Exact,
                best_move: None,
            }; capacity],
            mask: capacity - 1,
        }
    }

    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let index = (key as usize) & self.mask;
        let entry = self.entries[index];
        if entry.key == key {
            Some(entry)
        } else {
            None
        }
    }

    pub fn store(&mut self, key: u64, depth: u32, score: f64, flag: TTFlag, best_move: Option<Move>) {
        let index = (key as usize) & self.mask;
        let current_entry = self.entries[index];

        // Replacement Strategy: Replace if it's a completely different board (collision)
        // OR if the new search looked just as deep or deeper than the old one. TODO: look at this
        if current_entry.key != key || depth >= current_entry.depth {
            self.entries[index] = TTEntry { key, score, depth, flag, best_move };
        }
    }
}