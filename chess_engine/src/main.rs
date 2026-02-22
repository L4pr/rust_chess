use std::io::{self, BufRead};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use chess_engine::{Board, Engine};


fn main() {
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

                let move_time = parts.iter()
                    .position(|&r| r == "movetime")
                    .and_then(|i| parts.get(i + 1))
                    .and_then(|t| t.parse::<u64>().ok())
                    .unwrap_or(1000);

                let abort_clone = Arc::clone(&abort_search);

                thread::spawn(move || {
                    // Give the engine a 50ms buffer to finish its last node and print
                    let sleep_duration = if move_time > 50 { move_time - 50 } else { move_time };
                    thread::sleep(Duration::from_millis(sleep_duration));
                    abort_clone.store(true, Ordering::Relaxed);
                });

                let board_clone = board.clone();
                let abort_for_search = Arc::clone(&abort_search);

                thread::spawn(move || {
                    calculate_best_move(board_clone, abort_for_search);
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

/// DUMMY FUNCTION: The actual chess math.
fn calculate_best_move(board: Board, abort: Arc<AtomicBool>) {
    let mut engine = Engine::new();
    engine.set_board(board);
    let bestmove = engine.think(abort).unwrap();
    println!("bestmove {}", bestmove.to_uci());
}