//! sql301: `COPY ... FROM PROGRAM 'cmd'` / `COPY ... TO PROGRAM 'cmd'`
//! -- runs a shell command as the PG server OS user. Requires
//! superuser and is a massive RCE risk if reachable from
//! user-supplied SQL. Flag it loudly.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql301"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("COPY") {
      return;
    }
    let Some(at) = upper.find("PROGRAM") else { return };
    if at > 0 {
      let prev = body.as_bytes()[at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' {
        return;
      }
    }
    let abs_s = start + at;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql301",
      severity: Severity::Error,
      message: "COPY ... PROGRAM '<cmd>' runs arbitrary shell as the PG server OS user -- superuser-only, massive RCE risk; use STDIN/STDOUT or `\\copy` instead".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
