//! sql431: `SELECT ... FOR UPDATE` combined with `UNION` /
//! `INTERSECT` / `EXCEPT`. PG raises 0A000
//! "FOR UPDATE is not allowed with UNION/INTERSECT/EXCEPT operation"
//! both for the trailing-FOR-UPDATE shape and the per-arm shape
//! `(SELECT ... FOR UPDATE) UNION (...)`. Hoist the row-locking
//! query into a CTE / outer wrapper and apply FOR UPDATE there.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql431"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bu = upper.as_bytes();
    // Find a top-level (depth==0) UNION/INTERSECT/EXCEPT and a FOR
    // UPDATE / FOR SHARE / FOR NO KEY UPDATE / FOR KEY SHARE anywhere
    // in the statement (top-level or inside any paren -- both shapes
    // are rejected by PG).
    let mut depth = 0i32;
    let mut i = 0usize;
    let mut top_level_setop: Option<usize> = None;
    let bytes = cleaned.as_bytes();
    while i < bu.len() {
      let c = bytes[i];
      if c == b'\'' {
        i += 1;
        while i < bu.len() && bytes[i] != b'\'' {
          i += 1;
        }
        i = (i + 1).min(bu.len());
        continue;
      }
      if c == b'(' {
        depth += 1;
        i += 1;
        continue;
      }
      if c == b')' {
        depth -= 1;
        i += 1;
        continue;
      }
      if depth == 0 && top_level_setop.is_none() {
        for kw in [&b"UNION"[..], &b"INTERSECT"[..], &b"EXCEPT"[..]] {
          if i + kw.len() <= bu.len()
            && bu[i..i + kw.len()] == *kw
            && (i == 0 || !is_word(bu[i - 1] as char))
            && (i + kw.len() == bu.len() || !is_word(bu[i + kw.len()] as char))
          {
            top_level_setop = Some(i);
            break;
          }
        }
      }
      i += 1;
    }
    let Some(_) = top_level_setop else { return };
    // Find any FOR UPDATE / FOR SHARE / FOR NO KEY UPDATE / FOR KEY
    // SHARE token sequence (depth-agnostic; PG rejects both shapes).
    let Some(lock_at) = find_locking_for(bu) else { return };
    let abs_s = start + lock_at;
    // Heuristic end: scan to next word boundary past two words.
    let mut p = lock_at;
    for _ in 0..4 {
      while p < bu.len() && bu[p].is_ascii_whitespace() {
        p += 1;
      }
      while p < bu.len() && is_word(bu[p] as char) {
        p += 1;
      }
    }
    let abs_e = (start + p).min(end);
    out.push(Diagnostic {
      code: "sql431",
      severity: Severity::Error,
      message: "row-locking clause (`FOR UPDATE` / `FOR SHARE`) is not allowed with UNION / INTERSECT / EXCEPT -- PG raises 0A000; hoist the locking query into a CTE or outer wrapper and apply the lock there".into(),
      range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

/// Find the byte index of a `FOR <UPDATE|SHARE|NO KEY UPDATE|KEY SHARE>`
/// token sequence in the upper-cased, comment-stripped buffer. Skips
/// `FOR EACH ROW`, `FOR ALL`, etc. by checking that the following word
/// is one of the locking strengths.
fn find_locking_for(bu: &[u8]) -> Option<usize> {
  let mut i = 0usize;
  while i + 3 <= bu.len() {
    if bu[i..i + 3] == *b"FOR"
      && (i == 0 || !is_word(bu[i - 1] as char))
      && (i + 3 == bu.len() || !is_word(bu[i + 3] as char))
    {
      // Look at the next word.
      let mut j = i + 3;
      while j < bu.len() && bu[j].is_ascii_whitespace() {
        j += 1;
      }
      let w1_start = j;
      while j < bu.len() && is_word(bu[j] as char) {
        j += 1;
      }
      let w1 = &bu[w1_start..j];
      if w1 == b"UPDATE" || w1 == b"SHARE" {
        return Some(i);
      }
      if w1 == b"NO" || w1 == b"KEY" {
        // FOR NO KEY UPDATE / FOR KEY SHARE
        let mut k = j;
        while k < bu.len() && bu[k].is_ascii_whitespace() {
          k += 1;
        }
        let w2_start = k;
        while k < bu.len() && is_word(bu[k] as char) {
          k += 1;
        }
        let w2 = &bu[w2_start..k];
        if w1 == b"NO" && w2 == b"KEY" {
          // expect UPDATE
          let mut m = k;
          while m < bu.len() && bu[m].is_ascii_whitespace() {
            m += 1;
          }
          let w3_start = m;
          while m < bu.len() && is_word(bu[m] as char) {
            m += 1;
          }
          if &bu[w3_start..m] == b"UPDATE" {
            return Some(i);
          }
        }
        if w1 == b"KEY" && w2 == b"SHARE" {
          return Some(i);
        }
      }
    }
    i += 1;
  }
  None
}
