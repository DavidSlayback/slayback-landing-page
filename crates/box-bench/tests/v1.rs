//! The regression gate for bench v1 (`bench/README.md`). Its published runs read back into today's types unchanged
//! and summarise to the published numbers, on any machine. Re-analysing them from their records also replays the game,
//! which only reproduces on the platform and commit they were recorded on (Windows, fbc84d8): on Linux the game's
//! floats drift, and at fbc84d8 only 28 of the 75 runs replay to their outcome, none of the brawler's.
//! `scripts/bench-v1-gate.sh` runs that check with today's bench crates on fbc84d8.

use box_bench::{RunResult, Summary, analyse, summarise};
use box_mcp::Record;
use serde_json::Value;

/// Bench v1's sweeps, the README's table.
const V1: [&str; 6] = [
    "2026-10-23-0134-haiku",
    "2026-10-23-0150-sonnet",
    "2026-10-23-0202-opus",
    "2026-10-23-0253-brawler",
    "2026-10-23-0259-random",
    "2026-10-23-0413-policy-torch-mini-1",
];

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bench/results");

fn published(sweep: &str) -> (Value, Summary, Vec<RunResult>) {
    let text = std::fs::read_to_string(format!("{ROOT}/{sweep}/results.json")).unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    let summary = serde_json::from_value(doc["summary"].clone()).unwrap();
    let runs = serde_json::from_value(doc["runs"].clone()).unwrap();
    (doc, summary, runs)
}

/// Every field path in a JSON value, sorted.
fn fields(v: &Value) -> Vec<String> {
    fn walk(v: &Value, at: String, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, v)| walk(v, format!("{at}.{k}"), out)),
            Value::Array(a) => a.iter().for_each(|v| walk(v, format!("{at}[]"), out)),
            _ => out.push(at),
        }
    }
    let mut out = vec![];
    walk(v, String::new(), &mut out);
    out.sort();
    out.dedup();
    out
}

#[test]
fn bench_v1_runs_read_back_unchanged_and_summarise_as_published() {
    for sweep in V1 {
        let (doc, summary, runs) = published(sweep);
        // Typed, so an f32 written with a digit more or less is still the same number.
        let again: Vec<RunResult> = serde_json::from_value(serde_json::to_value(&runs).unwrap()).unwrap();
        assert_eq!(again, runs, "{sweep}: the runs read back unchanged");
        assert_eq!(fields(&serde_json::to_value(&runs).unwrap()), fields(&doc["runs"]), "{sweep}: the same fields");
        same(&summarise(&runs), &summary, sweep);
    }
}

/// The published summary exactly, but for the cost total: serde_json reads an f64 back to within its last bit, which
/// a sum of them can carry.
fn same(got: &Summary, published: &Summary, sweep: &str) {
    assert!((got.reported_cost_usd - published.reported_cost_usd).abs() < 1e-9, "{sweep}");
    let got = Summary { reported_cost_usd: published.reported_cost_usd, ..got.clone() };
    assert_eq!(&got, published, "{sweep}");
}

/// Only on Windows at fbc84d8: `scripts/bench-v1-gate.sh`.
#[test]
#[ignore]
fn bench_v1_replays_and_summarises_as_published() {
    for sweep in V1 {
        let (_, summary, runs) = published(sweep);
        let again: Vec<RunResult> = runs
            .iter()
            .map(|r| {
                let path = format!("{ROOT}/{sweep}/{}/record.json", r.dir);
                let record: Record = serde_json::from_str(&std::fs::read_to_string(&path).expect(&path)).unwrap();
                let analysis = analyse(&record);
                assert_eq!(Some(&analysis), r.analysis.as_ref(), "{sweep} {}", r.dir);
                RunResult { analysis: Some(analysis), ..r.clone() }
            })
            .collect();
        same(&summarise(&again), &summary, sweep);
    }
}
