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

// Move ordering bonus values
const TT_MOVE_BONUS: i32 = 100_000;
const PROMOTE_BONUS: i32 = 90_000;
const CAPTURE_BASE: i32 = 50_000;  // + MVV-LVA value
const KILLER_BONUS_0: i32 = 40_000;
const KILLER_BONUS_1: i32 = 39_000;
// History scores fill 0..~MAX_HISTORY

const DELTA_MARGIN: i32 = 200; // for delta pruning in qsearch

// ==========================================
// Search state passed through the tree
// ==========================================

struct SearchState {
    killers: [[Option<Move>; 2]; MAX_PLY as usize],
    history: [[i32; 64]; 16], // indexed by piece (us|pt) and to_sq
    nodes: u64,
}

impl SearchState {
    fn new() -> Self {
        SearchState {
            killers: [[None; 2]; MAX_PLY as usize],
            history: [[0; 64]; 16],
            nodes: 0,
        }
    }

    #[inline]
    fn store_killer(&mut self, ply: u32, m: Move) {
        let p = ply as usize;
        if self.killers[p][0] != Some(m) {
            self.killers[p][1] = self.killers[p][0];
            self.killers[p][0] = Some(m);
        }
    }

    #[inline]
    fn update_history(&mut self, board: &Board, m: Move, depth: u32) {
        let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
        let from_bit = 1u64 << m.from_sq();
        let piece_idx = find_piece_index(board, us, from_bit) as usize;
        let bonus = (depth * depth) as i32;
        let h = &mut self.history[piece_idx][m.to_sq() as usize];
        // Gravity: prevent unbounded growth
        *h += bonus - (*h * bonus.abs() / 16384);
    }

    #[inline]
    fn get_history(&self, board: &Board, m: Move) -> i32 {
        let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
        let from_bit = 1u64 << m.from_sq();
        let piece_idx = find_piece_index(board, us, from_bit) as usize;
        self.history[piece_idx][m.to_sq() as usize]
    }
}

