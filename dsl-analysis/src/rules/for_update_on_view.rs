//! sql175: `SELECT ... FROM <view> FOR UPDATE` -- views can't be
//! locked, PG errors at runtime ("FOR UPDATE cannot be applied to
//! the relation 'v'"). Flag the FOR UPDATE / FOR SHARE clause at
//! edit time when the FROM target is a TableKind::View or
//! MaterializedView in the catalog.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, TableKind};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql175"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(sel) = &stmt.kind else { return };
    if sel.from.is_empty() {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // FOR UPDATE / FOR SHARE / FOR NO KEY UPDATE / FOR KEY SHARE.
    let lock_at = ["FOR UPDATE", "FOR SHARE", "FOR NO KEY UPDATE", "FOR KEY SHARE"]
      .iter()
      .filter_map(|kw| upper.find(kw).map(|i| (i, *kw)))
      .min_by_key(|(i, _)| *i);
    let Some((lock_pos, kw)) = lock_at else { return };
    // Any FROM target a view?
    let view_target = sel.from.iter().find_map(|tr| {
      catalog
        .find_table(tr.schema.as_deref(), &tr.name)
        .filter(|t| matches!(t.kind, TableKind::View | TableKind::MaterializedView))
    });
    let Some(view) = view_target else { return };
    let abs_s = start + lock_pos;
    let abs_e = abs_s + kw.len();
    out.push(Diagnostic {
      code: "sql175",
      severity: Severity::Error,
      message: format!(
        "`{}` cannot be applied to view `{}.{}` -- views aren't lockable",
        kw, view.schema, view.name
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
