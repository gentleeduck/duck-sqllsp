//! sql778: `CREATE DOMAIN ... CHECK (VALUE <op> <literal>) DEFAULT
//! <literal>` where the DEFAULT literal plainly fails the CHECK. Only
//! handles a single `VALUE <op> numeric-literal` comparison (either
//! operand order) and a single numeric DEFAULT literal -- anything
//! more complex is left alone. Warning rather than Error: PostgreSQL's
//! exact validation timing for a domain's own default isn't confirmed.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql778"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE DOMAIN") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(chk) = find_word_from(ub, 0, b"CHECK") else { return };
    let p = skip_ws(ub, chk + "CHECK".len());
    if ub.get(p) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, p) else { return };
    let inner = upper[p + 1..close].trim();
    let Some((op, threshold)) = parse_value_compare(inner) else { return };
    let Some(default_start) = find_word_from(ub, close, b"DEFAULT") else { return };
    let mut d = skip_ws(ub, default_start + "DEFAULT".len());
    let neg = ub.get(d) == Some(&b'-');
    if neg {
      d += 1;
    }
    let num_start = d;
    while d < ub.len() && (ub[d].is_ascii_digit() || ub[d] == b'.') {
      d += 1;
    }
    if d == num_start {
      return;
    }
    let Ok(mut default_val) = upper[num_start..d].parse::<f64>() else { return };
    if neg {
      default_val = -default_val;
    }
    let satisfied = match op {
      ">" => default_val > threshold,
      ">=" => default_val >= threshold,
      "<" => default_val < threshold,
      "<=" => default_val <= threshold,
      "=" => (default_val - threshold).abs() < f64::EPSILON,
      "<>" => (default_val - threshold).abs() >= f64::EPSILON,
      _ => return,
    };
    if !satisfied {
      out.push(Diagnostic {
        code: "sql778",
        severity: Severity::Warning,
        message: format!("DEFAULT {default_val} fails the domain's own CHECK (VALUE {op} {threshold})"),
        range: crate::range_at(start + default_start, start + d),
      });
    }
  }
}

/// Parse `VALUE <op> <literal>` in either operand order, returning the
/// operator normalized to VALUE-on-the-left form and the threshold.
fn parse_value_compare(s: &str) -> Option<(&'static str, f64)> {
  const OPS: &[&str] = &["<>", "!=", ">=", "<=", "=", ">", "<"];
  for op in OPS {
    let Some(at) = s.find(op) else { continue };
    let (lhs, rhs) = (s[..at].trim(), s[at + op.len()..].trim());
    if lhs == "VALUE"
      && let Ok(n) = rhs.parse::<f64>()
    {
      let norm = if *op == "!=" { "<>" } else { op };
      return Some((norm, n));
    }
    if rhs == "VALUE"
      && let Ok(n) = lhs.parse::<f64>()
    {
      let flipped = match *op {
        ">" => "<",
        "<" => ">",
        ">=" => "<=",
        "<=" => ">=",
        "!=" => "<>",
        other => other,
      };
      return Some((flipped, n));
    }
  }
  None
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if &ub[i..i + w.len()] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
