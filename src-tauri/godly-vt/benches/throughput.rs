use criterion::{criterion_group, criterion_main, Criterion, Throughput, BenchmarkId};

/// Generate 1MB of pure ASCII printable text (no control chars or escape sequences).
fn gen_ascii_1mb() -> Vec<u8> {
    let line = b"The quick brown fox jumps over the lazy dog. 0123456789 ABCDEFGHIJKLMNOP\n";
    let mut data = Vec::with_capacity(1024 * 1024);
    while data.len() < 1024 * 1024 {
        data.extend_from_slice(line);
    }
    data.truncate(1024 * 1024);
    data
}

/// Generate 1MB of mixed UTF-8 text with CJK and emoji.
fn gen_unicode_1mb() -> Vec<u8> {
    let line = "The quick brown fox \u{4e16}\u{754c}\u{3053}\u{3093}\u{306b}\u{3061}\u{306f} \u{1f680}\u{1f30d}\u{2764}\n";
    let line_bytes = line.as_bytes();
    let mut data = Vec::with_capacity(1024 * 1024);
    while data.len() < 1024 * 1024 {
        data.extend_from_slice(line_bytes);
    }
    data.truncate(1024 * 1024);
    data
}

/// Generate data with heavy CSI sequences: 10K SGR color changes interleaved with text.
fn gen_csi_heavy() -> Vec<u8> {
    let mut data = Vec::with_capacity(512 * 1024);
    for i in 0..10_000 {
        // SGR with 256-color foreground
        let color = (i % 256) as u8;
        data.extend_from_slice(format!("\x1b[38;5;{}m", color).as_bytes());
        data.extend_from_slice(b"Hello World! ");
    }
    // Reset at end
    data.extend_from_slice(b"\x1b[0m");
    data
}

/// Simulated `cargo build` output: ANSI colors, file paths, warnings.
fn gen_mixed_realistic() -> Vec<u8> {
    let lines = [
        "\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m godly-vt v0.1.0 (C:\\Users\\dev\\godly-terminal\\src-tauri\\godly-vt)\n",
        "\x1b[0m\x1b[1m\x1b[33mwarning\x1b[0m: unused variable: `threshold`\n",
        " \x1b[0m\x1b[1m\x1b[34m-->\x1b[0m godly-vt\\src\\simd\\sse2.rs:20:13\n",
        "   \x1b[0m\x1b[1m\x1b[34m|\x1b[0m\n",
        "\x1b[1m\x1b[34m20\x1b[0m \x1b[0m\x1b[1m\x1b[34m|\x1b[0m         let threshold = _mm_set1_epi8(0x20);\n",
        "   \x1b[0m\x1b[1m\x1b[34m|\x1b[0m             \x1b[0m\x1b[1m\x1b[33m^^^^^^^^^\x1b[0m \x1b[0m\x1b[1m\x1b[33mhelp: prefix with underscore\x1b[0m\n",
        "\x1b[0m\x1b[1m\x1b[32m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 2.34s\n",
    ];
    let mut data = Vec::with_capacity(1024 * 1024);
    while data.len() < 1024 * 1024 {
        for line in &lines {
            data.extend_from_slice(line.as_bytes());
        }
    }
    data.truncate(1024 * 1024);
    data
}

fn bench_throughput(c: &mut Criterion) {
    let ascii_data = gen_ascii_1mb();
    let unicode_data = gen_unicode_1mb();
    let csi_data = gen_csi_heavy();
    let realistic_data = gen_mixed_realistic();

    let mut group = c.benchmark_group("parser_throughput");

    // ASCII 1MB
    group.throughput(Throughput::Bytes(ascii_data.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("ascii_1mb", ascii_data.len()),
        &ascii_data,
        |b, data| {
            b.iter(|| {
                let mut parser = godly_vt::Parser::new(24, 80, 0);
                parser.process(data);
            });
        },
    );

    // Unicode 1MB
    group.throughput(Throughput::Bytes(unicode_data.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("unicode_1mb", unicode_data.len()),
        &unicode_data,
        |b, data| {
            b.iter(|| {
                let mut parser = godly_vt::Parser::new(24, 80, 0);
                parser.process(data);
            });
        },
    );

    // CSI-heavy (10K SGR sequences)
    group.throughput(Throughput::Bytes(csi_data.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("csi_heavy", csi_data.len()),
        &csi_data,
        |b, data| {
            b.iter(|| {
                let mut parser = godly_vt::Parser::new(24, 80, 0);
                parser.process(data);
            });
        },
    );

    // Mixed realistic (cargo build output)
    group.throughput(Throughput::Bytes(realistic_data.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("mixed_realistic", realistic_data.len()),
        &realistic_data,
        |b, data| {
            b.iter(|| {
                let mut parser = godly_vt::Parser::new(24, 80, 0);
                parser.process(data);
            });
        },
    );

    group.finish();
}

fn bench_simd_scanner(c: &mut Criterion) {
    let ascii_data = gen_ascii_1mb();
    // Data with no control chars at all (remove newlines)
    let pure_printable: Vec<u8> = ascii_data.iter().copied().filter(|&b| b >= 0x20 && b != 0x7F).collect();

    let mut group = c.benchmark_group("simd_scanner");

    group.throughput(Throughput::Bytes(pure_printable.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("scan_for_control_no_match", pure_printable.len()),
        &pure_printable,
        |b, data| {
            b.iter(|| {
                godly_vt::simd::scan_for_control(data)
            });
        },
    );

    group.throughput(Throughput::Bytes(ascii_data.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("scan_for_control_with_newlines", ascii_data.len()),
        &ascii_data,
        |b, data| {
            b.iter(|| {
                godly_vt::simd::scan_for_control(data)
            });
        },
    );

    group.throughput(Throughput::Bytes(pure_printable.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_all_ascii_pure", pure_printable.len()),
        &pure_printable,
        |b, data| {
            b.iter(|| {
                godly_vt::simd::is_all_ascii(data)
            });
        },
    );

    group.finish();
}

criterion_group!(benches, bench_throughput, bench_simd_scanner);
criterion_main!(benches);
