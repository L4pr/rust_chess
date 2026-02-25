#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece(pub u8);

impl Piece {
    // Define Bitmask Constants
    const COLOR_MASK: u8 = 0b0000_1000; // 4th bit (8)
    const TYPE_MASK: u8  = 0b0000_0111; // first 3 bits (0-7)

    // Define Piece Types as numbers
    pub const PAWN: u8 = 1;
    pub const KNIGHT: u8 = 2;
    pub const BISHOP: u8 = 3;
    pub const ROOK: u8 = 4;
    pub const QUEEN: u8 = 5;
    pub const KING: u8 = 6;
    pub const ALL: u8 = 7;

    // Define Colors as numbers
    pub const WHITE: u8  = 0;
    pub const BLACK: u8  = 8;

    /// Constructor: Creates a piece from a color and type
    pub fn new(color: u8, piece_type: u8) -> Self {
        Piece(color | piece_type)
    }

    /// Extract the color (returns 0 for White, 8 for Black)
    pub fn is_white(&self) -> bool {
        (self.0 & Self::COLOR_MASK) == 0
    }

    /// Extract the type (returns 1-6)
    pub fn piece_type(&self) -> u8 {
        self.0 & Self::TYPE_MASK
    }
}