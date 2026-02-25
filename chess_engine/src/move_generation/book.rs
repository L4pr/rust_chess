use std::collections::HashMap;
use rand::{RngExt};

#[derive(Clone)]
pub struct BookMove {
    pub uci: String,
    pub weight: u32,
}

const BOOK_DATA: &str = include_str!("../../resources/Book.txt");

pub struct OpeningBook {
    positions: HashMap<String, Vec<BookMove>>,
}

impl OpeningBook {
    /// Loads the book from your specific text format
    pub fn load_from_file() -> Self {
        let mut positions: HashMap<String, Vec<BookMove>> = HashMap::new();
        let mut current_fen = String::new();

        println!("info string loading positions from book.");

        for line in BOOK_DATA.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }

            if line.starts_with("pos ") {
                // Extract everything after "pos "
                current_fen = line[4..].trim().to_string();
                // Ensure the vector exists for this FEN
                positions.entry(current_fen.clone()).or_insert_with(Vec::new);
            } else {
                // It's a move line! Split it by spaces. Example: "e2e4 243109"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 {
                    let uci = parts[0].to_string();
                    let weight: u32 = parts[1].parse().unwrap_or(0);

                    if let Some(moves) = positions.get_mut(&current_fen) {
                        moves.push(BookMove { uci, weight });
                    }
                }
            }
        }

        println!("info string Loaded {} positions from book.", positions.len());
        Self { positions }
    }

    /// Picks a move based on weighted randomness
    pub fn get_book_move(&self, fen: &str) -> Option<String> {
        let moves = self.positions.get(fen)?;
        if moves.is_empty() { return None; }

        // Calculate total weight of all moves for this position
        let total_weight: u32 = moves.iter().map(|m| m.weight).sum();
        if total_weight == 0 { return None; }

        // Pick a random number between 0 and total_weight
        let mut rng = rand::rng();
        let mut choice = rng.random_range(0..total_weight);

        // Find which move that random number falls into
        for m in moves {
            if choice < m.weight {
                return Some(m.uci.clone());
            }
            choice -= m.weight;
        }

        None
    }
}