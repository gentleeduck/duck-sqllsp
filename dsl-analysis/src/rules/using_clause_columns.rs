//! sql187: `JOIN other USING (col)` -- col must exist on BOTH sides
//! of the join. PG raises 42703 at runtime when missing. Flag at
//! edit time when the catalog has both tables but `col` isn't a
//! column of at least one.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql187"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(sel) = &stmt.kind else { return };
    if sel.from.is_empty() || sel.joins.is_empty() {
      return;
    }
    let Some(left_first) = catalog.find_table(sel.from[0].schema.as_deref(), &sel.from[0].name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();

    // Build a cumulative set of column names visible on the LEFT side
    // of each JOIN: start with the first FROM table + every other FROM
    // table (comma-separated FROMs are equivalent to CROSS JOIN), then
    // add the right side after each JOIN. For the i-th JOIN, the
    // visible-left set is everything before the JOIN.
    let mut visible: std::collections::HashSet<String> = std::collections::HashSet::new();
    for c in &left_first.columns {
      visible.insert(c.name.to_ascii_lowercase());
    }
    for fr in sel.from.iter().skip(1) {
      if let Some(t) = catalog.find_table(fr.schema.as_deref(), &fr.name) {
        for c in &t.columns { visible.insert(c.name.to_ascii_lowercase()); }
      }
    }

    // Scan every USING (...) -- pair it with the surrounding JOIN's
    // right side table via parallel walk through sel.joins.
    let mut join_iter = sel.joins.iter();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(" USING ") {
      let after = from + rel + " USING ".len();
      let bytes = body.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= bytes.len() || bytes[k] != b'(' {
        from = after;
        continue;
      }
      let close = match_paren(bytes, k);
      let inner = &body[k + 1..close];
      let cols: Vec<&str> = inner.split(',').map(|c| c.trim().trim_matches('"')).filter(|c| !c.is_empty()).collect();
      // The right side -- next unconsumed sel.joins entry.
      let Some(join) = join_iter.next() else { break };
      let right = catalog.find_table(join.table.schema.as_deref(), &join.table.name);
      for col in cols {
        let col_lc = col.to_ascii_lowercase();
        let in_left = visible.contains(&col_lc);
        let in_right = right.is_some_and(|r| r.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)));
        if in_left && in_right { continue; }
        // The AST doesn't distinguish USING vs ON joins. When the
        // query mixes them the text-based pairing of USING -> next
        // join may misalign and falsely flag a valid USING. Only fire
        // when the column is missing from EVERY known table (no
        // catalog table in the query has it).
        let in_any_join_table = sel.joins.iter().any(|j| {
          catalog
            .find_table(j.table.schema.as_deref(), &j.table.name)
            .is_some_and(|t| t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)))
        });
        if in_any_join_table { continue; }
        let abs_s = start + k + 1;
        let abs_e = start + close;
        let detail = if !in_left {
          format!("`{col}` missing on left (`{}.{}` and earlier joins)", left_first.schema, left_first.name)
        } else {
          format!("`{col}` missing on right `{}`", join.table.name)
        };
        out.push(Diagnostic {
          code: "sql187",
          severity: Severity::Error,
          message: format!("USING clause invalid: {detail}"),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      // After processing this JOIN, merge its right-side columns into
      // the visible-left set for subsequent USING clauses.
      if let Some(r) = right {
        for c in &r.columns { visible.insert(c.name.to_ascii_lowercase()); }
      }
      from = close + 1;
    }
  }
}

fn match_paren(bytes: &[u8], open: usize) -> usize {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return i;
        }
      }
      _ => {}
    }
    i += 1;
  }
  n
}
