//! sql341: `INSERT INTO t (col) VALUES (ARRAY[...])` where the array
//! element family doesn't match the target column's element family,
//! e.g. `text_col := ARRAY[1, 2, 3]`. Conservative: only fires when
//! both element + column families are known and disagree.

use crate::typing::{TypeFamily, classify, literal_family};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql341"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };
    if ins.columns.is_empty() {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(values_at) = upper.find("VALUES") else { return };
    let after = values_at + "VALUES".len();
    let bytes = body.as_bytes();
    let mut j = after;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
      j += 1
    }
    if j >= bytes.len() || bytes[j] != b'(' {
      return;
    }
    let Some(close) = matched_close(bytes, j) else { return };
    let row_body = &body[j + 1..close];
    let elems: Vec<&str> = top_level_commas(row_body);
    for (idx, raw_val) in elems.iter().enumerate() {
      let Some(col_name) = ins.columns.get(idx) else { continue };
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      let col_fam = classify(&col.data_type);
      let val_trim = raw_val.trim();
      let val_upper = val_trim.to_ascii_uppercase();
      if !val_upper.starts_with("ARRAY[") || !val_trim.ends_with(']') {
        continue;
      }
      let arr_inner = &val_trim[6..val_trim.len() - 1];
      let arr_elems: Vec<&str> = top_level_commas(arr_inner);
      let mut elem_fams = arr_elems.iter().map(|e| literal_family(e.trim())).filter(|f| *f != TypeFamily::Unknown);
      let Some(elem_fam) = elem_fams.next() else { continue };
      if col_fam != TypeFamily::Array {
        continue;
      }
      let raw_ty = col.data_type.trim_end_matches("[]");
      let target_elem = classify(raw_ty);
      if target_elem == TypeFamily::Unknown {
        continue;
      }
      if !families_compatible(elem_fam, target_elem) {
        let Some(at) = body.find(val_trim) else { return };
        let abs_s = start + at;
        let abs_e = abs_s + val_trim.len();
        out.push(Diagnostic {
          code: "sql341",
          severity: Severity::Warning,
          message: format!(
            "array element family `{}` doesn't match column `{}` element family `{}`",
            elem_fam.name(),
            col.name,
            target_elem.name()
          ),
          range: crate::range_at(abs_s, abs_e),
        });
        return;
      }
    }
  }
}

fn families_compatible(a: TypeFamily, b: TypeFamily) -> bool {
  if a == b {
    return true;
  }
  if a.is_numeric() && b.is_numeric() {
    return true;
  }
  false
}

fn matched_close(bytes: &[u8], open: usize) -> Option<usize> {
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

fn top_level_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1
      },
      _ => {},
    }
    i += 1;
  }
  out.push(&s[start..]);
  out
}
