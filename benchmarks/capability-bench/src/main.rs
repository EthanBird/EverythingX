#![forbid(unsafe_code)]

mod generated_adapters;

use std::hint::black_box;
use std::io::Cursor;
use std::time::{Duration, Instant};

use everythingx_kernel::Kernel;

const SMALL_PAYLOAD: usize = 16 * 1024;
const LARGE_PAYLOAD: usize = 4 * 1024 * 1024;
const WARMUPS: usize = 2;
const SMALL_SAMPLES: usize = 11;
const LARGE_SAMPLES: usize = 7;

const W64_RIFF: [u8; 16] = [0x72,0x69,0x66,0x66,0x2e,0x91,0xcf,0x11,0xa5,0xd6,0x28,0xdb,0x04,0xc1,0x00,0x00];
const W64_WAVE: [u8; 16] = [0x77,0x61,0x76,0x65,0xf3,0xac,0xd3,0x11,0x8c,0xd1,0x00,0xc0,0x4f,0x8e,0xdb,0x8a];
const W64_FMT: [u8; 16] = [0x66,0x6d,0x74,0x20,0xf3,0xac,0xd3,0x11,0x8c,0xd1,0x00,0xc0,0x4f,0x8e,0xdb,0x8a];
const W64_DATA: [u8; 16] = [0x64,0x61,0x74,0x61,0xf3,0xac,0xd3,0x11,0x8c,0xd1,0x00,0xc0,0x4f,0x8e,0xdb,0x8a];

#[derive(Debug)]
struct SampleSummary {
    p50_ns: u128,
    p95_ns: u128,
    output_bytes: u64,
    peak_memory_bytes: u64,
}

fn patterned_pcm(bytes: usize, big_endian: bool) -> Vec<u8> {
    let aligned = bytes.max(4) / 4 * 4;
    let mut out = Vec::with_capacity(aligned);
    for frame in 0..(aligned / 4) {
        let left = (frame as i16).wrapping_mul(31);
        let right = (frame as i16).wrapping_mul(-17);
        if big_endian {
            out.extend_from_slice(&left.to_be_bytes());
            out.extend_from_slice(&right.to_be_bytes());
        } else {
            out.extend_from_slice(&left.to_le_bytes());
            out.extend_from_slice(&right.to_le_bytes());
        }
    }
    out
}

fn wave_fmt() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&48_000u32.to_le_bytes());
    out.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out
}

fn riff_fixture(kind: &[u8; 4], payload_bytes: usize, bext: bool) -> Vec<u8> {
    let audio = patterned_pcm(payload_bytes, false);
    let fmt = wave_fmt();
    let extended = kind == b"RF64" || kind == b"BW64";
    let extra = if bext { 8 + 602 } else if extended { 8 + 28 } else { 0 };
    let total = 12 + 8 + fmt.len() + extra + 8 + audio.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(kind);
    out.extend_from_slice(&(if extended { u32::MAX } else { (total - 8) as u32 }).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    if extended {
        out.extend_from_slice(b"ds64");
        out.extend_from_slice(&28u32.to_le_bytes());
        out.extend_from_slice(&((total - 8) as u64).to_le_bytes());
        out.extend_from_slice(&(audio.len() as u64).to_le_bytes());
        out.extend_from_slice(&((audio.len() / 4) as u64).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
    }
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    out.extend_from_slice(&fmt);
    if bext {
        let mut payload = vec![0u8; 602];
        payload[346..348].copy_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(b"bext");
        out.extend_from_slice(&602u32.to_le_bytes());
        out.extend_from_slice(&payload);
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(if extended { u32::MAX } else { audio.len() as u32 }).to_le_bytes());
    out.extend_from_slice(&audio);
    out
}

fn caf_fixture(payload_bytes: usize) -> Vec<u8> {
    let audio = patterned_pcm(payload_bytes, false);
    let mut out = b"caff\0\x01\0\0desc".to_vec();
    out.extend_from_slice(&32i64.to_be_bytes());
    out.extend_from_slice(&48_000f64.to_bits().to_be_bytes());
    out.extend_from_slice(b"lpcm");
    out.extend_from_slice(&(4u32 | 8u32).to_be_bytes());
    out.extend_from_slice(&4u32.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&16u32.to_be_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&((audio.len() + 4) as i64).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&audio);
    out
}

fn au_fixture(payload_bytes: usize) -> Vec<u8> {
    let audio = patterned_pcm(payload_bytes, true);
    let mut out = b".snd".to_vec();
    out.extend_from_slice(&24u32.to_be_bytes());
    out.extend_from_slice(&(audio.len() as u32).to_be_bytes());
    out.extend_from_slice(&3u32.to_be_bytes());
    out.extend_from_slice(&48_000u32.to_be_bytes());
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&audio);
    out
}

fn wave64_fixture(payload_bytes: usize) -> Vec<u8> {
    let audio = patterned_pcm(payload_bytes, false);
    let fmt = wave_fmt();
    let fmt_total = 24 + fmt.len();
    let fmt_aligned = (fmt_total + 7) / 8 * 8;
    let data_size = 24 + audio.len();
    let data_aligned = (data_size + 7) / 8 * 8;
    let total = 40 + fmt_aligned + data_aligned;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&W64_RIFF);
    out.extend_from_slice(&(total as u64).to_le_bytes());
    out.extend_from_slice(&W64_WAVE);
    out.extend_from_slice(&W64_FMT);
    out.extend_from_slice(&(fmt_total as u64).to_le_bytes());
    out.extend_from_slice(&fmt);
    out.resize(40 + fmt_aligned, 0);
    out.extend_from_slice(&W64_DATA);
    out.extend_from_slice(&(data_size as u64).to_le_bytes());
    out.extend_from_slice(&audio);
    out.resize(total, 0);
    out
}

