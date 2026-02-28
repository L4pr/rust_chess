// Magic Bitboard Implementation for sliding piece move generation
// This provides ultra-fast lookups for bishop and rook moves

use std::sync::OnceLock;

// Pre-computed magic numbers
const ROOK_MAGICS: [u64; 64] = [
    0x0480053040008222, 0x4040100020004000, 0x1500104008200100, 0x0100050008201002,
    0xa080028800808400, 0x0600020010080415, 0x1100040082000100, 0x0200108040240a01,
    0x0850800020400082, 0x0152002042108100, 0x800a001200244086, 0x0000801000800800,
    0x0001001008020500, 0x0000800400800200, 0x1104000402086110, 0x010a000200408104,
    0x0010218002884001, 0x0000808040002000, 0x0050808020001000, 0x8400210009001000,
    0x0108008004000880, 0x0084808004000200, 0x0c00040010080201, 0x4000020000a1004c,
    0x0001400180006080, 0x0000420200210084, 0x2020004040100801, 0x0010040040400800,
    0x0000080080040080, 0x0000020080040080, 0x8008108400010842, 0x0000800280014500,
    0x061040018c800422, 0x0000400084802010, 0x5410104101002000, 0xd010080080801000,
    0x0400800800800400, 0x2110800400800200, 0x1401000401000200, 0x0212004102002084,
    0x6000400080008020, 0x0030004820004000, 0x0110801200420022, 0x8000090010010021,
    0x0388000811010004, 0x0001000400090002, 0x04a0100108040002, 0x1040804400820001,
    0x1094214080090100, 0x1040002010080120, 0x0220801200204200, 0x0300100021000900,
    0x0242002008041200, 0x2000040002008080, 0xa030021108501400, 0x2001140081284200,
    0x0824800100496211, 0x0100810040002011, 0x040820000b005043, 0x1004040900201001,
    0x2083000800040211, 0x0009001400020803, 0x80e0008228100104, 0x8900044285003402,
];

const BISHOP_MAGICS: [u64; 64] = [
    0x80080108022c4200, 0x3882880801004c00, 0x0809081201800440, 0x008404019a0000d4,
    0x0404042110842800, 0x0601100210000880, 0x0000880802100880, 0x8402020a84140210,
    0x0003085004881040, 0x00280810008a2241, 0x00400414008a0000, 0x0041082052400000,
    0x01002403084a0042, 0x2600082808080804, 0x08000c04246c1440, 0x48a3108400c80401,
    0x2008011202080800, 0x2208100408108400, 0x8021041010408104, 0x8048049022404084,
    0x2405001811400000, 0x0001000e03010150, 0x0405001200826020, 0x04c040182a021006,
    0x00201800a0095140, 0x2042101009104082, 0x04003000a8004041, 0x4040040044410020,
    0x0a01010020104004, 0x0208020212404a01, 0x200110410a082402, 0x0004820035044210,
    0x9004040400a0a100, 0x108801100008d248, 0x0001282800500084, 0x00281008201c0400,
    0x90c0010010410040, 0x8208882200884100, 0x4008008120041108, 0x808d010128070400,
    0x0412020240012080, 0x0404424220083000, 0x8212020424000a00, 0x01000a0202040421,
    0x0004281010100104, 0x1010200a04100020, 0x3006c80204140790, 0x480810a10210c240,
    0x04060201a0081010, 0x8004208c10188000, 0x0020008058080501, 0x0044141084040000,
    0x0000099082020000, 0x200020208a088280, 0x0840108420988090, 0x348404ac00460002,
    0x0021004802011000, 0x2000090045542004, 0x0011002084008800, 0x8188000650208800,
    0x200004a410020203, 0x0090210810042828, 0x0020081101080100, 0x0003080608060920,
];

const ROOK_SHIFTS: [u8; 64] = [
    52, 53, 53, 53, 53, 53, 53, 52,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    53, 54, 54, 54, 54, 54, 54, 53,
    52, 53, 53, 53, 53, 53, 53, 52,
];

const BISHOP_SHIFTS: [u8; 64] = [
    58, 59, 59, 59, 59, 59, 59, 58,
    59, 59, 59, 59, 59, 59, 59, 59,
    59, 59, 57, 57, 57, 57, 59, 59,
    59, 59, 57, 55, 55, 57, 59, 59,
    59, 59, 57, 55, 55, 57, 59, 59,
    59, 59, 57, 57, 57, 57, 59, 59,
    59, 59, 59, 59, 59, 59, 59, 59,
    58, 59, 59, 59, 59, 59, 59, 58,
];

struct MagicEntry {
    mask: u64,
    magic: u64,
    shift: u8,
    attacks: Vec<u64>,
}

static ROOK_TABLE: OnceLock<Vec<MagicEntry>> = OnceLock::new();
static BISHOP_TABLE: OnceLock<Vec<MagicEntry>> = OnceLock::new();

/// Initialize the magic bitboard tables (called once at startup)
pub fn init_magic_bitboards() {
    ROOK_TABLE.get_or_init(|| build_table(true));
    BISHOP_TABLE.get_or_init(|| build_table(false));
}

/// Get rook attacks for a given square and occupancy
#[inline]
pub fn get_rook_attacks(square: u8, occupancy: u64) -> u64 {
    let table = ROOK_TABLE.get().expect("Magic bitboards not initialized");
    let entry = &table[square as usize];
    let index = ((occupancy & entry.mask).wrapping_mul(entry.magic) >> entry.shift) as usize;
    entry.attacks[index]
}

