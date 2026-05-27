//! sql209: `COPY t TO 'file.csv'` or `COPY t FROM 'file.csv'` --
//! server-side file access requires PG superuser (or
//! pg_{read,write}_server_files membership). Almost always the author
//! wanted client-side `\copy` (psql) or STDIN/STDOUT. Suggest swap.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql209"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
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
    // Match COPY ... { FROM | TO } '<lit>' (any quote)
    for kw in [" FROM ", " TO "] {
      let Some(rel) = upper.find(kw) else { continue };
      let after = rel + kw.len();
      let rest = body[after..].trim_start();
      if rest.starts_with("STDIN")
        || rest.starts_with("STDOUT")
        || rest.starts_with("stdin")
        || rest.starts_with("stdout")
        || rest.starts_with("PROGRAM")
        || rest.starts_with("program")
      {
        continue;
      }
      if !rest.starts_with('\'') {
        continue;
      }
      // Find the matching closing quote.
      let lit_start = after + (body[after..].len() - rest.len()) + 1;
      let Some(close_rel) = body[lit_start..].find('\'') else { continue };
      let lit_end = lit_start + close_rel;
      let path = &body[lit_start..lit_end];
      let abs_s = start + lit_start - 1;
      let abs_e = start + lit_end + 1;
      out.push(Diagnostic {
        code: "sql209",
        severity: Severity::Warning,
        message: format!(
          "COPY with server-side path `{path}` -- requires superuser or pg_read_server_files / pg_write_server_files; use psql's `\\copy` or COPY ... {} STDIN/STDOUT for client-side I/O",
          if kw == " FROM " { "FROM" } else { "TO" },
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      return;
    }
  }
}
