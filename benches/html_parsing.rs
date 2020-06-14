use cratesfyi::utils::extract_head_and_body;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

mod common;

pub fn head_and_body(c: &mut Criterion) {
    for (name, path) in common::get_files() {
        let html = std::fs::read(path).unwrap();

        let mut group = c.benchmark_group(&format!(
            "Parse '{}', {:.2}MiB",
            name,
            html.len() as f64 / 1024.0 / 1024.0
        ));
        group.throughput(Throughput::Bytes(html.len() as u64));
        group.sample_size(10);

        group.bench_function("head and body", |b| {
            b.iter(|| extract_head_and_body(black_box(&html)).unwrap())
        });

        group.bench_function("head and body w/ node fetch", |b| {
            b.iter(|| {
                let extracted = extract_head_and_body(black_box(&html)).unwrap();

                let _head = black_box(extracted.head_node());
                let _body = black_box(extracted.body_node());
            })
        });
    }
}

criterion_group!(html_parsing, head_and_body);
criterion_main!(html_parsing);
