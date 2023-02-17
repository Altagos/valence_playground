use std::num::NonZeroUsize;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode};
use lru::LruCache;
use noise::SuperSimplex;
use valence::{prelude::Chunk, view::ChunkPos};
use valence_playground::minecraft::world_gen::{gen_chunk, gen_chunk_fors, ChunkWorkerState};

fn create_state(seed: u32) -> ChunkWorkerState {
    let (finished_sender, _finished_receiver) = flume::unbounded();
    let (_pending_sender, pending_receiver) = flume::unbounded();
    let cache = LruCache::new(NonZeroUsize::new(1).unwrap());

    ChunkWorkerState {
        sender: finished_sender,
        receiver: pending_receiver,
        cache,
        density: SuperSimplex::new(seed),
        hilly: SuperSimplex::new(seed.wrapping_add(1)),
        stone: SuperSimplex::new(seed.wrapping_add(2)),
        gravel: SuperSimplex::new(seed.wrapping_add(3)),
        grass: SuperSimplex::new(seed.wrapping_add(4)),
    }
}

pub fn compare_gen_chunk(c: &mut Criterion) {
    let mut group = c.benchmark_group("Gen Chunk");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(50);

    for i in 0u32..5u32 {
        group.bench_with_input(BenchmarkId::new("iproduct", i), &i, |b, i| {
            let state = create_state(*i);
            b.iter(|| {
                gen_chunk(
                    black_box(&state),
                    black_box(&mut Chunk::new(16)),
                    black_box(ChunkPos::new(rand::random(), rand::random())),
                )
            })
        });
        group.bench_with_input(BenchmarkId::new("for loops", i), &i, |b, i| {
            let state = create_state(*i);
            b.iter(|| {
                gen_chunk_fors(
                    black_box(&state),
                    black_box(&mut Chunk::new(16)),
                    black_box(ChunkPos::new(rand::random(), rand::random())),
                )
            })
        });
    }
    group.finish()
}

criterion_group!(benches, compare_gen_chunk,);
criterion_main!(benches);
