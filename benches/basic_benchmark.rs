use container_image_dist_ref::RefStr;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
fn parse_test_corpus(c: &mut Criterion) {
    const VALID: &str = include_str!("../tests/fixtures/references/valid/inputs.txt");
    const INVALID: &str = include_str!("../tests/fixtures/references/invalid/inputs.txt");
    let inputs: Vec<&str> = VALID
        .split("\n")
        .chain(INVALID.split("\n"))
        .filter(|s| !s.is_empty())
        .collect();
    c.bench_function("entire_test_suite", |b| {
        b.iter(|| {
            for input in inputs.iter() {
                let _parse = RefStr::new(black_box(input));
            }
        })
    });
}

criterion_group!(benches, parse_test_corpus);
criterion_main!(benches);
