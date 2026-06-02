//! sql489: `WHERE col + 0 = N`, `col - 0 = N`, `col * 1 = N`,
//! `col / 1 = N` (and the commutative `0 + col`, `1 * col`) --
//! wrapping a column in an arithmetic identity defeats a btree
//! index on that column. The expression is equal to `col` itself;
//! remove the no-op operand.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql489"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    // Find WHERE clause; bail if absent.
    let Some(rel_where) = find_clause(ub, b"WHERE") else {
      return;
    };
    let clause_end = find_clause_end(
      ub,
      rel_where + 5,
      &["GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING"],
    );
    let clause_start = rel_where + 5;
    let clause_end = clause_end.min(n);
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut i = clause_start;
    while i < clause_end {
      let c = bytes[i];
      // Pattern A: <ident> <op> <0|1>
      // Look for an operator surrounded by optional whitespace.
      if matches!(c, b'+' | b'-' | b'*' | b'/') {
        let op = c;
        let lit_needed: u8 = if op == b'+' || op == b'-' { b'0' } else { b'1' };
        // Right of operator: skip ws, find the literal (single
        // char `0` or `1`), then ensure no following digit/dot.
        let mut r = i + 1;
        while r < clause_end && bytes[r].is_ascii_whitespace() {
          r += 1;
        }
        let right_ok = r < clause_end
          && bytes[r] == lit_needed
          && (r + 1 == clause_end
            || (bytes[r + 1] != b'.' && !bytes[r + 1].is_ascii_digit()));
        // Left of operator: skip ws, walk back over an ident.
        let mut l = i;
        let pre_op = i;
        while l > clause_start && bytes[l - 1].is_ascii_whitespace() {
          l -= 1;
        }
        let ident_end = l;
        let mut id_start = ident_end;
        while id_start > clause_start && is_word(bytes[id_start - 1] as char) {
          id_start -= 1;
        }
        let left_ok = id_start < ident_end
          && !bytes[id_start].is_ascii_digit() // not a numeric literal
          && (id_start == clause_start
            // The byte before the ident must not be an ident/dot
            // (no `t.col` -- still flag) or operator-continuation.
            || !is_word(bytes[id_start - 1] as char));
        // Pattern B: <0|1> <op> <ident>  (only for + and *)
        let mut pattern_b_hit = false;
        if !right_ok && (op == b'+' || op == b'*') {
          // Reset and check the other direction.
          // Left side must be `0` (for +) or `1` (for *), followed
          // by op, then ident.
          let lit_b: u8 = if op == b'+' { b'0' } else { b'1' };
          let mut ll = i;
          while ll > clause_start && bytes[ll - 1].is_ascii_whitespace() {
            ll -= 1;
          }
          let lit_ok = ll > clause_start
            && bytes[ll - 1] == lit_b
            && (ll - 1 == clause_start
              || (!bytes[ll - 2].is_ascii_digit() && bytes[ll - 2] != b'.'));
          let mut rr = i + 1;
          while rr < clause_end && bytes[rr].is_ascii_whitespace() {
            rr += 1;
          }
          let mut id_end_b = rr;
          while id_end_b < clause_end && is_word(bytes[id_end_b] as char) {
            id_end_b += 1;
          }
          let ident_ok = rr < id_end_b && !bytes[rr].is_ascii_digit();
          if lit_ok && ident_ok && emitted.insert(pre_op) {
            let abs_s = start + (ll - 1);
            let abs_e = start + id_end_b;
            out.push(Diagnostic {
              code: "sql489",
              severity: Severity::Hint,
              message: format!(
                "`{}` is an arithmetic identity (no-op) wrapping a column -- defeats a btree index on the column for sargability. Drop the `{}` term and compare the column directly.",
                std::str::from_utf8(&bytes[ll - 1..id_end_b]).unwrap_or("?"),
                op as char,
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
            pattern_b_hit = true;
          }
        }
        if !pattern_b_hit && left_ok && right_ok && emitted.insert(pre_op) {
          let abs_s = start + id_start;
          let abs_e = start + r + 1;
          out.push(Diagnostic {
            code: "sql489",
            severity: Severity::Hint,
            message: format!(
              "`{}` is an arithmetic identity (no-op) wrapping a column -- defeats a btree index on the column for sargability. Drop the `{}` term and compare the column directly.",
              std::str::from_utf8(&bytes[id_start..r + 1]).unwrap_or("?"),
              op as char,
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i += 1;
    }
  }
}
