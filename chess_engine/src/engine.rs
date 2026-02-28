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

const DELTA_MARGIN: i32 = 200; // for delta pruning in qsearch

// Pruning margins
const RFP_MARGIN: i32 = 80;        // Reverse futility pruning margin per depth
const FUTILITY_MARGIN: i32 = 150;   // Futility pruning margin per depth

// Pre-computed LMR reduction table: LMR_TABLE[depth][move_count]
// Reduction = floor(0.75 + ln(depth) * ln(move_count) / 2.25)
static LMR_TABLE: [[u32; 64]; 64] = {
    let mut table = [[0u32; 64]; 64];
    let mut d = 1usize;
    while d < 64 {
        let mut m = 1usize;
        while m < 64 {
            // Integer approximation of ln using a lookup approach
            // We pre-compute at compile time with integer math
            // ln(1)=0, ln(2)≈0.69, ln(3)≈1.10, ln(4)≈1.39, ...
            // We'll store 100x values to avoid floats
            let ln_d_x100 = ln_approx_x100(d);
            let ln_m_x100 = ln_approx_x100(m);
            let val = 75 + (ln_d_x100 * ln_m_x100) / 225;
            table[d][m] = (val / 100) as u32;
            m += 1;
        }
        d += 1;
    }
    table
};

const fn ln_approx_x100(n: usize) -> u32 {
    // Approximate ln(n) * 100 using integer math
    // ln(1)=0, ln(2)=69, ln(3)=110, ln(4)=139, ln(5)=161, ln(6)=179,
    // ln(7)=195, ln(8)=208, ln(16)=277, ln(32)=347, ln(64)=416
    match n {
        0 | 1 => 0,
        2 => 69,
        3 => 110,
        4 => 139,
        5 => 161,
        6 => 179,
        7 => 195,
        8 => 208,
        9 => 220,
        10 => 230,
        11 => 240,
        12 => 249,
        13 => 256,
        14 => 264,
        15 => 271,
        16 => 277,
        _ => {
            // For larger values: ln(n) ≈ ln(n/2) + ln(2) = ln(n/2) + 69
            // Recursive halving
            let half = ln_approx_x100(n / 2);
            half + 69
        }
    }
}

// ==========================================
// Search state passed through the tree
// ==========================================

struct SearchState {
    killers: [[Option<Move>; 2]; MAX_PLY as usize],
    history: [[i32; 64]; 16], // indexed by piece (us|pt) and to_sq
    nodes: u64,
    eval_stack: [i32; MAX_PLY as usize], // static eval at each ply for "improving" check
}

