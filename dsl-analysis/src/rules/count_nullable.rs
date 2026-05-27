//! sql174: `COUNT(col)` where `col` is nullable. Skips NULL rows
//! which the user may not have intended. Suggest `COUNT(*)` or
//! `COUNT(col) FILTER (WHERE col IS NOT NULL)` to make the intent
//! explicit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql174"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(sel) = &stmt.kind else { return };
    if sel.from.is_empty() {
      return;
    }
    let target = &sel.from[0];
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("COUNT(") {
      let after = from + rel + "COUNT(".len();
      let bytes = body.as_bytes();
      let mut k = after;
      let mut depth = 1i32;
      while k < bytes.len() && depth > 0 {
        match bytes[k] {
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              break;
            }
          },
          _ => {},
        }
        k += 1;
      }
      let arg = body[after..k].trim();
      from = k + 1;
      if arg == "*" || arg.eq_ignore_ascii_case("DISTINCT") {
        continue;
      }
      // Strip leading DISTINCT.
      let arg_no_distinct =
        arg.strip_prefix("DISTINCT ").or_else(|| arg.strip_prefix("distinct ")).unwrap_or(arg).trim();
      // Strip alias prefix `a.col` -> `col`.
      let col_name = arg_no_distinct.rsplit('.').next().unwrap_or(arg_no_distinct).trim_matches('"');
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      if !col.nullable {
        continue;
      }
      let abs_s = start + after;
      let abs_e = start + k;
      out.push(Diagnostic {
        code: "sql174",
        severity: Severity::Hint,
        message: format!(
          "COUNT(`{}`) skips NULL rows -- column is nullable. Use COUNT(*) or COUNT(`{}`) FILTER (WHERE `{}` IS NOT NULL)",
          col.name, col.name, col.name
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
