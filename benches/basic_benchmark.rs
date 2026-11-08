use container_image_dist_ref::ImgRef;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
fn parse_test_corpus(c: &mut Criterion) {
    const VALID: &str = include_str!("../tests/fixtures/references/valid/inputs.txt");
    const INVALID: &str = include_str!("../tests/fixtures/references/invalid/inputs.txt");
    let inputs: Vec<&str> = VALID
        .split('\n')
        .chain(INVALID.split('\n'))
        .filter(|s| !s.is_empty())
        .collect();
    c.bench_function("entire_test_suite", |b| {
        b.iter(|| {
            for input in inputs.iter() {
                let _parse = ImgRef::new(black_box(input));
            }
        })
    });
}

fn parse_long_paths(c: &mut Criterion) {
    // A path at the maximum length, both as one component and split into many.
    let single = "a".repeat(255);
    let mut many = String::with_capacity(255);
    for i in 0..128 {
        if i > 0 {
            many.push('/');
        }
        many.push('a');
    }
    c.bench_function("long_path_single_component", |b| {
        b.iter(|| ImgRef::new(black_box(&single)))
    });
    c.bench_function("long_path_128_components", |b| {
        b.iter(|| ImgRef::new(black_box(&many)))
    });
}

criterion_group!(benches, parse_test_corpus, parse_long_paths);
criterion_main!(benches);
