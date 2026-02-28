use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{Board, Move, generate_all_moves, is_square_attacked, Piece, OpeningBook, is_draw_by_repetition, generate_captures};
use crate::board::zobrist::zobrist;

// ==========================================
// Constants
// ==========================================
const MATE_SCORE: i32 = 100_000;
const MATE_BOUND: i32 = 90_000;
const INF: i32 = i32::MAX - 1;
const MAX_PLY: u32 = 90;
const MAX_MOVES: usize = 218;

// ==========================================
// Reusable Helper Functions
// ==========================================

#[inline]
fn get_colors(board: &Board) -> (u8, u8) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    (us, us ^ 8)
}

#[inline]
fn is_move_legal(board: &Board, us: u8, enemy: u8) -> bool {
    let king_bit = board.pieces[(us | Piece::KING) as usize];
    if king_bit == 0 { return false; }
    !is_square_attacked(board, king_bit.trailing_zeros() as u8, enemy)
}

#[inline]
fn gives_check(board: &Board, us: u8, enemy: u8) -> bool {
    let enemy_king_bit = board.pieces[(enemy | Piece::KING) as usize];
    enemy_king_bit != 0 && is_square_attacked(board, enemy_king_bit.trailing_zeros() as u8, us)
}

#[inline]
fn check_time_abort(nodes: &mut u64, abort: &Arc<AtomicBool>) -> bool {
    *nodes += 1;
    (*nodes & 2047 == 0) && abort.load(Ordering::Relaxed)
}

#[inline]
fn score_from_tt(mut score: i32, ply: u32) -> i32 {
    if score > MATE_BOUND { score -= ply as i32; }
    else if score < -MATE_BOUND { score += ply as i32; }
    score
}

#[inline]
fn score_to_tt(mut score: i32, ply: u32) -> i32 {
    if score > MATE_BOUND { score += ply as i32; }
    else if score < -MATE_BOUND { score -= ply as i32; }
    score
}

#[inline]
fn format_score(score: i32) -> String {
    if score > MATE_BOUND {
        format!("mate {}", ((MATE_SCORE - score) + 1) / 2)
    } else if score < -MATE_BOUND {
        format!("mate -{}", ((MATE_SCORE + score) + 1) / 2)
    } else {
        format!("cp {}", score)
    }
}

#[inline]
fn pick_next_best_move(moves: &mut [Move], scores: &mut [i32], start_idx: usize, count: usize) {
    let mut best_idx = start_idx;
    for j in (start_idx + 1)..count {
        if scores[j] > scores[best_idx] {
            best_idx = j;
        }
    }
    moves.swap(start_idx, best_idx);
    scores.swap(start_idx, best_idx);
}

// ==========================================
// Engine & Search Implementation
// ==========================================

pub struct Engine {
    board: Board,
    tt: TranspositionTable,
    book: OpeningBook,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            board: Board::starting_position(),
            tt: TranspositionTable::new(64),
            book: OpeningBook::load_from_file(),
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

        let mut move_storage = [Move::new(0, 0); MAX_MOVES];
        let count = generate_all_moves(&self.board, &mut move_storage);

        let (us, enemy) = get_colors(&self.board);
        let mut root_moves = Vec::with_capacity(count);

        for i in 0..count {
            let m = move_storage[i];
            let mut test_board = self.board;
            test_board.make_move(m);

            if is_move_legal(&test_board, us, enemy) {
                root_moves.push(RootMove { m, score: score_move(m, None) });
            }
        }

        if root_moves.is_empty() { return None; }

        let mut nodes = 0;
        let mut absolute_best_move = root_moves[0].m;
        let mut absolute_best_score = -INF;
        let mut depth_searched = 0;
        let mut history_stack = Vec::with_capacity(1024);

        for depth in 1..25 {
            root_moves.sort_by(|a, b| b.score.cmp(&a.score));

            let mut alpha = -INF;
            let beta = INF;
            let mut search_aborted = false;

            let mut current_depth_best_move = root_moves[0].m;
            let mut current_depth_best_score = -INF;

            for root_move in root_moves.iter_mut() {
                let m = root_move.m;
                let mut new_board = self.board;
                let current_hash = self.board.zobrist_hash;

                new_board.make_move(m);
                let extension = if gives_check(&new_board, us, enemy) { 1 } else { 0 };

                history_stack.push(current_hash);
                let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, 1, &abort, &mut nodes, &mut self.tt, &mut history_stack);
                history_stack.pop();

                if abort.load(Ordering::Relaxed) {
                    search_aborted = true;
                    break;
                }

                root_move.score = score;

                if score > current_depth_best_score {
                    current_depth_best_score = score;
                    current_depth_best_move = m;
                }
                alpha = alpha.max(score);
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

            if absolute_best_score > MATE_BOUND { break; }
        }

