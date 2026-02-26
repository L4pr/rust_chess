pub mod board;
pub mod move_generation;
pub mod engine;

// Re-export commonly used items for convenience
pub use board::board::Board;
pub use board::pieces::{Piece};
pub use board::move_struct::Move;
pub use board::castling_rights::CastlingRights;
pub use move_generation::generate_moves::generate_all_moves;
pub use engine::Engine;
pub use move_generation::generate_moves::is_square_attacked;
pub use move_generation::tests::perft;
pub use board::zobrist::ZobristKeys;
pub use move_generation::book::OpeningBook;
pub use board::evaluation::evaluate;
pub use board::board::is_draw_by_repetition;
