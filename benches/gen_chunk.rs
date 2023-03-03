use criterion::{black_box, criterion_group, criterion_main, Criterion};
use noise::SuperSimplex;
use valence::view::ChunkPos;
use valence_playground::minecraft::world_gen::chunk_worker::{
    gen_chunk, gen_chunk_fors, ChunkWorkerState, TerrainSettings,
};

fn create_state(seed: u32) -> ChunkWorkerState {
    ChunkWorkerState {
        density: SuperSimplex::new(seed),
        hilly: SuperSimplex::new(seed.wrapping_add(1)),
        stone: SuperSimplex::new(seed.wrapping_add(2)),
        gravel: SuperSimplex::new(seed.wrapping_add(3)),
        grass: SuperSimplex::new(seed.wrapping_add(4)),
        settings: TerrainSettings::default(),
    }
}

pub fn bench_gen_chunk(c: &mut Criterion) {
    c.bench_function("gen_chunk (1, 1)", move |b| {
        let state = create_state(1);
        b.iter(|| gen_chunk(black_box(&state), black_box(ChunkPos::new(1, 1))));
    });
}

pub fn bench_gen_chunk_fors(c: &mut Criterion) {
    let state = create_state(1);

    c.bench_function("gen_chunk_fors (1, 1)", move |b| {
        b.iter(|| gen_chunk_fors(black_box(&state), black_box(ChunkPos::new(1, 1))));
    });
}

pub fn bench_gen_random_chunk(c: &mut Criterion) {
    let state = create_state(1);

    c.bench_function("gen_chunk random", move |b| {
        b.iter(|| {
            gen_chunk(
                black_box(&state),
                black_box(ChunkPos::new(rand::random(), rand::random())),
            )
        });
    });
}

pub fn bench_gen_random_chunk_fors(c: &mut Criterion) {
    let state = create_state(1);

    c.bench_function("gen_chunk_fors random", move |b| {
        b.iter(|| {
            gen_chunk_fors(
                black_box(&state),
                black_box(ChunkPos::new(rand::random(), rand::random())),
            )
        });
    });
}

criterion_group!(
    benches,
    bench_gen_chunk,
    bench_gen_chunk_fors,
    bench_gen_random_chunk,
    bench_gen_random_chunk_fors,
);
criterion_main!(benches);
