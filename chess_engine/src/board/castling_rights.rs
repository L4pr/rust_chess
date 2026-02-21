#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastlingRights(pub u8);

impl CastlingRights {
    // Bitmask Constants for Castling Rights
    pub const WHITE_KINGSIDE: u8 = 0b0001; // 1
    pub const WHITE_QUEENSIDE: u8 = 0b0010; // 2
    pub const BLACK_KINGSIDE: u8 = 0b0100; // 4
    pub const BLACK_QUEENSIDE: u8 = 0b1000; // 8

    /// Constructor: Creates a new CastlingRights from a bitmask
    pub fn new(rights: u8) -> Self {
        CastlingRights(rights)
    }

    /// Checks if White can castle kingside
    pub fn white_kingside(&self) -> bool {
        (self.0 & Self::WHITE_KINGSIDE) != 0
    }

    /// Checks if White can castle queenside
    pub fn white_queenside(&self) -> bool {
        (self.0 & Self::WHITE_QUEENSIDE) != 0
    }

    /// Checks if Black can castle kingside
    pub fn black_kingside(&self) -> bool {
        (self.0 & Self::BLACK_KINGSIDE) != 0
    }

    /// Checks if Black can castle queenside
    pub fn black_queenside(&self) -> bool {
        (self.0 & Self::BLACK_QUEENSIDE) != 0
    }

    /// Removes specific rights using a mask
    pub fn remove(&mut self, mask: u8) {
        self.0 &= !mask;
    }

    /// Adds specific rights (useful when unmaking a move)
    pub fn add(&mut self, mask: u8) {
        self.0 |= mask;
    }
}