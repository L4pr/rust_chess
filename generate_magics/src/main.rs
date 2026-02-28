use std::io;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {
    // 1. Setup the Ctrl+C interrupter
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\n\n🛑 Ctrl+C received! Stopping the grind and preparing your numbers...\n");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let mut rook_magics = [0u64; 64];
    let mut bishop_magics = [0u64; 64];
    let mut rook_shifts = [0u8; 64];
    let mut bishop_shifts = [0u8; 64];
    let mut seed = 123456789u64;

    // --- HELPER CLOSURES FOR PRECOMPUTATION ---
    // These calculate the mask, subsets, and exact attacks ONCE per square.
    let get_rook_data = |sq: usize| -> (u64, Vec<u64>, Vec<u64>) {
        let mask = mask_rook_attacks(sq);
        let mut subsets = Vec::new();
        let mut subset = 0u64;
        loop {
            subsets.push(subset);
            subset = subset.wrapping_sub(mask) & mask;
            if subset == 0 { break; }
        }
        let attacks = subsets.iter().map(|&s| generate_rook_attacks_on_the_fly(sq, s)).collect();
        (mask, subsets, attacks)
    };

    let get_bishop_data = |sq: usize| -> (u64, Vec<u64>, Vec<u64>) {
        let mask = mask_bishop_attacks(sq);
        let mut subsets = Vec::new();
        let mut subset = 0u64;
        loop {
            subsets.push(subset);
            subset = subset.wrapping_sub(mask) & mask;
            if subset == 0 { break; }
        }
        let attacks = subsets.iter().map(|&s| generate_bishop_attacks_on_the_fly(sq, s)).collect();
        (mask, subsets, attacks)
    };

    println!("Phase 1: Finding baseline safe magics...");
    for sq in 0..64 {
        let (r_mask, r_subsets, r_attacks) = get_rook_data(sq);
        let (b_mask, b_subsets, b_attacks) = get_bishop_data(sq);

        rook_shifts[sq] = 64 - r_mask.count_ones() as u8;
        rook_magics[sq] = loop {
            // Keep trying in batches until we get one
            if let Some(m) = try_magic(r_mask, &r_subsets, &r_attacks, 64 - rook_shifts[sq], &mut seed, 5_000_000) {
                break m;
            }
        };

        bishop_shifts[sq] = 64 - b_mask.count_ones() as u8;
        bishop_magics[sq] = loop {
            if let Some(m) = try_magic(b_mask, &b_subsets, &b_attacks, 64 - bishop_shifts[sq], &mut seed, 5_000_000) {
                break m;
            }
        };

        println!("  -> Baseline found for square {}", sq);
    }

    println!("Phase 2: Entering the Infinite Grind! 🚀");
    println!("Press Ctrl+C at any time to stop and print the arrays.\n");

    let mut sq = 0;

    // Helper closure to calculate the current size of the tables in Kilobytes
    let get_kb = |r_shifts: &[u8; 64], b_shifts: &[u8; 64]| {
        let mut size = 0;
        for &s in r_shifts.iter() { size += 1 << (64 - s); }
        for &s in b_shifts.iter() { size += 1 << (64 - s); }
        (size * 8) / 1024 // 8 bytes per u64, divided by 1024 for KB
    };

    while running.load(Ordering::SeqCst) {
        // --- THE PROGRESS UPDATE ---
        print!("\r⏳ Grinding Square {:02}/63 | Current Table Size: {} KB   ", sq, get_kb(&rook_shifts, &bishop_shifts));
        io::stdout().flush().unwrap(); // Force the terminal to draw the line immediately

        let (r_mask, r_subsets, r_attacks) = get_rook_data(sq);
        let (b_mask, b_subsets, b_attacks) = get_bishop_data(sq);

        // Try to compress Rook table by 1 more bit
        let target_r_bits = 64 - rook_shifts[sq] - 1;
        if target_r_bits >= 5 { // 5 is a reasonable absolute minimum for rooks
            if let Some(magic) = try_magic(r_mask, &r_subsets, &r_attacks, target_r_bits, &mut seed, 4_000_000) {
                rook_magics[sq] = magic;
                rook_shifts[sq] += 1;
                // \n at the start ensures we don't overwrite our progress bar!
                println!("\n  [+] Rook on sq {} compressed to {} bits! 📉", sq, target_r_bits);
            }
        }

        // Try to compress Bishop table by 1 more bit
        let target_b_bits = 64 - bishop_shifts[sq] - 1;
        if target_b_bits >= 5 {
            if let Some(magic) = try_magic(b_mask, &b_subsets, &b_attacks, target_b_bits, &mut seed, 4_000_000) {
                bishop_magics[sq] = magic;
                bishop_shifts[sq] += 1;
                println!("\n  [+] Bishop on sq {} compressed to {} bits! 📉", sq, target_b_bits);
            }
        }

        sq = (sq + 1) % 64;
    }

    // 3. Print the final results once the loop breaks
    println!("\n// === OPTIMIZED MAGICS FOR src/magic/constants.rs ===\n");
    print_array("ROOK_SHIFTS", &rook_shifts, "u8");
    print_array("BISHOP_SHIFTS", &bishop_shifts, "u8");
    print_hex_array("ROOK_MAGICS", &rook_magics);
    print_hex_array("BISHOP_MAGICS", &bishop_magics);
}

