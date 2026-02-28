use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use chess_engine::{Board, perft, init_magic_bitboards, init_zobrist};
use chess_engine::engine::bench_search;

fn bench_movegen(crit: &mut Criterion) {
    init_magic_bitboards();
    init_zobrist();

    let mut start = Board::starting_position();
    let mut kiwipete = Board::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
    );

    crit.bench_function("perft_5_startpos", |b| {
        b.iter(|| {
            perft(black_box(&mut start), 5)
        })
    });

    crit.bench_function("perft_4_kiwipete", |b| {
        b.iter(|| {
            perft(black_box(&mut kiwipete), 4)
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

    let kiwipete = Board::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"
    );
    crit.bench_function("alpha_beta_depth_5_kiwipete", |b| {
        b.iter(|| {
            let (score, nodes) = bench_search(black_box(&kiwipete), 5, &abort);
            black_box((score, nodes))
        })
    });
}

criterion_group!(benches, bench_alpha_beta, bench_movegen);
criterion_main!(benches);