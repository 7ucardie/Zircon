//! Benchmarks for `PatchInfo` serialization (PList.Bin format).
//!
//! The encode and decode paths run on every `zircon-patchmgr build` invocation,
//! once per file in the client directory (typically several thousand files).
//!
//! Run with:
//!   cargo bench --package zircon-patchmgr

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use zircon_patchmgr::patch_info::{from_bytes, to_bytes, PatchInfo};

// ── Fixture builders ──────────────────────────────────────────────────────────

fn make_entries(n: usize) -> Vec<PatchInfo> {
    (0..n)
        .map(|i| PatchInfo {
            file_name: format!("Data/Maps/Map{i:04}.bin"),
            compressed_len: (i as i64 + 1) * 4096,
            checksum: {
                let mut cs = [0u8; 16];
                cs[0] = (i & 0xFF) as u8;
                cs[1] = ((i >> 8) & 0xFF) as u8;
                cs
            },
        })
        .collect()
}

// ── Encode benchmarks ─────────────────────────────────────────────────────────

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("plist_encode");
    for n in [100usize, 500, 2_000] {
        let entries = make_entries(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &entries, |b, entries| {
            b.iter(|| black_box(to_bytes(entries)));
        });
    }
    group.finish();
}

// ── Decode benchmarks ─────────────────────────────────────────────────────────

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("plist_decode");
    for n in [100usize, 500, 2_000] {
        let data = to_bytes(&make_entries(n));
        group.bench_with_input(BenchmarkId::from_parameter(n), &data, |b, data| {
            b.iter_batched(
                || data.clone(),
                |d| black_box(from_bytes(&d)),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// ── Round-trip benchmark ──────────────────────────────────────────────────────

fn bench_roundtrip(c: &mut Criterion) {
    let entries = make_entries(1_000);
    c.bench_function("plist_roundtrip_1k", |b| {
        b.iter(|| {
            let bytes = to_bytes(black_box(&entries));
            black_box(from_bytes(&bytes).unwrap())
        });
    });
}

criterion_group!(benches, bench_encode, bench_decode, bench_roundtrip);
criterion_main!(benches);
