use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;
use chess_engine::{Board, Engine, Move, SearchResult, init_magic_bitboards, init_zobrist};

/// Message sent from the search/ponder thread back to main.
enum SearchMsg {
    /// Real search finished: best move + optional ponder move
    Done(SearchResult),
    /// Ponder search was stopped (no result needed from caller)
    PonderStopped,
}

fn main() {
    init_magic_bitboards();
    init_zobrist();

    let engine = Arc::new(Mutex::new(Engine::new()));
    let abort = Arc::new(AtomicBool::new(false));
    let mut board = Board::starting_position();
    let mut game_history: Vec<u64> = Vec::new();

    // Channel for search/ponder results
    let (msg_tx, msg_rx) = mpsc::channel::<SearchMsg>();

    // Track whether a background ponder is running
    let mut ponder_pending = false;

    // Generation counter: incremented on each new "go" command.
    // Timer threads capture the current generation; if it's stale when they
    // wake up, they know a new search has started and do NOT set abort.
    let search_gen = Arc::new(AtomicU64::new(0));

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let cmd = match line {
            Ok(l) => l.trim().to_string(),
            Err(_) => break,
        };
        if cmd.is_empty() { continue; }

        let parts: Vec<&str> = cmd.split_whitespace().collect();

        match parts[0] {
            "uci" => {
                println!("id name rust_chess");
                println!("id author Renzo");
                println!("uciok");
            }

            "isready" => {
                println!("readyok");
            }

            "ucinewgame" => {
                stop_ponder(&abort, &msg_rx, &mut ponder_pending);
                engine.lock().unwrap().clear_tt();
                board = Board::starting_position();
                game_history.clear();
            }

            "position" => {
                stop_ponder(&abort, &msg_rx, &mut ponder_pending);
                game_history.clear();
                parse_position(&mut board, &parts, &cmd, &mut game_history);
            }

            "go" => {
                stop_ponder(&abort, &msg_rx, &mut ponder_pending);
                abort.store(false, Ordering::SeqCst);

                // Bump generation so any old timer threads become stale
                let generation = search_gen.fetch_add(1, Ordering::SeqCst) + 1;

                let time_limit = parse_go(&parts, &board);
                let board_clone = board;
                let abort_clone = Arc::clone(&abort);
                let engine_clone = Arc::clone(&engine);
                let tx = msg_tx.clone();
                let history_clone = game_history.clone();

                // Timer thread: only abort if this generation is still current
                if time_limit < u64::MAX {
                    let abort_timer = Arc::clone(&abort);
                    let gen_ref = Arc::clone(&search_gen);
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(time_limit));
                        // Only abort if no newer search has started
                        if gen_ref.load(Ordering::SeqCst) == generation {
                            abort_timer.store(true, Ordering::SeqCst);
                        }
                    });
                }

                // Search thread
                thread::spawn(move || {
                    let mut eng = engine_clone.lock().unwrap();
                    eng.set_board(board_clone);
                    eng.set_game_history(history_clone);
                    if let Some(result) = eng.think(abort_clone) {
                        let _ = tx.send(SearchMsg::Done(result));
                    }
                });

                // Block until search finishes (timer or "stop" will abort it)
                if let Ok(SearchMsg::Done(result)) = msg_rx.recv() {
                    // Format the bestmove output
                    let mut output = format!("bestmove {}", result.best_move.to_uci());
                    if let Some(pm) = result.ponder_move {
                        output.push_str(&format!(" ponder {}", pm.to_uci()));
                    }
                    println!("{}", output);
                    let _ = io::stdout().flush();

                    // Start pondering if we have a predicted opponent move
                    if let Some(ponder_move) = result.ponder_move {
                        start_ponder(
                            &board, result.best_move, ponder_move,
                            &engine, &abort, &msg_tx, &mut ponder_pending,
                        );
                    }
                }
            }

            "stop" => {
                abort.store(true, Ordering::SeqCst);
            }

            "quit" => {
                abort.store(true, Ordering::SeqCst);
                break;
            }

            "d" | "display" => {
                print_board(&board);
            }

            _ => {}
        }
    }
}

// ============================================================
// Pondering
// ============================================================

