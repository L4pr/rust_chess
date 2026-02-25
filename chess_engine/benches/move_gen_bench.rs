use criterion::{criterion_group, criterion_main, Criterion, BatchSize};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use chess_engine::{Board, perft, ZobristKeys};
use chess_engine::engine::{alpha_beta, TranspositionTable};

fn bench_pawn_generation(crit: &mut Criterion) {
    let mut board = Board::starting_position();

    crit.bench_function("generate_moves", |b| {
        b.iter(|| {
            // black_box prevents the compiler from optimizing the code away
            perft(black_box(&mut board), 5)
        })
    });
}

fn bench_alpha_beta(crit: &mut Criterion) {
    let board = Board::starting_position();
    let zobrist = ZobristKeys::new();
    let abort = Arc::new(AtomicBool::new(false));

    // We use a small depth (like 5) so the benchmark runs in a reasonable time
    let target_depth = 5;

    crit.bench_function("alpha_beta_depth_5", |b| {
        b.iter_batched(
            || {
                // SETUP PHASE: Runs before every iteration (NOT timed).
                // Create a fresh 2MB TT and reset the node counter so it doesn't just read the cache.
                let tt = TranspositionTable::new(2);
                let nodes = 0u64;
                (tt, nodes)
            },
            |(mut tt, mut nodes)| {
                // BENCHMARK PHASE: This is the actual code being timed!
                alpha_beta(
                    black_box(&board),
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    target_depth,
                    0,
                    &abort,
                    &mut nodes,
                    &mut tt,
                    &zobrist,
                );

                // Return nodes so the compiler doesn't optimize the function call away entirely
                black_box(nodes)
            },
            BatchSize::SmallInput,
        )
    });
}

// Put BOTH functions inside the group here:
criterion_group!(benches, bench_alpha_beta, bench_pawn_generation);
criterion_main!(benches);