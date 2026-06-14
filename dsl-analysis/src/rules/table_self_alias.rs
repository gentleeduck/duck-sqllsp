//! sql549: `FROM users AS users` / `JOIN orders orders` -- aliasing a table to
//! its own name. The alias adds nothing; drop it (or pick a short alias like
//! `u`). Pure noise, and a tell-tale sign of a half-applied rename.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const NON_ALIAS: &[&str] = &[
  "ON", "USING", "WHERE", "GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "JOIN", "LEFT", "RIGHT", "INNER", "FULL",
  "CROSS", "NATURAL", "UNION", "INTERSECT", "EXCEPT", "RETURNING", "FETCH", "FOR", "WINDOW", "TABLESAMPLE", "LATERAL",
  "AS",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql549"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if !(word_at(ub, i, b"FROM") || word_at(ub, i, b"JOIN")) {
        i += 1;
        continue;
      }
      let kw_len = 4;
      // The table reference (and, for FROM, comma-separated siblings).
      let mut p = i + kw_len;
      loop {
        p = skip_ws(ub, p);
        if p >= n || ub[p] == b'(' {
          break; // subquery / function / end
        }
        let Some((name, after_name)) = read_qual_ident(body, ub, p) else { break };
        let q = skip_ws(ub, after_name);
        // Optional AS.
        let (alias_at, after_as) = if word_at(ub, q, b"AS") { (skip_ws(ub, q + 2), true) } else { (q, false) };
        if let Some((alias, after_alias)) = read_ident(body, ub, alias_at)
          && (after_as || !is_non_alias(ub, alias_at))
          && base_name(&name).eq_ignore_ascii_case(&alias)
        {
          out.push(Diagnostic {
            code: "sql549",
            severity: Severity::Hint,
            message: format!("`{alias}` aliases the table to its own name -- drop the alias"),
            range: crate::range_at(start + p, start + after_alias),
          });
          p = after_alias;
        } else {
          p = after_name;
        }
        // Continue across `, t2 t2` in a FROM list.
        let c = skip_ws(ub, p);
        if c < n && ub[c] == b',' {
          p = c + 1;
        } else {
          break;
        }
      }
      i += kw_len;
    }
  }
}

fn base_name(name: &str) -> &str {
  name.rsplit('.').next().unwrap_or(name).trim_matches('"')
}

fn is_non_alias(ub: &[u8], at: usize) -> bool {
  NON_ALIAS.iter().any(|kw| word_at(ub, at, kw.as_bytes()))
}

fn read_qual_ident(body: &str, ub: &[u8], from: usize) -> Option<(String, usize)> {
  let n = ub.len();
  let mut e = from;
  while e < n && (ub[e].is_ascii_alphanumeric() || ub[e] == b'_' || ub[e] == b'.' || ub[e] == b'"') {
    e += 1;
  }
  if e == from {
    return None;
  }
  Some((body[from..e].to_string(), e))
}

fn read_ident(body: &str, ub: &[u8], from: usize) -> Option<(String, usize)> {
  let n = ub.len();
  let mut e = from;
  while e < n && (is_word(ub[e] as char) || ub[e] == b'"') {
    e += 1;
  }
  if e == from {
    return None;
  }
  Some((body[from..e].trim_matches('"').to_string(), e))
}

fn word_at(ub: &[u8], i: usize, kw: &[u8]) -> bool {
  i + kw.len() <= ub.len()
    && ub[i..i + kw.len()] == *kw
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + kw.len() == ub.len() || !is_word(ub[i + kw.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