#[inline]
fn find_piece_index(board: &Board, us: u8, from_bit: u64) -> u8 {
    for pt in [Piece::PAWN, Piece::KNIGHT, Piece::BISHOP, Piece::ROOK, Piece::QUEEN, Piece::KING] {
        if board.pieces[(us | pt) as usize] & from_bit != 0 {
            return us | pt;
        }
    }
    0
}

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
fn check_time_abort(ss: &mut SearchState, abort: &Arc<AtomicBool>) -> bool {
    ss.nodes += 1;
    (ss.nodes & 2047 == 0) && abort.load(Ordering::Relaxed)
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

// MVV-LVA: Most Valuable Victim – Least Valuable Attacker
#[inline]
fn mvv_lva(board: &Board, m: Move) -> i32 {
    let to_bit = 1u64 << m.to_sq();
    let from_bit = 1u64 << m.from_sq();
    let (us, them) = get_colors(board);

    let victim = if m.flags() == Move::EN_PASSANT { 100 }
    else if (board.pieces[(them | Piece::QUEEN) as usize] & to_bit) != 0 { 900 }
    else if (board.pieces[(them | Piece::ROOK) as usize] & to_bit) != 0 { 500 }
    else if (board.pieces[(them | Piece::BISHOP) as usize] & to_bit) != 0 { 330 }
    else if (board.pieces[(them | Piece::KNIGHT) as usize] & to_bit) != 0 { 320 }
    else { 100 }; // pawn

    let attacker = if (board.pieces[(us | Piece::PAWN) as usize] & from_bit) != 0 { 100 }
    else if (board.pieces[(us | Piece::KNIGHT) as usize] & from_bit) != 0 { 320 }
    else if (board.pieces[(us | Piece::BISHOP) as usize] & from_bit) != 0 { 330 }
    else if (board.pieces[(us | Piece::ROOK) as usize] & from_bit) != 0 { 500 }
    else if (board.pieces[(us | Piece::QUEEN) as usize] & from_bit) != 0 { 900 }
    else { 0 }; // king

    victim * 10 - attacker
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
                root_moves.push(RootMove { m, score: 0 });
            }
        }

        if root_moves.is_empty() { return None; }

        let mut ss = SearchState::new();
        let mut absolute_best_move = root_moves[0].m;
        let mut absolute_best_score = -INF;
        let mut depth_searched = 0;
        let mut history_stack = Vec::with_capacity(1024);

        for depth in 1..25u32 {
            root_moves.sort_by(|a, b| b.score.cmp(&a.score));

            // Aspiration window: narrow search around previous best score
            let (mut alpha, mut beta) = if depth >= 4 && absolute_best_score.abs() < MATE_BOUND {
                (absolute_best_score - 50, absolute_best_score + 50)
            } else {
                (-INF, INF)
            };

            let mut search_aborted;
            let mut current_depth_best_move;
            let mut current_depth_best_score;

            // Aspiration window loop: widen if search falls outside window
            loop {
                search_aborted = false;
                current_depth_best_move = root_moves[0].m;
                current_depth_best_score = -INF;

                for root_move in root_moves.iter_mut() {
                    let m = root_move.m;
                    let mut new_board = self.board;
                    let current_hash = self.board.zobrist_hash;

                    new_board.make_move(m);
                    let extension = if gives_check(&new_board, us, enemy) { 1 } else { 0 };

                    history_stack.push(current_hash);
                    let score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, 1, &abort, &mut ss, &mut self.tt, &mut history_stack);
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
                    if score > alpha { alpha = score; }
                }

                if search_aborted { break; }

                // If score fell outside aspiration window, widen and re-search
                if current_depth_best_score <= alpha - 50 || current_depth_best_score >= beta {
                    alpha = -INF;
                    beta = INF;
                    continue; // re-search with full window
                }
                break;
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

            println!("info depth {} score {} nodes {} pv {}",
                     depth_searched, format_score(absolute_best_score), ss.nodes, absolute_best_move.to_uci());

            if absolute_best_score > MATE_BOUND { break; }
        }

        println!("info depth {} score {} nodes {} pv {}",
                 depth_searched, format_score(absolute_best_score), ss.nodes, absolute_best_move.to_uci());

        Some(absolute_best_move)
    }
}

/// Public entry point for benchmarking the search.
/// Creates a fresh SearchState and TT, then runs alpha_beta at the given depth.
/// Returns (score, nodes_searched).
pub fn bench_search(board: &Board, depth: u32, abort: &Arc<AtomicBool>) -> (i32, u64) {
    let mut ss = SearchState::new();
    let mut tt = TranspositionTable::new(2);
    let mut history: Vec<u64> = Vec::with_capacity(1024);
    let score = alpha_beta(board, -INF, INF, depth, 0, abort, &mut ss, &mut tt, &mut history);
    (score, ss.nodes)
}

