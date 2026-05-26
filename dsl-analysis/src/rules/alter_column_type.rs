//! sql189: `ALTER TABLE t ALTER COLUMN c TYPE <new_type>` where
//! `c`'s catalog type doesn't auto-cast to `<new_type>` and the
//! statement lacks `USING`. PG raises 42804 at runtime.
//!
//! Conservative: only flag when both source + target are known
//! type families AND the cast isn't trivially safe (same family,
//! widening numeric, etc).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql189"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(alter_at) = upper.find("ALTER TABLE") else { return };
    let after_alter = alter_at + "ALTER TABLE".len();
    let rest = &body[after_alter..];
    let lead = rest.len() - rest.trim_start().len();
    let raw = &rest[lead..];
    let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
    let table = raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_string();
    if table.is_empty() { return; }
    let Some(t) = catalog.find_table(None, &table) else { return };

    let Some(ac_at) = upper.find("ALTER COLUMN ") else { return };
    let after_ac = ac_at + "ALTER COLUMN ".len();
    let ac_rest = &body[after_ac..];
    let ac_lead = ac_rest.len() - ac_rest.trim_start().len();
    let ac_raw = &ac_rest[ac_lead..];
    let col_end = ac_raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '"').unwrap_or(ac_raw.len());
    let col_name = ac_raw[..col_end].trim_matches('"').to_string();
    let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(&col_name)) else { return };

    let Some(type_at) = upper.find(" TYPE ").or_else(|| upper.find(" SET DATA TYPE ")) else { return };
    let after_type = if upper[type_at..].starts_with(" SET DATA TYPE ") {
      type_at + " SET DATA TYPE ".len()
    } else {
      type_at + " TYPE ".len()
    };
    let type_rest = &body[after_type..];
    let new_type_end = type_rest
      .find(|c: char| c == ';' || c == ',' || c == '\n' || c == ' ')
      .unwrap_or(type_rest.len());
    let new_type = type_rest[..new_type_end].trim().to_ascii_uppercase();
    if new_type.is_empty() { return; }
    if upper.contains("USING ") { return; }

    let old = col.data_type.to_ascii_uppercase();
    let old_bare = old.rsplit('.').next().unwrap_or(&old).trim();
    let new_bare = new_type.rsplit('.').next().unwrap_or(&new_type).trim();
    if compatible(old_bare, new_bare) { return; }
    let abs_s = start + ac_at;
    let abs_e = start + after_type + new_type_end;
    out.push(Diagnostic {
      code: "sql189",
      severity: Severity::Warning,
      message: format!(
        "ALTER COLUMN `{}` from `{}` to `{}` without USING -- PG can't auto-cast, add `USING ({}::{})`",
        col.name, old_bare, new_bare, col.name, new_bare,
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn compatible(old: &str, new: &str) -> bool {
  if old.starts_with(new) || new.starts_with(old) {
    return true;
  }
  let numeric = ["INT", "INTEGER", "BIGINT", "SMALLINT", "INT4", "INT8", "INT2", "NUMERIC", "DECIMAL", "REAL", "DOUBLE", "FLOAT"];
  let strings = ["TEXT", "VARCHAR", "CHAR", "CHARACTER", "CITEXT", "NAME"];
  let in_numeric = numeric.iter().any(|t| old.starts_with(t)) && numeric.iter().any(|t| new.starts_with(t));
  let in_string = strings.iter().any(|t| old.starts_with(t)) && strings.iter().any(|t| new.starts_with(t));
  in_numeric || in_string
}
