use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aoc::slow;
// use aoc::fast;


fn criterion_benchmark(c: &mut Criterion) {
    
    c.bench_function("slow run", |b| b.iter(|| black_box(slow::run_calc())));
    
    // c.bench_function("fast run", |b| b.iter(|| black_box(fast::run_calc())));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);