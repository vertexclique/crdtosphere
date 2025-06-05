#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

use crdtosphere::prelude::*;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

fn benchmark_gcounter(c: &mut Criterion) {
    let mut group = c.benchmark_group("GCounter");

    for size in [1, 4, 8, 16, 32].iter() {
        group.bench_with_input(BenchmarkId::new("increment", size), size, |b, &size| {
            let mut counter = GCounter::<DefaultConfig>::new(0);
            b.iter(|| {
                for _ in 0..size {
                    counter.increment(black_box(1)).unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("merge", size), size, |b, &size| {
            let mut counter1 = GCounter::<DefaultConfig>::new(0);
            let mut counter2 = GCounter::<DefaultConfig>::new(1);
            for _ in 0..size {
                counter1.increment(1).unwrap();
                counter2.increment(1).unwrap();
            }
            b.iter(|| {
                let mut c1 = counter1.clone();
                c1.merge(black_box(&counter2)).unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("value", size), size, |b, &size| {
            let mut counter = GCounter::<DefaultConfig>::new(0);
            for _ in 0..size {
                counter.increment(1).unwrap();
            }
            b.iter(|| {
                black_box(counter.value());
            });
        });
    }
    group.finish();
}

fn benchmark_pncounter(c: &mut Criterion) {
    let mut group = c.benchmark_group("PNCounter");

    for size in [1, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::new("increment", size), size, |b, &size| {
            let mut counter = PNCounter::<DefaultConfig>::new(0);
            b.iter(|| {
                for _ in 0..size {
                    counter.increment(black_box(1)).unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("decrement", size), size, |b, &size| {
            let mut counter = PNCounter::<DefaultConfig>::new(0);
            b.iter(|| {
                for _ in 0..size {
                    counter.decrement(black_box(1)).unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("merge", size), size, |b, &size| {
            let mut counter1 = PNCounter::<DefaultConfig>::new(0);
            let mut counter2 = PNCounter::<DefaultConfig>::new(1);
            for _ in 0..size {
                counter1.increment(1).unwrap();
                counter2.decrement(1).unwrap();
            }
            b.iter(|| {
                let mut c1 = counter1.clone();
                c1.merge(black_box(&counter2)).unwrap();
            });
        });
    }
    group.finish();
}

fn benchmark_lww_register(c: &mut Criterion) {
    let mut group = c.benchmark_group("LWWRegister");

    for size in [1, 10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("set", size), size, |b, &size| {
            let mut register = LWWRegister::<u32, DefaultConfig>::new(0);
            b.iter(|| {
                for i in 0..size {
                    register
                        .set(black_box(i as u32), (1000 + i) as u64)
                        .unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("get", size), size, |b, &size| {
            let mut register = LWWRegister::<u32, DefaultConfig>::new(0);
            for i in 0..size {
                register.set(i as u32, (1000 + i) as u64).unwrap();
            }
            b.iter(|| {
                black_box(register.get());
            });
        });

        group.bench_with_input(BenchmarkId::new("merge", size), size, |b, &size| {
            let mut register1 = LWWRegister::<u32, DefaultConfig>::new(0);
            let mut register2 = LWWRegister::<u32, DefaultConfig>::new(1);
            for i in 0..size {
                register1.set(i as u32, (1000 + i) as u64).unwrap();
                register2.set((i + 1000) as u32, (2000 + i) as u64).unwrap();
            }
            b.iter(|| {
                let mut r1 = register1.clone();
                r1.merge(black_box(&register2)).unwrap();
            });
        });
    }
    group.finish();
}

fn benchmark_mv_register(c: &mut Criterion) {
    let mut group = c.benchmark_group("MVRegister");

    for size in [1, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::new("set", size), size, |b, &size| {
            let mut register = MVRegister::<u32, DefaultConfig>::new(0);
            b.iter(|| {
                for i in 0..size {
                    register
                        .set(black_box(i as u32), (1000 + i) as u64)
                        .unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("values_array", size), size, |b, &size| {
            let mut register = MVRegister::<u32, DefaultConfig>::new(0);
            for i in 0..size {
                register.set(i as u32, (1000 + i) as u64).unwrap();
            }
            b.iter(|| {
                black_box(register.values_array());
            });
        });
    }
    group.finish();
}

fn benchmark_gset(c: &mut Criterion) {
    let mut group = c.benchmark_group("GSet");

    for size in [1, 4, 6, 8].iter() {
        group.bench_with_input(BenchmarkId::new("insert", size), size, |b, &size| {
            let mut set = GSet::<u32, DefaultConfig>::new();
            b.iter(|| {
                for i in 0..size {
                    set.insert(black_box(i as u32)).unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("contains", size), size, |b, &size| {
            let mut set = GSet::<u32, DefaultConfig>::new();
            for i in 0..size {
                set.insert(i as u32).unwrap();
            }
            b.iter(|| {
                for i in 0..size {
                    black_box(set.contains(&(i as u32)));
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("merge", size), size, |b, &size| {
            let mut set1 = GSet::<u32, DefaultConfig>::new();
            let mut set2 = GSet::<u32, DefaultConfig>::new();
            // Use half the size for each set to avoid overflow when merging
            let half_size = size / 2;
            for i in 0..half_size {
                set1.insert(i as u32).unwrap();
                set2.insert((i + 1000) as u32).unwrap();
            }
            b.iter(|| {
                let mut s1 = set1.clone();
                s1.merge(black_box(&set2)).unwrap();
            });
        });
    }
    group.finish();
}

fn benchmark_orset(c: &mut Criterion) {
    let mut group = c.benchmark_group("ORSet");

    for size in [1, 4, 6, 8].iter() {
        group.bench_with_input(BenchmarkId::new("add", size), size, |b, &size| {
            let mut set = ORSet::<u32, DefaultConfig>::new(0);
            b.iter(|| {
                for i in 0..size {
                    set.add(black_box(i as u32), (1000 + i) as u64).unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("remove", size), size, |b, &size| {
            let mut set = ORSet::<u32, DefaultConfig>::new(0);
            for i in 0..size {
                set.add(i as u32, (1000 + i) as u64).unwrap();
            }
            b.iter(|| {
                for i in 0..size {
                    set.remove(&(i as u32), (2000 + i) as u64).unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("contains", size), size, |b, &size| {
            let mut set = ORSet::<u32, DefaultConfig>::new(0);
            for i in 0..size {
                set.add(i as u32, (1000 + i) as u64).unwrap();
            }
            b.iter(|| {
                for i in 0..size {
                    black_box(set.contains(&(i as u32)));
                }
            });
        });
    }
    group.finish();
}

fn benchmark_lww_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("LWWMap");

    for size in [1, 4, 6, 8].iter() {
        group.bench_with_input(BenchmarkId::new("insert", size), size, |b, &size| {
            let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
            b.iter(|| {
                for i in 0..size {
                    map.insert(
                        black_box(i as u32),
                        black_box((i * 2) as u32),
                        (1000 + i) as u64,
                    )
                    .unwrap();
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("get", size), size, |b, &size| {
            let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
            for i in 0..size {
                map.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                    .unwrap();
            }
            b.iter(|| {
                for i in 0..size {
                    black_box(map.get(&(i as u32)));
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("contains_key", size), size, |b, &size| {
            let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
            for i in 0..size {
                map.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                    .unwrap();
            }
            b.iter(|| {
                for i in 0..size {
                    black_box(map.contains_key(&(i as u32)));
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("remove", size), size, |b, &size| {
            b.iter(|| {
                let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
                // Fill the map first
                for i in 0..size {
                    map.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                        .unwrap();
                }
                // Then remove all entries
                for i in 0..size {
                    black_box(map.remove(&(i as u32)));
                }
            });
        });

        group.bench_with_input(
            BenchmarkId::new("remove_and_reinsert", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut map = LWWMap::<u32, u32, DefaultConfig>::new(0);
                    // Fill the map
                    for i in 0..size {
                        map.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                            .unwrap();
                    }
                    // Remove half the entries
                    for i in 0..(size / 2) {
                        black_box(map.remove(&(i as u32)));
                    }
                    // Reinsert them with new values
                    for i in 0..(size / 2) {
                        map.insert(i as u32, (i * 3) as u32, (2000 + i) as u64)
                            .unwrap();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("merge_with_removes", size),
            size,
            |b, &size| {
                let mut map1 = LWWMap::<u32, u32, DefaultConfig>::new(0);
                let mut map2 = LWWMap::<u32, u32, DefaultConfig>::new(1);

                // Setup: fill both maps
                for i in 0..size {
                    map1.insert(i as u32, (i * 2) as u32, (1000 + i) as u64)
                        .unwrap();
                    map2.insert(i as u32, (i * 3) as u32, (1100 + i) as u64)
                        .unwrap();
                }

                // Remove some entries from map1
                for i in 0..(size / 2) {
                    map1.remove(&(i as u32));
                }

                b.iter(|| {
                    let mut m1 = map1.clone();
                    m1.merge(black_box(&map2)).unwrap();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_gcounter,
    benchmark_pncounter,
    benchmark_lww_register,
    benchmark_mv_register,
    benchmark_gset,
    benchmark_orset,
    benchmark_lww_map
);
criterion_main!(benches);
