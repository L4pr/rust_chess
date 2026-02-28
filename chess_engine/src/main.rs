use std::io::{self, BufRead};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use chess_engine::{Board, Engine, init_magic_bitboards};


fn main() {
    // Initialize magic bitboards once at startup
    init_magic_bitboards();
    
    let engine = Arc::new(Mutex::new(Engine::new()));
    // This flag allows the main loop to tell the search thread to stop!
    let abort_search = Arc::new(AtomicBool::new(false));
    let mut board = Board::starting_position();
    
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let cmd = line.expect("Failed to read line").trim().to_string();
        if cmd.is_empty() { continue; }

        let parts: Vec<&str> = cmd.split_whitespace().collect();

        match parts[0] {
            // 1. GUI says: "Hello, are you a UCI engine?"
            "uci" => {
                println!("id name rust_chess");
                println!("id author Renzo");
                println!("uciok");
            }
            // 2. GUI says: "Are you done loading your memory?"
            "isready" => {
                println!("readyok");
            }
            // 3. GUI says: "Start a new game."
            "ucinewgame" => {
                engine.lock().unwrap().clear_tt();
                board = Board::starting_position();
            }
            // 4. GUI says: "Here is the current board."
            // Example: "position startpos moves e2e4 e7e5"
            "position" => {
                parse_position(&mut board, parts, &cmd);
            }
            // 5. GUI says: "Start calculating the best move!"
            "go" => {
                // Reset the abort flag
                abort_search.store(false, Ordering::Relaxed);

                let abort_clone = Arc::clone(&abort_search);
                let board_clone2 = board.clone();

                let (tx, rx) = mpsc::channel::<()>();

                thread::spawn(move || {
                    let time_limit = handle_go(&cmd, board_clone2);

                    // Wait for signal OR timeout
                    match rx.recv_timeout(Duration::from_millis(time_limit as u64)) {
                        Ok(_) => {
                            // Search finished early! We received the signal from tx.
                            // Do nothing and exit.
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // Time is up!
                            abort_clone.store(true, Ordering::SeqCst);
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            abort_clone.store(true, Ordering::SeqCst);
                            // Something went wrong, just exit safely.
                        }
                    }
                });

                let board_clone = board.clone();
                let abort_for_search = Arc::clone(&abort_search);
                let engine_clone = Arc::clone(&engine);

                thread::spawn(move || {
                    let mut eng = engine_clone.lock().unwrap();
                    eng.set_board(board_clone);
                    let bestmove = eng.think(abort_for_search).unwrap();
                    let _ = tx.send(());
                    println!("bestmove {}", bestmove.to_uci());
                });
            }
            // 6. GUI says: "Stop calculating immediately and give me your best guess!"
            "stop" => {
                abort_search.store(true, Ordering::Relaxed);
            }
            "quit" => {
                abort_search.store(true, Ordering::Relaxed);
                break;
            }
            _ => {
                // Ignore unknown commands (part of the UCI standard)
            }
        }
    }
}

fn parse_position(board: &mut Board, parts: Vec<&str>, cmd: &str) {
    if parts.len() > 1 && parts[1] == "startpos" {
        *board = Board::starting_position();
    } else if parts.len() > 1 && parts[1] == "fen" {
        let fen_str = cmd[13..].split("moves").next().unwrap().trim(); // Extract the FEN part before " moves "
        *board = Board::from_fen(fen_str);
    }
    let new_parts = cmd.split("moves").collect::<Vec<&str>>();
    if new_parts.len() > 1 {
        let moves_str = new_parts[1].trim();
        for move_str in moves_str.split_whitespace() {
            if let Some(m) = board.parse_uci_to_move(move_str) {
                board.make_move(m);
            } else {
                eprintln!("info string Error: Illegal move received: {}", move_str);
            }
        }
    }
}

fn handle_go(command: &str, board: Board) -> u64 {
    let parts: Vec<&str> = command.split_whitespace().collect();

    let bot_is_white = board.white_to_move;

    // Default values if not found
    let mut time = 30000; // 30s default
    let mut inc = 0;

    // Search the parts for the correct time label
    let time_label = if bot_is_white { "wtime" } else { "btime" };
    let inc_label = if bot_is_white { "winc" } else { "binc" };

    if let Some(pos) = parts.iter().position(|&r| r == time_label) {
        time = parts[pos + 1].parse().unwrap_or(30000);
    }
    if let Some(pos) = parts.iter().position(|&r| r == inc_label) {
        inc = parts[pos + 1].parse().unwrap_or(0);
    }

    calculate_thinking_time(time, inc)
}

pub fn calculate_thinking_time(available_time_ms: u64, increment_ms: u64) -> u64 {
    // 1. Never use all your time. Keep a safety margin (e.g., 50ms) for network lag.
    if available_time_ms < 100 {
        return available_time_ms.saturating_sub(20);
    }

    // 2. Estimate how many moves are left in the game.
    // 30-40 is a standard "divisor" for mid-game.
    let move_divisor = 30;

    // 3. Basic formula: (Remaining Time / moves_left) + increment
    // We subtract a small "overhead" buffer to be safe.
    let base_time = available_time_ms / move_divisor;
    let mut target_time = base_time + increment_ms;

    // 4. Emergency check: Don't spend more than 20% of your total remaining time on ONE move.
    let max_safety_limit = available_time_ms / 5;
    if target_time > max_safety_limit {
        target_time = max_safety_limit;
    }

    target_time.saturating_sub(50) // Subtract 50ms for Lichess/Bridge overhead
}