use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use chess_engine::Board;
use chess_engine::generate_all_moves;
use chess_engine::Move;

fn bench_pawn_generation(crit: &mut Criterion) {
    let board = Board::starting_position();
    let mut moves = [Move(0); 256];

    crit.bench_function("generate_moves", |b| {
        b.iter(|| {
            // black_box prevents the compiler from optimizing the code away
            generate_all_moves(black_box(&board), black_box(&mut moves))
        })
    });
}

criterion_group!(benches, bench_pawn_generation);
criterion_main!(benches);