fn alpha_beta(
    board: &Board, mut alpha: i32, mut beta: i32, depth: u32, ply: u32,
    abort: &Arc<AtomicBool>, ss: &mut SearchState, tt: &mut TranspositionTable,
    history: &mut Vec<u64>,
) -> i32 {
    if check_time_abort(ss, abort) { return 0; }

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

    let in_check = board.is_in_check();
    let depth = if depth == 0 && in_check { 1 } else { depth };
    if depth == 0 { return quiescence_search(board, alpha, beta, abort, ss); }

    // --- Null Move Pruning ---
    if depth >= 3 && !in_check && board.has_non_pawn_material() {
        let mut null_board = *board;
        let z = zobrist();
        null_board.zobrist_hash ^= z.black_to_move;
        if let Some(ep_sq) = null_board.en_passant_square {
            null_board.zobrist_hash ^= z.en_passant[(ep_sq % 8) as usize];
        }
        null_board.white_to_move = !null_board.white_to_move;
        null_board.en_passant_square = None;

        let r = if depth >= 6 { 3 } else { 2 };
        let null_score = -alpha_beta(&null_board, -beta, -beta + 1, depth - 1 - r, ply + 1, abort, ss, tt, history);

        if null_score >= beta {
            return beta;
        }
    }

    let mut moves = [Move(0); MAX_MOVES];
    let mut scores = [0i32; MAX_MOVES];
    let count = generate_all_moves(board, &mut moves);

    // Score moves with TT move, killers, MVV-LVA, and history
    let killers = ss.killers[ply as usize];
    for i in 0..count {
        let m = moves[i];
        if Some(m) == tt_move {
            scores[i] = TT_MOVE_BONUS;
        } else if m.is_promotion() {
            scores[i] = PROMOTE_BONUS;
        } else if m.is_capture() {
            scores[i] = CAPTURE_BASE + mvv_lva(board, m);
        } else if Some(m) == killers[0] {
            scores[i] = KILLER_BONUS_0;
        } else if Some(m) == killers[1] {
            scores[i] = KILLER_BONUS_1;
        } else {
            scores[i] = ss.get_history(board, m);
        }
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
        if depth >= 3 && legal_moves > 3 && extension == 0 && !m.is_capture() && !m.is_promotion() && !in_check {
            reduction = 1;
            if legal_moves > 6 { reduction += 1; }
        }

        history.push(hash_key);
        let mut score;

        // PVS: First move gets full window, rest get null window first
        if legal_moves == 1 {
            score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, ss, tt, history);
        } else {
            // Scout search with null window
            score = -alpha_beta(&new_board, -alpha - 1, -alpha, depth - 1 + extension - reduction, ply + 1, abort, ss, tt, history);
            // Re-search if it improved alpha
            if score > alpha && (reduction > 0 || score < beta) {
                score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, ss, tt, history);
            }
        }
        history.pop();

        if abort.load(Ordering::Relaxed) { return 0; }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
            // Store killer and history for quiet moves that cause cutoffs
            if !m.is_capture() && !m.is_promotion() {
                ss.store_killer(ply, m);
                ss.update_history(board, m, depth);
            }
            tt.store(hash_key, depth, score_to_tt(best_score, ply), TTFlag::LowerBound, best_move);
            return best_score;
        }
        alpha = alpha.max(score);
    }

    // Terminal Node Handling
    if legal_moves == 0 {
        best_score = if in_check { -MATE_SCORE + (ply as i32) } else { 0 };
        tt.store(hash_key, depth, score_to_tt(best_score, ply), TTFlag::Exact, None);
        return best_score;
    }

    let tt_flag = if best_score <= original_alpha { TTFlag::UpperBound } else { TTFlag::Exact };
    tt.store(hash_key, depth, score_to_tt(best_score, ply), tt_flag, best_move);

    best_score
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

fn quiescence_search(board: &Board, mut alpha: i32, beta: i32, abort: &Arc<AtomicBool>, ss: &mut SearchState) -> i32 {
    if check_time_abort(ss, abort) { return 0; }

    let stand_pat = board.evaluate_board();
    if stand_pat >= beta { return stand_pat; }
    alpha = alpha.max(stand_pat);

    let mut captures = [Move(0); MAX_MOVES];
    let mut scores = [0i32; MAX_MOVES];
    let count = generate_captures(board, &mut captures);
    let (us, enemy) = get_colors(board);

    for i in 0..count {
        scores[i] = mvv_lva(board, captures[i]) + if captures[i].is_promotion() { 900 } else { 0 };
    }

    for i in 0..count {
        pick_next_best_move(&mut captures, &mut scores, i, count);
        let m = captures[i];

        // Delta pruning: skip captures that can't possibly raise alpha
        if !m.is_promotion() && stand_pat + scores[i] / 10 + DELTA_MARGIN < alpha {
            continue;
        }

        let mut new_board = *board;
        new_board.make_move(m);

        if !is_move_legal(&new_board, us, enemy) { continue; }

        let score = -quiescence_search(&new_board, -beta, -alpha, abort, ss);

        if abort.load(Ordering::Relaxed) { return 0; }

        if score >= beta { return beta; }
        alpha = alpha.max(score);
    }

    alpha
}