//! Benchmarks for the Zircon wire-protocol framing layer.
//!
//! Two hot paths:
//!   encode — `RawFrame::encode(id, payload)` builds a framed `Bytes`
//!   parse  — `RawFrame::parse(&mut BytesMut)` decodes a frame
//!
//! Run with:
//!   cargo bench --package zircon-protocol

use bytes::BytesMut;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use zircon_protocol::RawFrame;

// ── Encode benchmarks ─────────────────────────────────────────────────────────

fn bench_encode(c: &mut Criterion) {
    let payloads: &[(&str, &[u8])] = &[
        ("empty",   &[]),
        ("16B",     &[0u8; 16]),
        ("256B",    &[0u8; 256]),
        ("1KiB",    &[0u8; 1024]),
    ];

    let mut group = c.benchmark_group("encode");
    for (label, payload) in payloads {
        group.bench_with_input(BenchmarkId::from_parameter(label), payload, |b, p| {
            b.iter(|| {
                let frame = RawFrame::encode(black_box(42), black_box(p));
                black_box(frame)
            });
        });
    }
    group.finish();
}

// ── Parse benchmarks ──────────────────────────────────────────────────────────

/// Build a BytesMut containing `n` copies of the same frame.
fn make_burst(id: u16, payload: &[u8], n: usize) -> BytesMut {
    let frame_bytes = RawFrame::encode(id, payload);
    let mut buf = BytesMut::with_capacity(frame_bytes.len() * n);
    for _ in 0..n {
        buf.extend_from_slice(&frame_bytes);
    }
    buf
}

fn bench_parse_burst(c: &mut Criterion) {
    const N: usize = 1_000;

    let payloads: &[(&str, &[u8])] = &[
        ("empty",   &[]),
        ("64B",     &[0u8; 64]),
        ("256B",    &[0u8; 256]),
        ("1KiB",    &[0u8; 1024]),
    ];

    let mut group = c.benchmark_group("parse_burst_1k");
    for (label, payload) in payloads {
        let data = make_burst(4, payload, N);
        group.bench_with_input(BenchmarkId::from_parameter(label), &data, |b, data| {
            b.iter_batched(
                || data.clone(),
                |mut buf| {
                    let mut count = 0u32;
                    while let Ok(Some(_frame)) = RawFrame::parse(&mut buf) {
                        count += 1;
                    }
                    black_box(count)
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Measure what fraction of time is parse-loop overhead vs. actual work.
fn bench_parse_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_single");

    for (label, payload) in [("empty", &[][..]), ("256B", &[0u8; 256][..])] {
        let frame_bytes = RawFrame::encode(4, payload);
        group.bench_with_input(BenchmarkId::from_parameter(label), &frame_bytes, |b, data| {
            b.iter_batched(
                || BytesMut::from(data.as_ref()),
                |mut buf| black_box(RawFrame::parse(&mut buf)),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_encode, bench_parse_burst, bench_parse_single);
criterion_main!(benches);