// --- The Core Trial Function ---

fn try_magic(
    mask: u64,
    subsets: &[u64],
    actual_attacks: &[u64],
    target_bits: u8,
    seed: &mut u64,
    max_attempts: u32,
) -> Option<u64> {
    let shift = 64 - target_bits;
    let table_size = 1usize << target_bits;

    // Allocate ONCE outside the attempt loop
    let mut attack_table = vec![0u64; table_size];
    let mut epoch_table = vec![0u32; table_size]; // Tracks the attempt number

    for attempt in 1..=max_attempts {
        // RNG
        *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17; let r1 = *seed;
        *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17; let r2 = *seed;
        *seed ^= *seed << 13; *seed ^= *seed >> 7; *seed ^= *seed << 17; let r3 = *seed;
        let magic = r1 & r2 & r3;

        // Quick entropy check
        if (mask.wrapping_mul(magic) & 0xFF00_0000_0000_0000).count_ones() < 6 { continue; }

        let mut fail = false;

        for i in 0..subsets.len() {
            let index = (subsets[i].wrapping_mul(magic) >> shift) as usize;

            if epoch_table[index] != attempt {
                // First time hitting this index in the CURRENT attempt. Claim it.
                epoch_table[index] = attempt;
                attack_table[index] = actual_attacks[i];
            } else if attack_table[index] != actual_attacks[i] {
                // We've hit this index this attempt, and the attacks don't match. Collision!
                fail = true;
                break;
            }
        }

        if !fail { return Some(magic); }
    }
    None
}

// --- Raycasting & Printing Helpers (Same as before) ---
fn mask_rook_attacks(sq: usize) -> u64 {
    let mut mask = 0u64; let tr = (sq / 8) as i32; let tf = (sq % 8) as i32;
    for r in (tr + 1)..7 { mask |= 1 << (r * 8 + tf); } for r in 1..tr { mask |= 1 << (r * 8 + tf); }
    for f in (tf + 1)..7 { mask |= 1 << (tr * 8 + f); } for f in 1..tf { mask |= 1 << (tr * 8 + f); }
    mask
}
fn mask_bishop_attacks(sq: usize) -> u64 {
    let mut mask = 0u64; let tr = (sq / 8) as i32; let tf = (sq % 8) as i32;
    for (r, f) in ((tr + 1)..7).zip((tf + 1)..7) { mask |= 1 << (r * 8 + f); }
    for (r, f) in ((tr + 1)..7).zip((1..tf).rev()) { mask |= 1 << (r * 8 + f); }
    for (r, f) in ((1..tr).rev()).zip((tf + 1)..7) { mask |= 1 << (r * 8 + f); }
    for (r, f) in ((1..tr).rev()).zip((1..tf).rev()) { mask |= 1 << (r * 8 + f); }
    mask
}
fn generate_rook_attacks_on_the_fly(sq: usize, blockers: u64) -> u64 {
    let mut attacks = 0u64; let tr = (sq / 8) as i32; let tf = (sq % 8) as i32;
    for r in (tr + 1)..8 { attacks |= 1 << (r * 8 + tf); if blockers & (1 << (r * 8 + tf)) != 0 { break; } }
    for r in (0..tr).rev() { attacks |= 1 << (r * 8 + tf); if blockers & (1 << (r * 8 + tf)) != 0 { break; } }
    for f in (tf + 1)..8 { attacks |= 1 << (tr * 8 + f); if blockers & (1 << (tr * 8 + f)) != 0 { break; } }
    for f in (0..tf).rev() { attacks |= 1 << (tr * 8 + f); if blockers & (1 << (tr * 8 + f)) != 0 { break; } }
    attacks
}
fn generate_bishop_attacks_on_the_fly(sq: usize, blockers: u64) -> u64 {
    let mut attacks = 0u64; let tr = (sq / 8) as i32; let tf = (sq % 8) as i32;
    for (r, f) in ((tr + 1)..8).zip((tf + 1)..8) { attacks |= 1 << (r * 8 + f); if blockers & (1 << (r * 8 + f)) != 0 { break; } }
    for (r, f) in ((tr + 1)..8).zip((0..tf).rev()) { attacks |= 1 << (r * 8 + f); if blockers & (1 << (r * 8 + f)) != 0 { break; } }
    for (r, f) in ((0..tr).rev()).zip((tf + 1)..8) { attacks |= 1 << (r * 8 + f); if blockers & (1 << (r * 8 + f)) != 0 { break; } }
    for (r, f) in ((0..tr).rev()).zip((0..tf).rev()) { attacks |= 1 << (r * 8 + f); if blockers & (1 << (r * 8 + f)) != 0 { break; } }
    attacks
}
fn print_array(name: &str, arr: &[u8], type_name: &str) {
    print!("pub const {}: [{}; 64] = [\n    ", name, type_name);
    for (i, &val) in arr.iter().enumerate() { print!("{}, ", val); if (i + 1) % 8 == 0 && i != 63 { print!("\n    "); } }
    println!("\n];\n");
}
fn print_hex_array(name: &str, arr: &[u64]) {
    print!("pub const {}: [u64; 64] = [\n    ", name);
    for (i, &val) in arr.iter().enumerate() { print!("0x{:016x}, ", val); if (i + 1) % 4 == 0 && i != 63 { print!("\n    "); } }
    println!("\n];\n");
}