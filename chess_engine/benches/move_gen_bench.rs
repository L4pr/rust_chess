use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use chess_engine::{Board, perft, init_magic_bitboards, init_zobrist};
use chess_engine::engine::bench_search;
use chess_engine::move_generation::tests::perft2;

fn bench_pawn_generation(crit: &mut Criterion) {
    init_magic_bitboards();
    init_zobrist();

    let mut board = Board::starting_position();

    crit.bench_function("generate_moves", |b| {
        b.iter(|| {
            perft(black_box(&mut board), 5)
        })
    });

    crit.bench_function("generate_captures", |b| {
        b.iter(|| {
            perft2(black_box(&mut board), 5)
        })
    });
}

fn bench_alpha_beta(crit: &mut Criterion) {
    init_magic_bitboards();
    init_zobrist();

    let board = Board::starting_position();
    let abort = Arc::new(AtomicBool::new(false));

    crit.bench_function("alpha_beta_depth_5", |b| {
        b.iter(|| {
            let (score, nodes) = bench_search(black_box(&board), 5, &abort);
            black_box((score, nodes))
        })
    });
}

criterion_group!(benches, bench_alpha_beta, bench_pawn_generation);
criterion_main!(benches);