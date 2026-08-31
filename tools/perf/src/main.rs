use std::hint::black_box;
use std::time::{Duration, Instant};

use uax14_linebreak::linebreaks;

const CASES: &str = include_str!("../../../tests/data/line-break-test-17.txt");

/// Time ceiling for one decade of input growth. A linear pass costs ten
/// times more per tenfold input, so anything at or under this bound rules
/// out a quadratic rescan while leaving room for constant overhead and
/// timer noise.
const DECADE_RATIO_LIMIT: f64 = 11.0;

/// Throughput floors in MiB/s per corpus, set with wide margin under the
/// values measured on the development machine so the gate catches a real
/// regression instead of scheduler noise.
const FLOORS: [(&str, f64); 3] = [("ascii", 100.0), ("conformance", 95.0), ("cjk", 190.0)];

fn main() {
    let ascii =
        "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs. "
            .repeat(256);
    let cjk = "日本語の行分割を測定します。中文换行性能测试。한국어 줄바꿈 성능 측정. ".repeat(256);
    let conformance = conformance_corpus();
    let corpora = [("ascii", ascii), ("conformance", conformance), ("cjk", cjk)];

    let mut failed = false;
    for (name, text) in corpora {
        let decades = [text.clone(), text.repeat(10), text.repeat(100)];
        let mut per_pass = Vec::new();
        for decade in &decades {
            let rounds = (32_000_000 / decade.len()).max(3);
            let total = median(samples(|| run(decade, rounds)));
            per_pass.push(total.as_secs_f64() / rounds as f64);
        }
        let throughput = decades[1].len() as f64 / per_pass[1] / (1024.0 * 1024.0);
        let ratios = [per_pass[1] / per_pass[0], per_pass[2] / per_pass[1]];
        let floor = FLOORS
            .iter()
            .find(|(corpus, _)| *corpus == name)
            .map(|(_, floor)| *floor)
            .unwrap();
        let linear = ratios.iter().all(|ratio| *ratio <= DECADE_RATIO_LIMIT);
        let fast_enough = throughput >= floor;
        println!(
            "{name}: throughput={throughput:.1} MiB/s (floor {floor:.1}) decade ratios={:.2}/{:.2} (limit {DECADE_RATIO_LIMIT})",
            ratios[0], ratios[1]
        );
        failed |= !linear || !fast_enough;
    }
    if failed {
        std::process::exit(1);
    }
}

fn samples<F>(mut run: F) -> Vec<Duration>
where
    F: FnMut() -> Duration,
{
    run();
    (0..9).map(|_| run()).collect()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn run(text: &str, rounds: usize) -> Duration {
    let start = Instant::now();
    let mut total = 0;
    for _ in 0..rounds {
        total += linebreaks(black_box(text)).count();
    }
    black_box(total);
    start.elapsed()
}

fn conformance_corpus() -> String {
    let mut text = String::new();
    for line in CASES.lines() {
        for token in line.split_whitespace() {
            if token == "×" || token == "÷" {
                continue;
            }
            let point = u32::from_str_radix(token, 16).unwrap();
            text.push(char::from_u32(point).unwrap());
        }
    }
    text
}
