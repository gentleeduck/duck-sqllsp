//! sql423: `col ~ '^prefix'` (or `~* '^prefix'`) where the regex is
//! just an anchored literal prefix could be rewritten as `col LIKE
//! 'prefix%'` (or `ILIKE`). The LIKE form is sargable when the
//! column has a btree `text_pattern_ops` (or default `text_ops` for
//! the C locale) index; the regex form usually isn't.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql423"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      if bytes[i] != b'~' {
        i += 1;
        continue;
      }
      // Determine operator form: `~` or `~*` (also avoid `!~`).
      if i > 0 && bytes[i - 1] == b'!' {
        i += 1;
        continue;
      }
      let case_insensitive = i + 1 < n && bytes[i + 1] == b'*';
      let op_end = if case_insensitive { i + 2 } else { i + 1 };
      // Skip whitespace.
      let mut j = op_end;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= n || bytes[j] != b'\'' {
        i = op_end;
        continue;
      }
      let lit_start = j + 1;
      let mut k = lit_start;
      while k < n && bytes[k] != b'\'' {
        k += 1;
      }
      if k >= n {
        i = op_end;
        continue;
      }
      let pat = &body[lit_start..k];
      let lit_end = k + 1;
      if let Some(rewrite) = literal_prefix_pattern(pat) {
        let abs_s = start + i;
        let abs_e = start + lit_end;
        let msg = match rewrite {
          Rewrite::Prefix(p) => {
            let like_kw = if case_insensitive { "ILIKE" } else { "LIKE" };
            format!(
              "regex `{pat}` is an anchored literal prefix -- consider `{like_kw} '{p}%'` (LIKE can use btree text_pattern_ops indexes; regex usually can't)"
            )
          },
          Rewrite::Exact(p) => {
            if case_insensitive {
              format!("regex `{pat}` is an exact-match literal -- consider `lower(col) = '{p}'` (or `ILIKE '{p}'` for the case-insensitive comparison)")
            } else {
              format!("regex `{pat}` is an exact-match literal -- consider `= '{p}'` (equality uses any btree index; regex usually doesn't)")
            }
          },
        };
        out.push(Diagnostic {
          code: "sql423",
          severity: Severity::Hint,
          message: msg,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = lit_end.max(op_end);
    }
  }
}

enum Rewrite {
  Prefix(String),
  Exact(String),
}

/// Recognize `^literal` (optionally followed by `.*` or `$`) and
/// return the literal as a Prefix or Exact rewrite hint. Returns
/// None for any pattern containing regex metacharacters that would
/// break the rewrite. Conservative.
fn literal_prefix_pattern(pat: &str) -> Option<Rewrite> {
  let bytes = pat.as_bytes();
  if bytes.is_empty() || bytes[0] != b'^' {
    return None;
  }
  // Detect `^...$` (exact match) vs prefix.
  let exact = bytes[bytes.len() - 1] == b'$';
  let mut end = bytes.len();
  if exact {
    end -= 1;
  } else if end >= 2 && &bytes[end - 2..end] == b".*" {
    end -= 2;
  }
  let body = &bytes[1..end];
  if body.is_empty() {
    return None;
  }
  // Reject any regex metacharacter in the body.
  const META: &[u8] = b"[]().*+?|{}\\^$";
  for &b in body {
    if META.contains(&b) {
      return None;
    }
  }
  let text = std::str::from_utf8(body).ok()?.to_string();
  Some(if exact { Rewrite::Exact(text) } else { Rewrite::Prefix(text) })
}
