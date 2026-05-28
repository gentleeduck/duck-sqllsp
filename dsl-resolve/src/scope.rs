//! The set of bindings visible inside a single statement.
//!
//! Looking up `users` after `FROM users u` returns the binding for the
//! `users` table either by alias `u` or by raw name `users`. The unaliased
//! form is added so qualified column refs like `users.id` resolve even
//! when the user never bothered to write an alias.

use crate::binding::Binding;
use indexmap::IndexMap;
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Scope {
  pub bindings: IndexMap<String, Binding>,
  /// CTE projection columns, keyed by CTE name. Empty `Vec` means the
  /// CTE is declared in this statement but the resolver did not (yet)
  /// inspect its body to learn its columns. Callers should treat
  /// `Some(empty)` as "exists but unknown" and `None` as "no such CTE".
  pub cte_columns: IndexMap<String, Vec<String>>,
}

impl Scope {
  /// Case-insensitive lookup by alias or bare table name. SQL folds
  /// unquoted identifiers (PG: lowercase, ANSI: upper), so the user
  /// typing `U.` must resolve a binding declared as `u`. The fast
  /// exact-match path runs first; a linear case-insensitive scan
  /// covers the cold path.
  pub fn get(&self, name: &str) -> Option<&Binding> {
    if let Some(b) = self.bindings.get(name) {
      return Some(b);
    }
    self.bindings.iter().find_map(|(k, v)| k.eq_ignore_ascii_case(name).then_some(v))
  }

  pub fn tables(&self) -> impl Iterator<Item = &Binding> {
    self.bindings.values()
  }

  pub fn len(&self) -> usize {
    self.bindings.len()
  }
  pub fn is_empty(&self) -> bool {
    self.bindings.is_empty()
  }

  /// CTE columns for `name`, in projection order. Returns `None` when
  /// the CTE was not declared in this scope; `Some(empty)` when it
  /// was declared but the resolver could not parse its body yet.
  /// Matches case-insensitively for the same reason as [`get`].
  pub fn cte_columns_of(&self, name: &str) -> Option<&Vec<String>> {
    if let Some(v) = self.cte_columns.get(name) {
      return Some(v);
    }
    self.cte_columns.iter().find_map(|(k, v)| k.eq_ignore_ascii_case(name).then_some(v))
  }
}
