use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode};
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

pub fn gen_multiple_chunks(c: &mut Criterion) {
    let mut group = c.benchmark_group("Gen Multiple Chunks");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(50);

    for i in [100, 200, 400, 800, 1600].iter() {
        group.bench_with_input(BenchmarkId::new("iproduct", i), i, |b, _i| {
            let state = create_state(1);
            b.iter(|| {
                for j in 0..*i {
                    let _ = gen_chunk(black_box(&state), black_box(ChunkPos::new(j, j)));
                }
            })
        });
        group.bench_with_input(BenchmarkId::new("for loops", i), i, |b, i| {
            let state = create_state(1);
            b.iter(|| {
                for j in 0..*i {
                    let _ = gen_chunk_fors(black_box(&state), black_box(ChunkPos::new(j, j)));
                }
            })
        });
    }
    group.finish()
}

criterion_group!(benches, gen_multiple_chunks);
criterion_main!(benches);