fn aiff_fixture(payload_bytes: usize) -> Vec<u8> {
    let audio = patterned_pcm(payload_bytes, true);
    let mut body = b"AIFFCOMM".to_vec();
    body.extend_from_slice(&18u32.to_be_bytes());
    body.extend_from_slice(&2u16.to_be_bytes());
    body.extend_from_slice(&((audio.len() / 4) as u32).to_be_bytes());
    body.extend_from_slice(&16u16.to_be_bytes());
    body.extend_from_slice(&[0x40, 0x0e, 0xbb, 0x80, 0, 0, 0, 0, 0, 0]);
    body.extend_from_slice(b"SSND");
    body.extend_from_slice(&((audio.len() + 8) as u32).to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&audio);
    let mut out = b"FORM".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn bmp_fixture(large: bool) -> Vec<u8> {
    let (width, height) = if large { (1024u32, 1024u32) } else { (64u32, 64u32) };
    let row = ((width as usize * 3) + 3) & !3;
    let pixels = row * height as usize;
    let mut out = Vec::with_capacity(54 + pixels);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + pixels) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixels as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    for y in 0..height {
        for x in 0..width {
            out.extend_from_slice(&[(x ^ y) as u8, x.wrapping_mul(3) as u8, y.wrapping_mul(5) as u8]);
        }
        out.resize(out.len() + row - width as usize * 3, 0);
    }
    out
}

