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
}

impl Scope {
    pub fn get(&self, name: &str) -> Option<&Binding> {
        self.bindings.get(name)
    }

    pub fn tables(&self) -> impl Iterator<Item = &Binding> {
        self.bindings.values()
    }

    pub fn len(&self) -> usize { self.bindings.len() }
    pub fn is_empty(&self) -> bool { self.bindings.is_empty() }
}
