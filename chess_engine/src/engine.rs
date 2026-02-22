use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use crate::{Board, Move, generate_all_moves};
use rand::prelude::*;
use std::thread;
use std::time::Duration;

pub struct Engine {
    board: Board,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            board: Board::starting_position(),
        }
    }

    pub fn set_board(&mut self, new_board: Board) {
        self.board = new_board;
    }

    pub fn think(&mut self, time_limit_ms: u64, abort: Arc<AtomicBool>) -> Option<Move> {
        let mut move_storage = [Move(0); 218];
        let count = generate_all_moves(&self.board, &mut move_storage);
        let random = rand::rng().random_range(0..count);
        thread::sleep(Duration::from_secs(2));
        if count > 0 {
            return Some(move_storage[random]);
        }
        None
    }


}