/// Start a background ponder search.
/// We assume our best move + the predicted opponent reply, then search from there.
fn start_ponder(
    board: &Board,
    our_move: Move,
    ponder_move: Move,
    engine: &Arc<Mutex<Engine>>,
    abort: &Arc<AtomicBool>,
    tx: &mpsc::Sender<SearchMsg>,
    ponder_pending: &mut bool,
) {
    // Build the ponder board: current → our move → opponent's predicted reply
    let mut ponder_board = *board;
    ponder_board.make_move(our_move);
    ponder_board.make_move(ponder_move);

    // Reset abort for the ponder search
    abort.store(false, Ordering::SeqCst);

    let engine_clone = Arc::clone(engine);
    let abort_clone = Arc::clone(abort);
    let tx_clone = tx.clone();

    thread::spawn(move || {
        let mut eng = engine_clone.lock().unwrap();
        eng.ponder(ponder_board, abort_clone);
        let _ = tx_clone.send(SearchMsg::PonderStopped);
    });

    *ponder_pending = true;
}

// ============================================================
// Search control
// ============================================================

fn stop_ponder(abort: &Arc<AtomicBool>, rx: &mpsc::Receiver<SearchMsg>, ponder_pending: &mut bool) {
    if *ponder_pending {
        abort.store(true, Ordering::SeqCst);
        // Wait for PonderStopped message
        loop {
            match rx.recv() {
                Ok(SearchMsg::PonderStopped) => break,
                Ok(_) => {} // drain any other messages
                Err(_) => break,
            }
        }
        *ponder_pending = false;
    }
}

// ============================================================
// UCI Parsing
// ============================================================

fn parse_position(board: &mut Board, parts: &[&str], cmd: &str, game_history: &mut Vec<u64>) {
    if parts.len() > 1 && parts[1] == "startpos" {
        *board = Board::starting_position();
    } else if parts.len() > 1 && parts[1] == "fen" {
        let fen_part = cmd[13..].split(" moves").next().unwrap_or("").trim();
        *board = Board::from_fen(fen_part);
    }

    // Record the initial position hash
    game_history.push(board.zobrist_hash);

    if let Some(moves_idx) = cmd.find(" moves ") {
        for move_str in cmd[moves_idx + 7..].split_whitespace() {
            if let Some(m) = board.parse_uci_to_move(move_str) {
                board.make_move(m);
                game_history.push(board.zobrist_hash);
            } else {
                eprintln!("info string Error: Illegal move '{}'", move_str);
            }
        }
    }
}

fn parse_go(parts: &[&str], board: &Board) -> u64 {
    if parts.contains(&"infinite") { return u64::MAX; }
    if parts.contains(&"depth") { return u64::MAX; }
    if let Some(val) = get_param(parts, "movetime") {
        return val.saturating_sub(20);
    }

    let (time_key, inc_key) = if board.white_to_move {
        ("wtime", "winc")
    } else {
        ("btime", "binc")
    };

    let time = get_param(parts, time_key).unwrap_or(30000);
    let inc = get_param(parts, inc_key).unwrap_or(0);
    let moves_to_go = get_param(parts, "movestogo");

    calculate_think_time(time, inc, moves_to_go)
}

fn get_param(parts: &[&str], key: &str) -> Option<u64> {
    parts.iter()
        .position(|&p| p == key)
        .and_then(|i| parts.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn calculate_think_time(time_ms: u64, inc_ms: u64, moves_to_go: Option<u64>) -> u64 {
    if time_ms < 100 {
        return time_ms.saturating_sub(20);
    }

    let divisor = match moves_to_go {
        Some(mtg) if mtg > 0 => mtg.min(40),
        _ => 25,
    };

    let base = time_ms / divisor;
    let mut target = base + (inc_ms * 3 / 4);

    let max_limit = time_ms / 4;
    if target > max_limit {
        target = max_limit;
    }

    target.saturating_sub(30)
}

// ============================================================
// Debug display
// ============================================================

fn print_board(board: &Board) {
    let piece_char = |sq: usize| -> char {
        let p = board.mailbox[sq];
        if p == 0xFF { return '.'; }
        let ch = match p & 0x07 {
            1 => 'p', 2 => 'n', 3 => 'b', 4 => 'r', 5 => 'q', 6 => 'k', _ => '?',
        };
        if (p & 0x08) == 0 { ch.to_ascii_uppercase() } else { ch }
    };

    eprintln!();
    for rank in (0..8).rev() {
        eprint!("  {} ", rank + 1);
        for file in 0..8 {
            eprint!(" {}", piece_char(rank * 8 + file));
        }
        eprintln!();
    }
    eprintln!("     a b c d e f g h");
    eprintln!("  FEN: {}", board.get_fen());
    eprintln!("  Zobrist: {:016x}", board.zobrist_hash);
    eprintln!();
}