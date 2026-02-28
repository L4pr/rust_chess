use std::io;
use std::io::Write;
use std::time::Instant;

fn main() {
    let mut rook_magics = [0u64; 64];
    let mut bishop_magics = [0u64; 64];
    let mut rook_shifts = [0u8; 64];
    let mut bishop_shifts = [0u8; 64];
    let mut seed = 728361249571u64;

    // Precompute all mask/subset/attack data once
    let mut rook_data: Vec<(u64, Vec<u64>, Vec<u64>)> = Vec::with_capacity(64);
    let mut bishop_data: Vec<(u64, Vec<u64>, Vec<u64>)> = Vec::with_capacity(64);
    for sq in 0..64 {
        rook_data.push(precompute_data(sq, true));
        bishop_data.push(precompute_data(sq, false));
    }

    // Find magics at optimal shifts (index bits = mask bits)
    // This is the theoretical minimum for standard (non-fancy) magic bitboards.
    // Each square needs 2^(mask_bits) table entries — there's no way around it
    // because that many distinct occupancy patterns exist.
    let start = Instant::now();
    println!("Finding optimal magic numbers...\n");

    for sq in 0..64 {
        let (mask, ref subs, ref atks) = rook_data[sq];
        rook_shifts[sq] = 64 - mask.count_ones() as u8;
        rook_magics[sq] = find_magic(subs, atks, rook_shifts[sq], &mut seed);

        let (mask, ref subs, ref atks) = bishop_data[sq];
        bishop_shifts[sq] = 64 - mask.count_ones() as u8;
        bishop_magics[sq] = find_magic(subs, atks, bishop_shifts[sq], &mut seed);

        print!("\r  {}/64 squares done", sq + 1);
        io::stdout().flush().unwrap();
    }

    let elapsed = start.elapsed();
    let total_kb = get_table_kb(&rook_shifts, &bishop_shifts);
    println!("\n\n  ✅ Done in {:.2?}", elapsed);
    println!("  Total lookup table size: {} KB", total_kb);
    println!("  (This is the minimum for standard magic bitboards.");
    println!("   ~80 KB is only achievable with \"fancy\" shared-table or PEXT approaches.)\n");

    print_array("ROOK_SHIFTS", &rook_shifts, "u8");
    print_array("BISHOP_SHIFTS", &bishop_shifts, "u8");
    print_hex_array("ROOK_MAGICS", &rook_magics);
    print_hex_array("BISHOP_MAGICS", &bishop_magics);
}

// ============================================================
// Precompute
// ============================================================

fn precompute_data(sq: usize, is_rook: bool) -> (u64, Vec<u64>, Vec<u64>) {
    let mask = if is_rook { mask_rook(sq) } else { mask_bishop(sq) };
    let mut subsets = Vec::new();
    let mut subset = 0u64;
    loop {
        subsets.push(subset);
        subset = subset.wrapping_sub(mask) & mask;
        if subset == 0 { break; }
    }
    let attacks: Vec<u64> = subsets.iter().map(|&s| {
        if is_rook { rook_attacks_slow(sq, s) } else { bishop_attacks_slow(sq, s) }
    }).collect();
    (mask, subsets, attacks)
}

fn find_magic(subsets: &[u64], actual_attacks: &[u64], shift: u8, seed: &mut u64) -> u64 {
    let table_size = 1usize << (64 - shift);
    let mut attack_table = vec![0u64; table_size];
    let mut epoch_table = vec![0u32; table_size];
    let num_subsets = subsets.len();

    for attempt in 1u32.. {
        let magic = xorshift(seed) & xorshift(seed) & xorshift(seed);
        if magic == 0 { continue; }

        let mut fail = false;
        for i in 0..num_subsets {
            let index = (subsets[i].wrapping_mul(magic) >> shift) as usize;

            if epoch_table[index] != attempt {
                epoch_table[index] = attempt;
                attack_table[index] = actual_attacks[i];
            } else if attack_table[index] != actual_attacks[i] {
                fail = true;
                break;
            }
        }

        if !fail {
            return magic;
        }
    }
    unreachable!()
}

#[inline]
fn xorshift(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn get_table_kb(r_shifts: &[u8; 64], b_shifts: &[u8; 64]) -> usize {
    let mut size = 0usize;
    for &s in r_shifts.iter() { size += 1 << (64 - s); }
    for &s in b_shifts.iter() { size += 1 << (64 - s); }
    (size * 8) / 1024
}

// ============================================================
// Mask generators (exclude edges)
// ============================================================

fn mask_rook(sq: usize) -> u64 {
    let mut m = 0u64;
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    for i in (r + 1)..7 { m |= 1 << (i * 8 + f); }
    for i in 1..r       { m |= 1 << (i * 8 + f); }
    for i in (f + 1)..7 { m |= 1 << (r * 8 + i); }
    for i in 1..f       { m |= 1 << (r * 8 + i); }
    m
}

fn mask_bishop(sq: usize) -> u64 {
    let mut m = 0u64;
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    for (i, j) in ((r+1)..7).zip((f+1)..7) { m |= 1 << (i * 8 + j); }
    for (i, j) in ((r+1)..7).zip((1..f).rev()) { m |= 1 << (i * 8 + j); }
    for (i, j) in ((1..r).rev()).zip((f+1)..7) { m |= 1 << (i * 8 + j); }
    for (i, j) in ((1..r).rev()).zip((1..f).rev()) { m |= 1 << (i * 8 + j); }
    m
}

// ============================================================
// Slow attack generators (include edges, for table building)
// ============================================================

fn rook_attacks_slow(sq: usize, occ: u64) -> u64 {
    let mut a = 0u64;
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    for i in (r+1)..8 { let b = 1u64 << (i*8+f); a |= b; if occ & b != 0 { break; } }
    for i in (0..r).rev() { let b = 1u64 << (i*8+f); a |= b; if occ & b != 0 { break; } }
    for i in (f+1)..8 { let b = 1u64 << (r*8+i); a |= b; if occ & b != 0 { break; } }
    for i in (0..f).rev() { let b = 1u64 << (r*8+i); a |= b; if occ & b != 0 { break; } }
    a
}

fn bishop_attacks_slow(sq: usize, occ: u64) -> u64 {
    let mut a = 0u64;
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    for (i,j) in ((r+1)..8).zip((f+1)..8) { let b = 1u64<<(i*8+j); a |= b; if occ&b!=0 { break; } }
    for (i,j) in ((r+1)..8).zip((0..f).rev()) { let b = 1u64<<(i*8+j); a |= b; if occ&b!=0 { break; } }
    for (i,j) in ((0..r).rev()).zip((f+1)..8) { let b = 1u64<<(i*8+j); a |= b; if occ&b!=0 { break; } }
    for (i,j) in ((0..r).rev()).zip((0..f).rev()) { let b = 1u64<<(i*8+j); a |= b; if occ&b!=0 { break; } }
    a
}

// ============================================================
// Formatting
// ============================================================

fn print_array(name: &str, arr: &[u8], type_name: &str) {
    print!("pub const {}: [{}; 64] = [\n    ", name, type_name);
    for (i, &val) in arr.iter().enumerate() {
        print!("{}, ", val);
        if (i + 1) % 8 == 0 && i != 63 { print!("\n    "); }
    }
    println!("\n];\n");
}

fn print_hex_array(name: &str, arr: &[u64]) {
    print!("pub const {}: [u64; 64] = [\n    ", name);
    for (i, &val) in arr.iter().enumerate() {
        print!("0x{:016x}, ", val);
        if (i + 1) % 4 == 0 && i != 63 { print!("\n    "); }
    }
    println!("\n];\n");
}