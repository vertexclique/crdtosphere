use crdtosphere::prelude::*;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::mem;

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Usage");

    // Benchmark memory footprint of different CRDTs
    group.bench_function("gcounter_size", |b| {
        b.iter(|| {
            let counter = GCounter::<DefaultConfig>::new(black_box(0));
            black_box(mem::size_of_val(&counter));
        });
    });

    group.bench_function("pncounter_size", |b| {
        b.iter(|| {
            let counter = PNCounter::<DefaultConfig>::new(black_box(0));
            black_box(mem::size_of_val(&counter));
        });
    });

    group.bench_function("lww_register_size", |b| {
        b.iter(|| {
            let register = LWWRegister::<u32, DefaultConfig>::new(black_box(0));
            black_box(mem::size_of_val(&register));
        });
    });

    group.bench_function("mv_register_size", |b| {
        b.iter(|| {
            let register = MVRegister::<u32, DefaultConfig>::new(black_box(0));
            black_box(mem::size_of_val(&register));
        });
    });

    group.bench_function("gset_size", |b| {
        b.iter(|| {
            let set = GSet::<u32, DefaultConfig>::new();
            black_box(mem::size_of_val(&set));
        });
    });

    group.bench_function("orset_size", |b| {
        b.iter(|| {
            let set = ORSet::<u32, DefaultConfig>::new(black_box(0));
            black_box(mem::size_of_val(&set));
        });
    });

    group.bench_function("lww_map_size", |b| {
        b.iter(|| {
            let map = LWWMap::<u32, u32, DefaultConfig>::new(black_box(0));
            black_box(mem::size_of_val(&map));
        });
    });

    group.finish();
}

fn benchmark_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Scaling");

    // Test how memory usage scales with data
    for size in [1, 4, 6, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("gcounter_with_data", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut counter = GCounter::<DefaultConfig>::new(0);
                    for _ in 0..size {
                        counter.increment(1).unwrap();
                    }
                    black_box(mem::size_of_val(&counter));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("gset_with_data", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut set = GSet::<u32, DefaultConfig>::new();
                    for i in 0..size {
                        set.insert(i as u32).unwrap();
                    }
                    black_box(mem::size_of_val(&set));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("lww_map_with_data", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
                    for i in 0..size {
                        map.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                            .unwrap();
                    }
                    black_box(mem::size_of_val(&map));
                });
            },
        );
    }

    group.finish();
}

fn benchmark_clone_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("Clone Performance");

    // Test cloning performance for different CRDT sizes
    for size in [1, 4, 6, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("gcounter_clone", size),
            size,
            |b, &size| {
                let mut counter = GCounter::<DefaultConfig>::new(0);
                for _ in 0..size {
                    counter.increment(1).unwrap();
                }
                b.iter(|| {
                    black_box(counter.clone());
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("gset_clone", size), size, |b, &size| {
            let mut set = GSet::<u32, DefaultConfig>::new();
            for i in 0..size {
                set.insert(i as u32).unwrap();
            }
            b.iter(|| {
                black_box(set.clone());
            });
        });

        group.bench_with_input(BenchmarkId::new("lww_map_clone", size), size, |b, &size| {
            let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
            for i in 0..size {
                map.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                    .unwrap();
            }
            b.iter(|| {
                black_box(map.clone());
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_memory_usage,
    benchmark_memory_scaling,
    benchmark_clone_performance
);
criterion_main!(benches);
