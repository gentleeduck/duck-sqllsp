//! sql117: `INSERT INTO t (col) VALUES ('true')` where `col` is
//! boolean -- the literal `'true'` is text, not bool. Catches the
//! missing `::boolean` cast.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql117" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let StatementKind::Insert(ref ins) = stmt.kind else { return };
        let table = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name);
        let Some(table) = table else { return };
        if ins.columns.is_empty() { return; }
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        // Find `VALUES (` and split the first tuple by top-level commas.
        let Some(values_at) = upper.find("VALUES") else { return };
        let bytes = body.as_bytes();
        let n = bytes.len();
        let mut k = values_at + 6;
        while k < n && bytes[k].is_ascii_whitespace() { k += 1; }
        if k >= n || bytes[k] != b'(' { return; }
        let open = k;
        let mut depth = 1i32;
        let mut j = open + 1;
        let mut parts: Vec<(usize, usize)> = Vec::new();
        let mut part_start = j;
        while j < n {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => { depth -= 1; if depth == 0 { parts.push((part_start, j)); break; } }
                b'\'' => {
                    j += 1;
                    while j < n && bytes[j] != b'\'' { j += 1; }
                }
                b',' if depth == 1 => { parts.push((part_start, j)); part_start = j + 1; }
                _ => {}
            }
            j += 1;
        }
        for (idx, col_name) in ins.columns.iter().enumerate() {
            let Some(col) = table.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
            let ty = col.data_type.to_ascii_lowercase();
            if !(ty == "bool" || ty == "boolean") { continue; }
            let Some(&(ps, pe)) = parts.get(idx) else { continue };
            let v = body[ps..pe].trim();
            let v_up = v.to_ascii_uppercase();
            if !matches!(v_up.as_str(), "'TRUE'" | "'FALSE'" | "'T'" | "'F'") { continue; }
            let lit_start = start + ps + (body[ps..pe].len() - body[ps..pe].trim_start().len());
            let lit_end = lit_start + v.len();
            out.push(Diagnostic {
                code: "sql117",
                severity: Severity::Hint,
                message: format!(
                    "`{}` is text being inserted into boolean column `{}` -- drop the quotes or add `::boolean`",
                    v, col.name
                ),
                range: text_size::TextRange::new(
                    (lit_start as u32).into(),
                    (lit_end as u32).into(),
                ),
            });
            return;
        }
    }
}
