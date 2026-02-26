use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{Board, Move, generate_all_moves, is_square_attacked, Piece, ZobristKeys, OpeningBook, is_draw_by_repetition, generate_captures};

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
            tt: TranspositionTable::new(64),
            book,
        }
    }

    pub fn clear_tt(&mut self) {
        self.tt = TranspositionTable::new(64);
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

        let mut history_stack: Vec<u64> = Vec::with_capacity(1024);

        for depth in 1..25 {
            root_moves.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            let mut alpha = f64::NEG_INFINITY;
            let beta = f64::INFINITY;
            let mut search_aborted = false;

            let mut current_depth_best_move = root_moves[0].m;
            let mut current_depth_best_score = f64::NEG_INFINITY;

            for i in 0..root_moves.len() {
                let m = root_moves[i].m;
                let mut new_board = self.board;

                let current_hash = self.zobrist.hash(&self.board);

                new_board.make_move(m);
                history_stack.push(current_hash);

                let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1, 1, &abort, &mut nodes, &mut self.tt, &self.zobrist, &mut history_stack);

                history_stack.pop();

                if abort.load(Ordering::Relaxed) {
                    search_aborted = true;
                    break;
                }

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
                if current_depth_best_score > absolute_best_score {
                    absolute_best_move = current_depth_best_move;
                }
                break;
            }

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
    mut beta: f64,
    depth: u32,
    ply: u32,
    abort: &Arc<AtomicBool>,
    nodes: &mut u64,
    tt: &mut TranspositionTable,
    zobrist: &ZobristKeys,
    history: &mut Vec<u64>,
) -> f64 {
    *nodes += 1;
    if *nodes & 2047 == 0 && abort.load(Ordering::Relaxed) {
        return 0.0;
    }

    if ply >= 90 {
        println!("info string Reached ply {}, something is probably wrong. Aborting search.", ply);
        return board.evaluate_board();
    }

    let hash_key = zobrist.hash(board);

    if ply > 0 && (board.halfmove_clock >= 100 || is_draw_by_repetition(board.halfmove_clock, history, hash_key)) {
        return 0.0;
    }

    let original_alpha = alpha;
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

    if depth == 0 {
        // return board.evaluate_board();
        return quiescence_search(board, alpha, beta, abort, nodes);
    }

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
            }
        }

        history.push(hash_key);

        let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, nodes, tt, zobrist, history);

        history.pop();

        if abort.load(Ordering::Relaxed) {
            return 0.0;
        }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
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
            alpha = score;
        }
    }

    if legal_moves == 0 {
        let king_bit = board.pieces[(us | Piece::KING) as usize];
        let king_sq = king_bit.trailing_zeros() as u8;
        best_score = if is_square_attacked(board, king_sq, enemy) {
            -10000.0 + (ply as f64)
        } else {
            0.0
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

    let tt_flag = if best_score <= original_alpha {
        TTFlag::UpperBound
    } else {
        TTFlag::Exact
    };

    tt.store(hash_key, depth, best_score, tt_flag, best_move);

    alpha
}

fn score_move(m: Move, _board: &Board, tt_move: Option<Move>) -> i32 {
    if Some(m) == tt_move {
        return 10_000;
    }

    let mut score = 0;

    if m.is_capture() {
        score += 1_000;
    }

    if m.is_promotion() {
        score += 900;
    }

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
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub score: f64,
    pub depth: u32,
    pub flag: TTFlag,
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    entries: Vec<TTEntry>,
    mask: usize,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let entry_size = std::mem::size_of::<TTEntry>();
        let num_entries = (size_mb * 1024 * 1024) / entry_size;
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

        if current_entry.key != key || depth >= current_entry.depth {
            self.entries[index] = TTEntry { key, score, depth, flag, best_move };
        }
    }
}

fn score_capture_qs(board: &Board, m: Move) -> i32 {
    let to_bit = 1u64 << m.to_sq();
    let from_bit = 1u64 << m.from_sq();

    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let them = us ^ 8;

    // 1. Find Victim Value (Unrolled for speed)
    let victim_val = if m.flags() == Move::EN_PASSANT { 100 }
    else if (board.pieces[(them | Piece::QUEEN) as usize] & to_bit) != 0 { 900 }
    else if (board.pieces[(them | Piece::ROOK) as usize] & to_bit) != 0 { 500 }
    else if (board.pieces[(them | Piece::BISHOP) as usize] & to_bit) != 0 { 330 }
    else if (board.pieces[(them | Piece::KNIGHT) as usize] & to_bit) != 0 { 320 }
    else { 100 }; // Defaults to Pawn

    // 2. Find Attacker Value (Unrolled for speed)
    let attacker_val = if (board.pieces[(us | Piece::PAWN) as usize] & from_bit) != 0 { 100 }
    else if (board.pieces[(us | Piece::KNIGHT) as usize] & from_bit) != 0 { 320 }
    else if (board.pieces[(us | Piece::BISHOP) as usize] & from_bit) != 0 { 330 }
    else if (board.pieces[(us | Piece::ROOK) as usize] & from_bit) != 0 { 500 }
    else if (board.pieces[(us | Piece::QUEEN) as usize] & from_bit) != 0 { 900 }
    else { 20000 }; // King

    let promo_bonus = if m.is_promotion() { 900 } else { 0 };

    (victim_val * 10) - attacker_val + promo_bonus
}

pub fn quiescence_search(
    board: &Board,
    mut alpha: f64,
    beta: f64,
    abort: &Arc<AtomicBool>,
    nodes: &mut u64,
) -> f64 {
    // return board.evaluate_board();


    // 1. Time management check
    *nodes += 1;
    if *nodes & 2047 == 0 && abort.load(Ordering::Relaxed) {
        return 0.0;
    }

    let stand_pat = board.evaluate_board();
    if stand_pat >= beta {
        return beta;
    }
    if alpha < stand_pat {
        alpha = stand_pat;
    }

    let mut captures = [Move(0); 218];
    let count = generate_captures(board, &mut captures);

    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;

    let mut scores = [0i32; 32];

    for i in 0..count {
        scores[i] = score_capture_qs(board, captures[i]);
    }

    for i in 0..count {
        let mut best_idx = i;
        for j in (i + 1)..count {
            if scores[j] > scores[best_idx] {
                best_idx = j;
            }
        }
        captures.swap(i, best_idx);
        scores.swap(i, best_idx);

        let m = captures[i];

        let mut new_board = *board;
        new_board.make_move(m);

        let king_bit = new_board.pieces[(us | Piece::KING) as usize];
        if king_bit == 0 { continue; }
        let king_sq = king_bit.trailing_zeros() as u8;
        if is_square_attacked(&new_board, king_sq, enemy) {
            continue;
        }

        let score = -quiescence_search(&new_board, -beta, -alpha, abort, nodes);

        if abort.load(Ordering::Relaxed) {
            return 0.0;
        }

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}