use cratesfyi::storage::{compress, decompress};
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

mod common;

pub fn compression_benches(c: &mut Criterion) {
    for (name, path) in common::get_files() {
        // this isn't a great benchmark because it only tests on one file
        // ideally we would build a whole crate and compress each file, taking the average
        let html = std::fs::read_to_string(path).unwrap();
        let html_slice = html.as_bytes();

        let mut group = c.benchmark_group(&format!(
            "Compress and Decompress '{}', {:.2}MiB",
            name,
            html.len() as f64 / 1024.0 / 1024.0
        ));
        group.throughput(Throughput::Bytes(html.len() as u64));
        group.sample_size(10);

        group.bench_function("compress", |b| b.iter(|| compress(black_box(html_slice))));

        let (compressed, alg) = compress(html_slice).unwrap();
        group.bench_function("decompress", |b| {
            b.iter(|| decompress(black_box(compressed.as_slice()), alg))
        });
    }
}

criterion_group!(compression, compression_benches);
criterion_main!(compression);