impl SearchState {
    fn new() -> Self {
        SearchState {
            killers: [[None; 2]; MAX_PLY as usize],
            history: [[0; 64]; 16],
            nodes: 0,
            eval_stack: [0; MAX_PLY as usize],
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
    fn update_history(&mut self, board: &Board, m: Move, bonus: i32) {
        let piece_idx = board.mailbox[m.from_sq() as usize] as usize;
        let h = &mut self.history[piece_idx][m.to_sq() as usize];
        // Gravity formula: prevents unbounded growth
        *h += bonus - (*h * bonus.abs() / 16384);
    }

    #[inline]
    fn get_history(&self, board: &Board, m: Move) -> i32 {
        let piece_idx = board.mailbox[m.from_sq() as usize] as usize;
        self.history[piece_idx][m.to_sq() as usize]
    }
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
    let victim = if m.flags() == Move::EN_PASSANT {
        100
    } else {
        piece_value(board.mailbox[m.to_sq() as usize] & 0x07)
    };
    let attacker = piece_value(board.mailbox[m.from_sq() as usize] & 0x07);
    victim * 10 - attacker
}

#[inline]
fn piece_value(pt: u8) -> i32 {
    match pt {
        Piece::PAWN => 100,
        Piece::KNIGHT => 320,
        Piece::BISHOP => 330,
        Piece::ROOK => 500,
        Piece::QUEEN => 900,
        Piece::KING => 0,
        _ => 0,
    }
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
            let (mut alpha, mut beta, mut window) = if depth >= 4 && absolute_best_score.abs() < MATE_BOUND {
                (absolute_best_score - 25, absolute_best_score + 25, 25i32)
            } else {
                (-INF, INF, INF)
            };

            let mut search_aborted;
            let mut current_depth_best_move;
            let mut current_depth_best_score;

            // Aspiration window loop: widen gradually if search falls outside window
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

                // Widen aspiration window gradually
                if current_depth_best_score <= alpha.saturating_sub(window) {
                    window *= 4;
                    alpha = absolute_best_score.saturating_sub(window);
                    beta = absolute_best_score.saturating_add(window);
                    if window > 1000 { alpha = -INF; beta = INF; }
                    continue;
                }
                if current_depth_best_score >= beta {
                    window *= 4;
                    alpha = absolute_best_score.saturating_sub(window);
                    beta = absolute_best_score.saturating_add(window);
                    if window > 1000 { alpha = -INF; beta = INF; }
                    continue;
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

    let is_pv = beta - alpha > 1;
    let hash_key = board.zobrist_hash;

    if ply > 0 && (board.halfmove_clock >= 100 || is_draw_by_repetition(board.halfmove_clock, history, hash_key)) {
        return 0;
    }

    let original_alpha = alpha;
    let mut tt_move = None;

    if let Some(entry) = tt.probe(hash_key) {
        tt_move = entry.best_move;
        // Only use TT cutoffs in non-PV nodes
        if !is_pv && entry.depth >= depth {
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

    // Static eval for pruning decisions
    let static_eval = board.evaluate_board();
    ss.eval_stack[ply as usize] = static_eval;

    // Is our position improving compared to 2 plies ago?
    let improving = ply >= 2 && static_eval > ss.eval_stack[(ply - 2) as usize];

    // ---- Reverse Futility Pruning (Static Null Move Pruning) ----
    // If we're already so far ahead that even after subtracting a margin we still beat beta,
    // it's extremely unlikely any move will change that. Skip the search.
    // NEVER prune when mate scores are involved — we need to find the actual mate.
    if !is_pv && !in_check && depth <= 6
        && static_eval - RFP_MARGIN * (depth as i32) >= beta
        && beta.abs() < MATE_BOUND
    {
        return static_eval;
    }

    // --- Null Move Pruning ---
    if !is_pv && depth >= 3 && !in_check && board.has_non_pawn_material()
        && static_eval >= beta && beta.abs() < MATE_BOUND
    {
        let mut null_board = *board;
        let z = zobrist();
        null_board.zobrist_hash ^= z.black_to_move;
        if let Some(ep_sq) = null_board.en_passant_square {
            null_board.zobrist_hash ^= z.en_passant[(ep_sq % 8) as usize];
        }
        null_board.white_to_move = !null_board.white_to_move;
        null_board.en_passant_square = None;

        let r = 3 + depth / 4 + ((static_eval - beta) / 200).min(3) as u32;
        let null_score = -alpha_beta(&null_board, -beta, -beta + 1, depth.saturating_sub(r + 1), ply + 1, abort, ss, tt, history);

        if null_score >= beta {
            // Don't return unproven mate scores from null move
            if null_score >= MATE_BOUND { return beta; }
            return null_score;
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
        } else if m.is_capture() && m.is_promotion() {
            scores[i] = PROMOTE_BONUS + 100; // capture-promotion is best
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
    let futility_pruning = !is_pv && !in_check && depth <= 3
        && static_eval + FUTILITY_MARGIN * (depth as i32) <= alpha
        && alpha.abs() < MATE_BOUND;

    // Track quiet moves searched so we can penalize them on cutoffs
    let mut quiets_searched = [Move(0); MAX_MOVES];
    let mut num_quiets_searched = 0;

    for i in 0..count {
        pick_next_best_move(&mut moves, &mut scores, i, count);
        let m = moves[i];

        let mut new_board = *board;
        new_board.make_move(m);

        if !is_move_legal(&new_board, us, enemy) { continue; }

        legal_moves += 1;
        let is_quiet = !m.is_capture() && !m.is_promotion();
        let extension = if gives_check(&new_board, us, enemy) { 1 } else { 0 };

        // ---- Futility Pruning ----
        // At low depths, if static eval + margin is still below alpha,
        // quiet moves are unlikely to improve things. Skip them.
        if futility_pruning && legal_moves > 1 && is_quiet && extension == 0 {
            continue;
        }

        // ---- Late Move Pruning ----
        // At very low depths, after trying enough moves, stop generating quiet moves
        if !is_pv && depth <= 3 && is_quiet && !in_check
            && best_score.abs() < MATE_BOUND
            && legal_moves > (3 + 4 * depth as usize) {
            continue;
        }

        // ---- Late Move Reductions ----
        let mut reduction: u32 = 0;
        if depth >= 3 && legal_moves > 1 && is_quiet && extension == 0 && !in_check {
            let d = (depth as usize).min(63);
            let m_idx = (legal_moves as usize).min(63);
            reduction = LMR_TABLE[d][m_idx];

            // Reduce less in PV nodes
            if is_pv && reduction > 0 { reduction -= 1; }
            // Reduce less if position is improving
            if improving && reduction > 0 { reduction -= 1; }
            // Reduce more for moves with bad history
            if ss.get_history(board, m) < -1000 { reduction += 1; }

            // Don't reduce into qsearch (ensure at least depth 1)
            reduction = reduction.min(depth - 1);
        }

        history.push(hash_key);
        let mut score;

        // PVS: First move gets full window, rest get null window first
        if legal_moves == 1 {
            score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, ss, tt, history);
        } else {
            // Scout search with null window + reduction
            score = -alpha_beta(&new_board, -alpha - 1, -alpha, depth - 1 + extension - reduction, ply + 1, abort, ss, tt, history);
            // Re-search at full depth if reduced search beat alpha
            if score > alpha && reduction > 0 {
                score = -alpha_beta(&new_board, -alpha - 1, -alpha, depth - 1 + extension, ply + 1, abort, ss, tt, history);
            }
            // Full PV re-search if scout beat alpha in a PV node
            if score > alpha && score < beta {
                score = -alpha_beta(&new_board, -beta, -alpha, depth - 1 + extension, ply + 1, abort, ss, tt, history);
            }
        }
        history.pop();

        if abort.load(Ordering::Relaxed) { return 0; }

        if is_quiet {
            quiets_searched[num_quiets_searched] = m;
            num_quiets_searched += 1;
        }

        if score > best_score {
            best_score = score;
            best_move = Some(m);
        }

        if score >= beta {
            // On a beta cutoff by a quiet move:
            if is_quiet {
                // 1. Store killer
                ss.store_killer(ply, m);
                // 2. Give a history bonus to the move that caused the cutoff
                let bonus = (depth * depth) as i32;
                ss.update_history(board, m, bonus);
                // 3. Penalize all other quiet moves that were tried and failed
                for j in 0..num_quiets_searched {
                    if quiets_searched[j] != m {
                        ss.update_history(board, quiets_searched[j], -bonus);
                    }
                }
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

    let in_check = board.is_in_check();
    let (us, enemy) = get_colors(board);

    // If in check, we can't stand pat — we must search all evasions
    if in_check {
        let mut moves = [Move(0); MAX_MOVES];
        let count = generate_all_moves(board, &mut moves);
        let mut legal_moves = 0;

        // Simple ordering: captures first, then quiets
        let mut scores = [0i32; MAX_MOVES];
        for i in 0..count {
            if moves[i].is_capture() {
                scores[i] = CAPTURE_BASE + mvv_lva(board, moves[i]);
            }
        }

        for i in 0..count {
            pick_next_best_move(&mut moves, &mut scores, i, count);
            let m = moves[i];

            let mut new_board = *board;
            new_board.make_move(m);
            if !is_move_legal(&new_board, us, enemy) { continue; }

            legal_moves += 1;
            let score = -quiescence_search(&new_board, -beta, -alpha, abort, ss);

            if abort.load(Ordering::Relaxed) { return 0; }
            if score >= beta { return beta; }
            alpha = alpha.max(score);
        }

        // No legal moves while in check = checkmate
        if legal_moves == 0 { return -MATE_SCORE; }
        return alpha;
    }

    // Not in check: stand pat
    let stand_pat = board.evaluate_board();
    if stand_pat >= beta { return stand_pat; }
    alpha = alpha.max(stand_pat);

    // Only search captures + promotions
    let mut captures = [Move(0); MAX_MOVES];
    let mut scores = [0i32; MAX_MOVES];
    let count = generate_captures(board, &mut captures);

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