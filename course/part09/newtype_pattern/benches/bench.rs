use criterion::{criterion_group, criterion_main, Criterion};
use std::cmp::min;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::hint::black_box;

fn bench_options(c: &mut Criterion) {
    let strings = (0..10_000)
        .map(|_| {
            let len = 100 + (rand::random::<u8>() as usize) * 20;

            let mut s = String::with_capacity(len);

            for _ in 0..len {
                let c = b'a' + (rand::random::<u8>() % 26);
                s.push(c as char);
            }

            s
        })
        .collect::<Vec<String>>();

    let string_refs = strings.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let containered_refs = strings
        .iter()
        .map(|s| Container(s.as_str()))
        .collect::<Vec<_>>();
    let containered_strings = strings
        .iter()
        .map(|s| Container(s.clone()))
        .collect::<Vec<_>>();

    assert_eq!(
        collate(&strings),
        collate(&string_refs),
        "should produce same result for both types"
    );

    assert_eq!(
        collate(&string_refs),
        collate(&containered_refs),
        "should produce same result for both types"
    );

    assert_eq!(
        collate(&containered_refs),
        collate(&containered_strings),
        "should produce same result for both types"
    );

    c.bench_function("naive string refs", |b| {
        b.iter(|| collate(black_box(&string_refs)))
    });
    c.bench_function("naive strings", |b| b.iter(|| collate(black_box(&strings))));
    c.bench_function("containered string refs", |b| {
        b.iter(|| collate(black_box(&containered_refs)))
    });
    c.bench_function("containered strings", |b| {
        b.iter(|| collate(black_box(&containered_strings)))
    });
}

fn collate<T: Hash + Eq>(input: &[T]) -> usize {
    HashSet::<&T>::from_iter(input.iter()).len()
}

#[derive(Debug, Eq, PartialEq)]
struct Container<T>(T);

impl<T> Hash for Container<T>
where
    T: AsRef<str>,
{
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let b = self.0.as_ref().as_bytes();
        let len = min(256, b.len());
        b[0..len].hash(state);
    }
}

criterion_group!(benches, bench_options);
criterion_main!(benches);