/// Get bishop attacks for a given square and occupancy
#[inline]
pub fn get_bishop_attacks(square: u8, occupancy: u64) -> u64 {
    let table = BISHOP_TABLE.get().expect("Magic bitboards not initialized");
    let entry = &table[square as usize];
    let index = ((occupancy & entry.mask).wrapping_mul(entry.magic) >> entry.shift) as usize;
    entry.attacks[index]
}

/// Get queen attacks (combination of rook and bishop)
#[inline]
pub fn get_queen_attacks(square: u8, occupancy: u64) -> u64 {
    get_rook_attacks(square, occupancy) | get_bishop_attacks(square, occupancy)
}

// ============================================================
// Table construction
// ============================================================

fn build_table(is_rook: bool) -> Vec<MagicEntry> {
    let mut table = Vec::with_capacity(64);

    for sq in 0u8..64 {
        let mask = if is_rook { rook_mask(sq) } else { bishop_mask(sq) };
        let shift = if is_rook { ROOK_SHIFTS[sq as usize] } else { BISHOP_SHIFTS[sq as usize] };
        let magic = if is_rook { ROOK_MAGICS[sq as usize] } else { BISHOP_MAGICS[sq as usize] };
        let table_size = 1usize << (64 - shift);

        let mut attacks = vec![0u64; table_size];

        // Enumerate all subsets of the mask using Carry-Rippler
        let mut occ = 0u64;
        loop {
            let attack = if is_rook {
                rook_attacks_slow(sq, occ)
            } else {
                bishop_attacks_slow(sq, occ)
            };

            let index = (occ.wrapping_mul(magic) >> shift) as usize;
            attacks[index] = attack;

            // Carry-Rippler trick to enumerate all subsets of mask
            occ = occ.wrapping_sub(mask) & mask;
            if occ == 0 { break; }
        }

        table.push(MagicEntry { mask, magic, shift, attacks });
    }

    table
}

// ============================================================
// Mask generators  (exclude edges – standard for magic BB)
// ============================================================

fn rook_mask(sq: u8) -> u64 {
    let rank = sq / 8;
    let file = sq % 8;
    let mut mask = 0u64;
    for r in (rank + 1)..7 { mask |= 1u64 << (r * 8 + file); }
    for r in 1..rank        { mask |= 1u64 << (r * 8 + file); }
    for f in (file + 1)..7  { mask |= 1u64 << (rank * 8 + f); }
    for f in 1..file         { mask |= 1u64 << (rank * 8 + f); }
    mask
}

fn bishop_mask(sq: u8) -> u64 {
    let rank = sq as i32 / 8;
    let file = sq as i32 % 8;
    let mut mask = 0u64;
    for i in 1..8 {
        let r = rank + i; let f = file + i;
        if r >= 7 || f >= 7 { break; }
        mask |= 1u64 << (r * 8 + f);
    }
    for i in 1..8 {
        let r = rank + i; let f = file - i;
        if r >= 7 || f <= 0 { break; }
        mask |= 1u64 << (r * 8 + f);
    }
    for i in 1..8 {
        let r = rank - i; let f = file + i;
        if r <= 0 || f >= 7 { break; }
        mask |= 1u64 << (r * 8 + f);
    }
    for i in 1..8 {
        let r = rank - i; let f = file - i;
        if r <= 0 || f <= 0 { break; }
        mask |= 1u64 << (r * 8 + f);
    }
    mask
}

// ============================================================
// Slow attack generators  (include edges – used to fill tables)
// ============================================================

fn rook_attacks_slow(sq: u8, occ: u64) -> u64 {
    let rank = sq as i32 / 8;
    let file = sq as i32 % 8;
    let mut attacks = 0u64;

    for i in 1..8 {
        let r = rank + i;
        if r > 7 { break; }
        let b = 1u64 << (r * 8 + file);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    for i in 1..8 {
        let r = rank - i;
        if r < 0 { break; }
        let b = 1u64 << (r * 8 + file);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    for i in 1..8 {
        let f = file + i;
        if f > 7 { break; }
        let b = 1u64 << (rank * 8 + f);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    for i in 1..8 {
        let f = file - i;
        if f < 0 { break; }
        let b = 1u64 << (rank * 8 + f);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    attacks
}

fn bishop_attacks_slow(sq: u8, occ: u64) -> u64 {
    let rank = sq as i32 / 8;
    let file = sq as i32 % 8;
    let mut attacks = 0u64;

    for i in 1..8 {
        let r = rank + i; let f = file + i;
        if r > 7 || f > 7 { break; }
        let b = 1u64 << (r * 8 + f);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    for i in 1..8 {
        let r = rank + i; let f = file - i;
        if r > 7 || f < 0 { break; }
        let b = 1u64 << (r * 8 + f);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    for i in 1..8 {
        let r = rank - i; let f = file + i;
        if r < 0 || f > 7 { break; }
        let b = 1u64 << (r * 8 + f);
        attacks |= b;
        if occ & b != 0 { break; }
    }
    for i in 1..8 {
        let r = rank - i; let f = file - i;
        if r < 0 || f < 0 { break; }
        let b = 1u64 << (r * 8 + f);
        attacks |= b;
        if occ & b != 0 { break; }
    }

    attacks
}