        println!("info depth {} score {} nodes {} pv {}",
                 depth_searched, format_score(absolute_best_score), nodes, absolute_best_move.to_uci());

        Some(absolute_best_move)
    }
}

pub fn alpha_beta(
    board: &Board, mut alpha: i32, mut beta: i32, depth: u32, ply: u32,
    abort: &Arc<AtomicBool>, nodes: &mut u64, tt: &mut TranspositionTable,
    history: &mut Vec<u64>,
) -> i32 {
    if check_time_abort(nodes, abort) { return 0; }

    if ply >= MAX_PLY {
        return board.evaluate_board();
    }

    let hash_key = board.zobrist_hash;

    if ply > 0 && (board.halfmove_clock >= 100 || is_draw_by_repetition(board.halfmove_clock, history, hash_key)) {
        return 0;
    }

    let original_alpha = alpha;
    let mut tt_move = None;

    if let Some(entry) = tt.probe(hash_key) {
        tt_move = entry.best_move;
        if entry.depth >= depth {
            let tt_score = score_from_tt(entry.score, ply);
            match entry.flag {
                TTFlag::Exact => return tt_score,
                TTFlag::LowerBound => alpha = alpha.max(tt_score),
                TTFlag::UpperBound => beta = beta.min(tt_score),
            }
            if alpha >= beta { return tt_score; }
        }
    }

    let depth = if depth == 0 && board.is_in_check() { 1 } else { depth };
    if depth == 0 { return quiescence_search(board, alpha, beta, abort, nodes); }

    // --- Null Move Pruning ---
    if depth >= 3 && !board.is_in_check() && board.has_non_pawn_material() {
        let mut null_board = *board;
        let z = zobrist();
        // Flip side to move
        null_board.zobrist_hash ^= z.black_to_move;
        // Remove old en passant from hash
        if let Some(ep_sq) = null_board.en_passant_square {
            null_board.zobrist_hash ^= z.en_passant[(ep_sq % 8) as usize];
        }
        null_board.white_to_move = !null_board.white_to_move;
        null_board.en_passant_square = None;

        let r = if depth >= 6 { 3 } else { 2 };
        let null_score = -alpha_beta(&null_board, -beta, -beta + 1, depth - 1 - r, ply + 1, abort, nodes, tt, history);

        if null_score >= beta {
            return beta;
        }
    }

    let mut moves = [Move(0); MAX_MOVES];
    let mut scores = [0i32; MAX_MOVES];
    let count = generate_all_moves(board, &mut moves);

    for i in 0..count {
        scores[i] = score_move(moves[i], tt_move);
    }

    let (us, enemy) = get_colors(board);
    let mut legal_moves = 0;
    let mut best_score = -INF;
    let mut best_move = None;

    for i in 0..count {
        pick_next_best_move(&mut moves, &mut scores, i, count);
        let m = moves[i];

        let mut new_board = *board;
        new_board.make_move(m);

        if !is_move_legal(&new_board, us, enemy) { continue; }

        legal_moves += 1;
        let extension = if gives_check(&new_board, us, enemy) { 1 } else { 0 };

        // --- Late Move Reductions ---
        let mut reduction = 0;
        if depth >= 3 && legal_moves > 3 && extension == 0 && !m.is_capture() && !m.is_promotion() {
            reduction = 1;
        }

        history.push(hash_key);
        let mut score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension - reduction, ply + 1, abort, nodes, tt, history);

        // Re-search at full depth if reduced search found something good
        if reduction > 0 && score > alpha {
            score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, nodes, tt, history);
        }
        history.pop();

        if abort.load(Ordering::Relaxed) { return 0; }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
            tt.store(hash_key, depth, score_to_tt(best_score, ply), TTFlag::LowerBound, best_move);
            return best_score;
        }
        alpha = alpha.max(score);
    }

    // Terminal Node Handling
    if legal_moves == 0 {
        best_score = if gives_check(board, enemy, us) { -MATE_SCORE + (ply as i32) } else { 0 };
        tt.store(hash_key, depth, score_to_tt(best_score, ply), TTFlag::Exact, None);
        return best_score;
    }

    let tt_flag = if best_score <= original_alpha { TTFlag::UpperBound } else { TTFlag::Exact };
    tt.store(hash_key, depth, score_to_tt(best_score, ply), tt_flag, best_move);

    best_score
}

