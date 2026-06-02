use dsl_analysis::run;
use dsl_catalog::Catalog;
use dsl_parse::{parse, Dialect};
use dsl_resolve::resolve_with_source;

#[test]
#[ignore]
fn perf_10k_stmts() {
  let mut s = String::with_capacity(400_000);
  for i in 0..10_000 {
    s.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let t0 = std::time::Instant::now();
  let file = parse(&s, Dialect::Postgres);
  let p = t0.elapsed();
  let scopes = resolve_with_source(&file.statements, &s);
  let r = t0.elapsed();
  let _ = run(&s, &file, &scopes, &Catalog::default());
  let elapsed = t0.elapsed();
  eprintln!("parse: {:?}  resolve: {:?}  total: {:?}", p, r - p, elapsed);
}
