//! sql584: the internal `pg_catalog` type aliases (`int4`, `int8`, `float8`,
//! `serial4`, ...) in DDL. They're valid but read as implementation detail;
//! the SQL-standard spellings (`integer`, `bigint`, `double precision`, ...)
//! are clearer and what the docs use. Scoped to DDL / `::` casts so a column
//! or alias coincidentally named `int4` isn't flagged.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const ALIASES: &[(&str, &str)] = &[
  ("INT2", "smallint"),
  ("INT4", "integer"),
  ("INT8", "bigint"),
  ("FLOAT4", "real"),
  ("FLOAT8", "double precision"),
  ("SERIAL2", "smallserial"),
  ("SERIAL4", "serial"),
  ("SERIAL8", "bigserial"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql584"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let is_ddl = upper.contains("CREATE TABLE") || upper.contains("ALTER TABLE") || upper.contains("CREATE TYPE") || upper.contains("CREATE DOMAIN");
    if !is_ddl && !upper.contains("::") {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    'outer: while i < n {
      if ub[i] == b'\'' {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
        i += 1;
        continue;
      }
      let cast = i >= 2 && &ub[i - 2..i] == b"::";
      if is_ddl || cast {
        for (alias, std) in ALIASES {
          let a = alias.as_bytes();
          if i + a.len() <= n
            && &ub[i..i + a.len()] == a
            && (i == 0 || !is_word(ub[i - 1] as char))
            && (i + a.len() == n || !is_word(ub[i + a.len()] as char))
          {
            out.push(Diagnostic {
              code: "sql584",
              severity: Severity::Hint,
              message: format!("`{}` is an internal type alias -- prefer the standard `{std}`", alias.to_ascii_lowercase()),
              range: crate::range_at(start + i, start + i + a.len()),
            });
            i += a.len();
            continue 'outer;
          }
        }
      }
      i += 1;
    }
  }
}