fn utf16_fixture(payload_bytes: usize) -> Vec<u8> {
    let units = payload_bytes.max(4) / 2;
    let mut out = Vec::with_capacity(units * 2);
    out.extend_from_slice(&[0xff, 0xfe]);
    for index in 1..units {
        let value = b'a' as u16 + (index % 26) as u16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn fixture(format: &str, large: bool) -> Vec<u8> {
    let size = if large { LARGE_PAYLOAD } else { SMALL_PAYLOAD };
    match format {
        "exfmt:audio:raw-pcm" => patterned_pcm(size, false),
        "exfmt:audio:wav-pcm" => riff_fixture(b"RIFF", size, false),
        "exfmt:audio:rf64-pcm" => riff_fixture(b"RF64", size, false),
        "exfmt:audio:bw64-pcm" => riff_fixture(b"BW64", size, false),
        "exfmt:audio:bwf-pcm" => riff_fixture(b"RIFF", size, true),
        "exfmt:audio:caf-pcm" => caf_fixture(size),
        "exfmt:audio:au-pcm" => au_fixture(size),
        "exfmt:audio:wave64-pcm" => wave64_fixture(size),
        "exfmt:audio:aiff-pcm" => aiff_fixture(size),
        "exfmt:image:bmp-family" => bmp_fixture(large),
        "exfmt:text:utf-16" => utf16_fixture(size),
        other => panic!("no benchmark fixture for {other}"),
    }
}

fn percentile(samples: &mut [Duration], numerator: usize, denominator: usize) -> u128 {
    samples.sort_unstable();
    let index = ((samples.len() - 1) * numerator + denominator - 1) / denominator;
    samples[index].as_nanos()
}

fn measure(
    kernel: &Kernel,
    adapter_id: &str,
    capability_id: &str,
    source: &[u8],
    samples: usize,
) -> SampleSummary {
    for _ in 0..WARMUPS {
        let mut input = Cursor::new(source);
        let mut output = Vec::new();
        black_box(kernel.invoke_defaults(adapter_id, capability_id, &mut input, &mut output).expect("benchmark invocation"));
        black_box(output);
    }
    let mut durations = Vec::with_capacity(samples);
    let mut output_bytes = 0;
    let mut peak_memory_bytes = 0;
    for _ in 0..samples {
        let mut input = Cursor::new(source);
        let mut output = Vec::new();
        let started = Instant::now();
        let report = kernel.invoke_defaults(adapter_id, capability_id, &mut input, &mut output).expect("benchmark invocation");
        durations.push(started.elapsed());
        output_bytes = report.measurements.output_bytes.unwrap_or(output.len() as u64);
        peak_memory_bytes = report.measurements.peak_memory_bytes.unwrap_or(0);
        black_box(output);
    }
    let mut p50_samples = durations.clone();
    let p50_ns = percentile(&mut p50_samples, 50, 100);
    let p95_ns = percentile(&mut durations, 95, 100);
    SampleSummary { p50_ns, p95_ns, output_bytes, peak_memory_bytes }
}

fn calibrated_copy(payload_bytes: usize) -> SampleSummary {
    let source = patterned_pcm(payload_bytes, false);
    let mut durations = Vec::with_capacity(21);
    for _ in 0..3 {
        black_box(source.clone());
    }
    for _ in 0..21 {
        let started = Instant::now();
        black_box(source.clone());
        durations.push(started.elapsed());
    }
    let mut p50_samples = durations.clone();
    SampleSummary {
        p50_ns: percentile(&mut p50_samples, 50, 100),
        p95_ns: percentile(&mut durations, 95, 100),
        output_bytes: source.len() as u64,
        peak_memory_bytes: source.len() as u64,
    }
}

fn main() {
    let mut kernel = Kernel::default();
    let handshakes = generated_adapters::register_all(&mut kernel);
    let calibration = calibrated_copy(LARGE_PAYLOAD);
    println!("EXBENCH\tCALIBRATION\t{}\t{}\t{}", LARGE_PAYLOAD, calibration.p50_ns, calibration.p95_ns);
    let mut count = 0usize;
    for handshake in handshakes {
        for capability in handshake.capabilities {
            let source_format = capability.source_formats.first().expect("source format");
            let small_source = fixture(source_format, false);
            let large_source = fixture(source_format, true);
            let small = measure(&kernel, &handshake.adapter_id, &capability.capability_id, &small_source, SMALL_SAMPLES);
            let large = measure(&kernel, &handshake.adapter_id, &capability.capability_id, &large_source, LARGE_SAMPLES);
            println!(
                "EXBENCH\tCAPABILITY\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                handshake.capsule.id,
                capability.capability_id,
                capability.strategy,
                capability.backend,
                source_format,
                small_source.len(),
                small.output_bytes,
                small.p50_ns,
                small.p95_ns,
                large_source.len(),
                large.output_bytes,
                large.p50_ns,
                large.p95_ns,
                large.peak_memory_bytes,
            );
            count += 1;
        }
    }
    println!("EXBENCH\tSUMMARY\t{}\t{}", count, generated_adapters::ADAPTER_COUNT);
}
