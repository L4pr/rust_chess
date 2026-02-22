use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use chess_engine::{Board, perft};


fn bench_pawn_generation(crit: &mut Criterion) {
    let mut board = Board::starting_position();

    crit.bench_function("generate_moves", |b| {
        b.iter(|| {
            // black_box prevents the compiler from optimizing the code away
            perft(black_box(&mut board), 5)
        })
    });
}

criterion_group!(benches, bench_pawn_generation);
criterion_main!(benches);