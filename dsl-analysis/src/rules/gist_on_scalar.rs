//! sql272: `CREATE INDEX ... USING GIST (col)` where `col`'s catalog
//! type doesn't have a default GIST operator class. Common cases:
//! plain int/text/uuid. PG raises 42704 unless the btree_gist
//! extension is installed and the opclass is explicit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql272"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("INDEX") { return }
    if !upper.contains("USING GIST") { return }
    if upper.contains("BTREE_GIST") || upper.contains("OPCLASS") || upper.contains("INT4_OPS") { return }
    let Some(on_at) = upper.find(" ON ") else { return };
    let after = on_at + " ON ".len();
    let rest = &body[after..];
    let id_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let table_raw = &rest[..id_end];
    let table = table_raw.rsplit('.').next().unwrap_or(table_raw).trim_matches('"');
    let Some(t) = catalog.find_table(None, table) else { return };
    let Some(gist_at) = upper.find("USING GIST") else { return };
    let after_gist = gist_at + "USING GIST".len();
    let post = body[after_gist..].trim_start();
    if !post.starts_with('(') { return }
    let open = after_gist + (body[after_gist..].len() - post.len());
    let Some(close) = find_matching_paren(body, open) else { return };
    let cols = &body[open + 1..close];
    for raw in cols.split(',') {
      let token = raw.trim();
      if token.contains('(') || token.contains(' ') { continue }
      let bare = token.trim_matches('"');
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(bare)) else { continue };
      let ty = col.data_type.to_ascii_lowercase();
      let ok = ty.starts_with("tsvector") || ty.starts_with("tsquery")
        || ty.contains("range") || ty.contains("multirange")
        || ty.contains("geometry") || ty.contains("geography")
        || ty.contains("point") || ty.contains("box") || ty.contains("circle")
        || ty.contains("polygon") || ty.contains("path") || ty.contains("lseg")
        || ty.contains("inet") || ty.contains("cidr");
      if ok { continue }
      let off = cols.find(token).unwrap_or(0);
      let abs_s = start + open + 1 + off;
      let abs_e = abs_s + token.len();
      out.push(Diagnostic {
        code: "sql272",
        severity: Severity::Warning,
        message: format!(
          "GIST index on `{bare}` (type `{}`) -- no default GIST opclass; install `btree_gist` and pick an explicit opclass",
          col.data_type,
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}
