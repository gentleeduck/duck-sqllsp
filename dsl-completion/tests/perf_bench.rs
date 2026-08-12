use dsl_catalog::Catalog;
use dsl_completion::complete;
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;
use text_size::TextSize;

/// Build an N-statement buffer and the cursor offset right after each
/// statement's `WHERE id = <i>`.
fn build_buffer(n: usize) -> (String, Vec<TextSize>) {
  let mut s = String::with_capacity(n * 50);
  let mut offsets = Vec::with_capacity(n);
  for i in 0..n {
    s.push_str(&format!("SELECT id FROM users WHERE id = {i}"));
    offsets.push(TextSize::from(s.len() as u32));
    s.push_str(";\n");
  }
  (s, offsets)
}

#[test]
#[ignore]
fn perf_10k_stmts_complete_after_each() {
  let (s, offsets) = build_buffer(10_000);
  let t0 = std::time::Instant::now();
  let file = parse(&s, Dialect::Postgres);
  let p = t0.elapsed();
  let scopes = resolve_with_source(&file.statements, &s);
  let r = t0.elapsed();
  let cat = Catalog::default();
  let c0 = std::time::Instant::now();
  for &off in &offsets {
    let _ = complete(&s, &file, &scopes, &cat, off);
  }
  let complete_elapsed = c0.elapsed();
  let elapsed = t0.elapsed();
  eprintln!(
    "parse: {:?}  resolve: {:?}  complete(10k calls): {:?}  avg/call: {:?}  total: {:?}",
    p,
    r - p,
    complete_elapsed,
    complete_elapsed / 10_000,
    elapsed
  );
}

/// Confirms whether `complete()`'s per-call cost depends on total
/// buffer/statement count or just on local (cursor-proximate)
/// context. Found during Phase A baselining (2026-08-18): at 3,000
/// statements complete() already averages ~8ms/call (vs the < 5ms
/// p50 target) and the cost is the SAME whether the cursor sits near
/// the start or the end of the buffer -- ruling out "distance from
/// cursor back to start" as the driver and pointing at something
/// closer to O(total statement/buffer size) per call. Fits
/// `cost ~= 0.65ms + 0.0025ms * n_statements` closely across
/// n = 200/1000/3000 (checked by hand against these three points).
/// This test doesn't assert a hard threshold (timing is
/// machine-dependent) -- it prints first-vs-last timing at three
/// sizes so a regression (or a fix -- first/last staying flat as n
/// grows) is visible by eye. Re-run after the Phase B registry
/// refactor to confirm this scaling problem is actually addressed,
/// not just moved around.
#[test]
#[ignore]
fn perf_scaling_is_position_independent_not_size_independent() {
  for n in [200, 1000, 3000] {
    let (s, offsets) = build_buffer(n);
    let file = parse(&s, Dialect::Postgres);
    let scopes = resolve_with_source(&file.statements, &s);
    let cat = Catalog::default();
    let sample = 20.min(offsets.len());
    let t_first = std::time::Instant::now();
    for &off in offsets.iter().take(sample) {
      let _ = complete(&s, &file, &scopes, &cat, off);
    }
    let first_elapsed = t_first.elapsed();
    let t_last = std::time::Instant::now();
    for &off in offsets.iter().rev().take(sample) {
      let _ = complete(&s, &file, &scopes, &cat, off);
    }
    let last_elapsed = t_last.elapsed();
    eprintln!(
      "n={n:6}  buf_len={:8}  first{sample}: {:?} ({:?}/call)  last{sample}: {:?} ({:?}/call)",
      s.len(),
      first_elapsed,
      first_elapsed / sample as u32,
      last_elapsed,
      last_elapsed / sample as u32
    );
  }
}
