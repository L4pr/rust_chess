#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move(pub u16);

impl Move {
    const FROM_MASK: u16 = 0b0000_0000_0011_1111; // Bits 0-5 (0x3F)
    const TO_MASK: u16   = 0b0000_1111_1100_0000; // Bits 6-11 (0xFC0)
    const FLAG_MASK: u16 = 0b1111_0000_0000_0000; // Bits 12-15 (0xF000)

    // --- Standard Move Flags ---
    pub const QUIET: u16             = 0;
    pub const DOUBLE_PAWN_PUSH: u16  = 1;
    pub const KING_CASTLE: u16       = 2;
    pub const QUEEN_CASTLE: u16      = 3;
    pub const CAPTURE: u16           = 4;
    pub const EN_PASSANT: u16        = 5;

    // Promotions (Add 8 to make it a promotion, add 12 if it's also a capture)
    pub const PR_KNIGHT: u16         = 8;
    pub const PR_BISHOP: u16         = 9;
    pub const PR_ROOK: u16           = 10;
    pub const PR_QUEEN: u16          = 11;

    pub const PC_KNIGHT: u16         = 12; // Promotion + Capture
    pub const PC_BISHOP: u16         = 13;
    pub const PC_ROOK: u16           = 14;
    pub const PC_QUEEN: u16          = 15;

    /// Constructor: Packs the from square, to square, and flags into a u16
    pub fn new_with_flags(from: u8, to: u8, flags: u16) -> Self {
        Move((from as u16) | ((to as u16) << 6) | (flags << 12))
    }

    pub fn new(from: u8, to: u8) -> Self {
        Move((from as u16) | ((to as u16) << 6))
    }

    /// Extracts the starting square (0-63)
    pub fn from_sq(&self) -> u8 {
        (self.0 & Self::FROM_MASK) as u8
    }

    /// Extracts the destination square (0-63)
    pub fn to_sq(&self) -> u8 {
        ((self.0 & Self::TO_MASK) >> 6) as u8
    }

    /// Extracts the move flags (0-15)
    pub fn flags(&self) -> u16 {
        (self.0 & Self::FLAG_MASK) >> 12
    }

    // --- Helpful Utility Methods ---

    /// Returns true if the move is any kind of capture
    pub fn is_capture(&self) -> bool {
        let f = self.flags();
        f == Self::CAPTURE || f == Self::EN_PASSANT || f >= Self::PC_KNIGHT
    }

    /// Returns true if the move is any kind of promotion
    pub fn is_promotion(&self) -> bool {
        self.flags() >= Self::PR_KNIGHT
    }

    pub fn to_uci(&self) -> String {
        let from = self.from_sq();
        let to = self.to_sq();

        let from_file = (b'a' + (from % 8)) as char;
        let from_rank = (b'1' + (from / 8)) as char;
        let to_file   = (b'a' + (to % 8)) as char;
        let to_rank   = (b'1' + (to / 8)) as char;

        let mut uci = format!("{}{}{}{}", from_file, from_rank, to_file, to_rank);

        // Append promotion character if necessary
        if self.is_promotion() {
            let promo_char = match self.flags() {
                Self::PR_KNIGHT | Self::PC_KNIGHT => 'n',
                Self::PR_BISHOP | Self::PC_BISHOP => 'b',
                Self::PR_ROOK   | Self::PC_ROOK   => 'r',
                _ => 'q', // Default to queen
            };
            uci.push(promo_char);
        }

        uci
    }
}