#[macro_use] extern crate criterion;

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::BuildHasher;
use std::hash::BuildHasherDefault;
use std::hash::Hash;
use std::hash::Hasher;

use criterion::black_box;
use criterion::Criterion;
use slimfilter::Builder;
use slimfilter::Filter;
use slimfilter::FilterBuilder;

fn bench(c: &mut Criterion) {
    let x = BuildHasherDefault::<DefaultHasher>::default();

    let n = 1000_000;

    let ks: BTreeSet<u64> = (0..n)
        .map(|i| {
            let hashed_key = {
                let mut hasher = x.build_hasher();
                i.hash(&mut hasher);
                hasher.finish()
            };
            // println!("{:064b}", hashed_key);
            hashed_key
        })
        .collect();

    let keys = ks.iter().copied().collect::<Vec<_>>();

    let mut b = Builder::new(8);

    b.add_keys(&keys);
    let f = b.build(8).unwrap();

    for k in keys.iter() {
        assert!(f.contains(k));
    }

    let mut hit = 0;
    let mut miss = 0;

    let mut absent_keys = Vec::with_capacity(n * 100);

    for i in n..n * 101 {
        let hashed_key = {
            let mut hasher = x.build_hasher();
            i.hash(&mut hasher);
            hasher.finish()
        };

        if ks.contains(&hashed_key) {
            continue;
        }

        absent_keys.push(hashed_key);
    }

    let mut iter_index = 0;
    c.bench_function("contains", |b| {
        b.iter(|| {
            let k = &absent_keys[iter_index % absent_keys.len()];
            if f.contains(k) {
                hit += 1;
            } else {
                miss += 1;
            }
            iter_index += 1;
        })
    });

    println!("hit: {}, miss: {}, 1/fp: {}", hit, miss, miss / (hit + 1))
}

criterion_group!(benches, bench);
criterion_main!(benches);
