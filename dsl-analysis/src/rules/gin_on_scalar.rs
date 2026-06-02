//! sql230: `CREATE INDEX ... USING GIN (col)` where `col` is a
//! plain scalar (text/int/etc) -- GIN supports array, jsonb,
//! tsvector, and trgm-extension operator classes. PG raises 42704
//! "data type X has no default operator class for access method gin"
//! when none of the GIN ops applies.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql230"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("INDEX") {
      return;
    }
    if !upper.contains("USING GIN") {
      return;
    }
    if upper.contains("OPCLASS") || upper.contains("GIN_TRGM_OPS") || upper.contains("JSONB_PATH_OPS") {
      return;
    }
    // Find table after ON.
    let Some(on_at) = upper.find(" ON ") else { return };
    let after = on_at + " ON ".len();
    let rest = &body[after..];
    let id_end =
      rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let table_raw = &rest[..id_end];
    let table = table_raw.rsplit('.').next().unwrap_or(table_raw).trim_matches('"');
    let Some(t) = catalog.find_table(None, table) else { return };
    // Find column list after USING GIN.
    let Some(gin_at) = upper.find("USING GIN") else { return };
    let after_gin = gin_at + "USING GIN".len();
    let post = body[after_gin..].trim_start();
    if !post.starts_with('(') {
      return;
    }
    let open = after_gin + (body[after_gin..].len() - post.len());
    let Some(close) = find_matching_paren(body, open) else { return };
    let cols = &body[open + 1..close];
    for raw in cols.split(',') {
      let token = raw.trim();
      // Skip expressions / operator classes.
      if token.contains('(') || token.contains(' ') {
        continue;
      }
      let bare = token.trim_matches('"');
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(bare)) else { continue };
      let ty = col.data_type.to_ascii_lowercase();
      let ok = ty.ends_with("[]")
        || ty.contains("jsonb")
        || ty.contains("tsvector")
        || ty.contains("tsquery")
        || ty.contains("hstore")
        || ty.contains("trgm");
      if ok {
        continue;
      }
      let off = cols.find(token).unwrap_or(0);
      let abs_s = start + open + 1 + off;
      let abs_e = abs_s + token.len();
      out.push(Diagnostic {
        code: "sql230",
        severity: Severity::Warning,
        message: format!(
          "GIN index on `{bare}` (type `{}`) -- GIN needs array/jsonb/tsvector or a custom opclass like gin_trgm_ops",
          col.data_type,
        ),
        range: crate::range_at(abs_s, abs_e),
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
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