fn score_move(m: Move, tt_move: Option<Move>) -> i32 {
    if Some(m) == tt_move { return 10_000; }
    let mut score = 0;
    if m.is_capture() { score += 1_000; }
    if m.is_promotion() { score += 900; }
    score
}

#[derive(Copy, Clone)]
struct RootMove { m: Move, score: i32 }

#[derive(Clone, Copy, PartialEq)]
pub enum TTFlag { Exact, LowerBound, UpperBound }

#[derive(Clone, Copy)]
pub struct TTEntry {
    pub key: u64,
    pub score: i32,
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
            entries: vec![TTEntry { key: 0, score: 0, depth: 0, flag: TTFlag::Exact, best_move: None }; capacity],
            mask: capacity - 1,
        }
    }

    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let entry = self.entries[(key as usize) & self.mask];
        if entry.key == key { Some(entry) } else { None }
    }

    pub fn store(&mut self, key: u64, depth: u32, score: i32, flag: TTFlag, best_move: Option<Move>) {
        let index = (key as usize) & self.mask;
        let current = self.entries[index];

        if current.key != key || depth >= current.depth {
            self.entries[index] = TTEntry { key, score, depth, flag, best_move };
        }
    }
}

fn score_capture_qs(board: &Board, m: Move) -> i32 {
    let to_bit = 1u64 << m.to_sq();
    let from_bit = 1u64 << m.from_sq();
    let (us, them) = get_colors(board);

    let victim_val = if m.flags() == Move::EN_PASSANT { 100 }
    else if (board.pieces[(them | Piece::QUEEN) as usize] & to_bit) != 0 { 900 }
    else if (board.pieces[(them | Piece::ROOK) as usize] & to_bit) != 0 { 500 }
    else if (board.pieces[(them | Piece::BISHOP) as usize] & to_bit) != 0 { 330 }
    else if (board.pieces[(them | Piece::KNIGHT) as usize] & to_bit) != 0 { 320 }
    else { 100 };

    let attacker_val = if (board.pieces[(us | Piece::PAWN) as usize] & from_bit) != 0 { 100 }
    else if (board.pieces[(us | Piece::KNIGHT) as usize] & from_bit) != 0 { 320 }
    else if (board.pieces[(us | Piece::BISHOP) as usize] & from_bit) != 0 { 330 }
    else if (board.pieces[(us | Piece::ROOK) as usize] & from_bit) != 0 { 500 }
    else if (board.pieces[(us | Piece::QUEEN) as usize] & from_bit) != 0 { 900 }
    else { 20000 };

    (victim_val * 10) - attacker_val + if m.is_promotion() { 900 } else { 0 }
}

pub fn quiescence_search(board: &Board, mut alpha: i32, beta: i32, abort: &Arc<AtomicBool>, nodes: &mut u64) -> i32 {
    if check_time_abort(nodes, abort) { return 0; }

    let stand_pat = board.evaluate_board();
    if stand_pat >= beta { return stand_pat; }
    alpha = alpha.max(stand_pat);

    let mut captures = [Move(0); MAX_MOVES];
    let mut scores = [0i32; MAX_MOVES];
    let count = generate_captures(board, &mut captures);
    let (us, enemy) = get_colors(board);

    for i in 0..count {
        scores[i] = score_capture_qs(board, captures[i]);
    }

    for i in 0..count {
        pick_next_best_move(&mut captures, &mut scores, i, count);
        let m = captures[i];

        let mut new_board = *board;
        new_board.make_move(m);

        if !is_move_legal(&new_board, us, enemy) { continue; }

        let score = -quiescence_search(&new_board, -beta, -alpha, abort, nodes);

        if abort.load(Ordering::Relaxed) { return 0; }

        if score >= beta { return beta; }
        alpha = alpha.max(score);
    }

    alpha
}