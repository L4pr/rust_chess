use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{Board, Move, generate_all_moves, is_square_attacked, Piece, ZobristKeys};

pub struct Engine {
    board: Board,
    zobrist: ZobristKeys,
    tt: TranspositionTable,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            board: Board::starting_position(),
            zobrist: ZobristKeys::new(),
            tt: TranspositionTable::new(64), // 64 MB transposition table
        }
    }

    pub fn set_board(&mut self, new_board: Board) {
        self.board = new_board;
    }

    pub fn think(&mut self, abort: Arc<AtomicBool>) -> Option<Move> {
        let mut move_storage = [Move::new(0, 0); 256];
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
                    root_moves.push(RootMove { m, score: f64::NEG_INFINITY });
                }
            }
        }

        let mut nodes = 0;

        for depth in 1..25 { // Iterative Deepening

            // 2. SORT THE ENTIRE ARRAY based on the previous depth's scores!
            // This puts the best move 1st, second best 2nd, etc.
            root_moves.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            let mut alpha = f64::NEG_INFINITY;
            let beta = f64::INFINITY;

            // We need to track if we should break completely out of the depth
            let mut search_aborted = false;

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
                let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1, &abort, &mut nodes, &mut self.tt, &self.zobrist);

                if abort.load(Ordering::Relaxed) {
                    search_aborted = true;
                    break;
                }

                // 4. Update the exact score for THIS specific move so we can sort it next depth
                root_moves[i].score = score;

                if score > alpha {
                    alpha = score;
                }
            }

            if search_aborted { break; }

            // Because we sorted the array at the start, and updated scores,
            // the best move of this depth will be sorted to index 0 on the next loop!
            // We can just print the best one we found so far:

            // (We have to re-find the max here just for the print, because the scores
            // just updated and aren't sorted again until the next depth starts)
            if let Some(best_root_move) = root_moves.iter().max_by(|a, b| a.score.partial_cmp(&b.score).unwrap()) {
                println!("info depth {} score cp {} nodes {} pv {}",
                         depth,
                         best_root_move.score as i32,
                         nodes,
                         best_root_move.m.to_uci()
                );
            }
        }

        // When time runs out, the best move from the last fully completed depth
        // is sitting at index 0 (because it was sorted at the top of the loop!)
        Some(root_moves.iter().max_by(|a, b| a.score.partial_cmp(&b.score).unwrap()).unwrap().m)
    }
}

fn alpha_beta(
    board: &Board,
    mut alpha: f64,
    mut beta: f64, // Needs to be mut now so we can update it from the TT
    depth: u32,
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

    // --- TT PROBE START ---
    let original_alpha = alpha;
    let hash_key = zobrist.hash(board);

    if let Some(entry) = tt.probe(hash_key) {
        if entry.depth >= depth {
            match entry.flag {
                TTFlag::Exact => return entry.score,
                TTFlag::LowerBound => alpha = alpha.max(entry.score),
                TTFlag::UpperBound => beta = beta.min(entry.score),
            }
            if alpha >= beta {
                return entry.score;
            }
        }
    }
    // --- TT PROBE END ---

    // 2. Base Case: Leaf Node
    if depth == 0 {
        return quiescence_search(board, alpha, beta, abort, nodes, tt, zobrist);
    }

    // 3. Generate and Sort Moves
    let mut move_storage = [Move(0); 218];
    let count = generate_all_moves(board, &mut move_storage);

    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;

    let mut legal_moves = 0;
    let mut best_score = f64::NEG_INFINITY;
    let mut best_move = None;

    for i in 0..count {
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

        // Recursively call with -beta and -alpha (Negamax style)
        // This flips the perspective for the other player
        let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1, abort, nodes, tt, zobrist);

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
            // STORE BEFORE RETURNING!
            tt.store(hash_key, depth, best_score, TTFlag::LowerBound, best_move);
            return beta; // Beta Cutoff: This branch is too good for the opponent to allow
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
            -10000.0 - (depth as f64) // Checkmate (prefer faster mates)
        } else {
            0.0 // Stalemate
        };

        tt.store(hash_key, depth, best_score, TTFlag::Exact, None);
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

pub fn quiescence_search(
    board: &Board,
    mut alpha: f64,
    mut beta: f64, // Mutated by TT probe
    abort: &Arc<AtomicBool>,
    nodes: &mut u64,
    tt: &mut TranspositionTable, // Added
    zobrist: &ZobristKeys,       // Added
) -> f64 {
    // 1. Abort check
    *nodes += 1;
    if *nodes & 1023 == 0 && abort.load(Ordering::Relaxed) {
        return 0.0;
    }

    // --- TT PROBE START ---
    let original_alpha = alpha;
    let hash_key = zobrist.hash(board);

    // In QS, we don't need to check if entry.depth >= depth,
    // because any entry in the TT is at least depth 0 (QS level).
    if let Some(entry) = tt.probe(hash_key) {
        match entry.flag {
            TTFlag::Exact => return entry.score,
            TTFlag::LowerBound => alpha = alpha.max(entry.score),
            TTFlag::UpperBound => beta = beta.min(entry.score),
        }
        if alpha >= beta {
            return entry.score;
        }
    }
    // --- TT PROBE END ---

    // 2. The "Stand Pat" Score
    let stand_pat = board.evaluate_board();

    if stand_pat >= beta {
        // STORE BEFORE RETURNING!
        tt.store(hash_key, 0, stand_pat, TTFlag::LowerBound, None);
        return beta;
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // We track the best score for the TT.
    // In QS, our "worst case" is just standing pat!
    let mut best_score = stand_pat;

    // 3. Generate ONLY Captures
    let mut move_storage = [Move::new(0, 0); 256];
    let count = generate_all_moves(board, &mut move_storage);

    let mut captures = Vec::with_capacity(count);
    for i in 0..count {
        let m = move_storage[i];
        if m.is_capture() {
            captures.push(m);
        }
    }

    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;

    // 4. Search the Captures
    for m in captures {
        let mut new_board = *board;
        new_board.make_move(m);

        let king_bit = new_board.pieces[(us | Piece::KING) as usize];
        if king_bit == 0 { continue; }
        let king_sq = king_bit.trailing_zeros() as u8;
        if is_square_attacked(&new_board, king_sq, enemy) {
            continue;
        }

        // Pass TT and Zobrist recursively!
        let score = -quiescence_search(&new_board, -beta, -alpha, abort, nodes, tt, zobrist);

        if score > best_score {
            best_score = score;
        }

        if score >= beta {
            // STORE BEFORE RETURNING!
            tt.store(hash_key, 0, beta, TTFlag::LowerBound, Some(m));
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    // --- TT STORE START ---
    let tt_flag = if best_score <= original_alpha {
        TTFlag::UpperBound
    } else {
        TTFlag::Exact
    };

    tt.store(hash_key, 0, best_score, tt_flag, None);
    // --- TT STORE END ---

    alpha
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