//! Indexed views over [`Catalog`](crate::model::Catalog).
//!
//! These are the hot lookups used by completion / hover / analysis. Kept
//! as inherent methods on `Catalog` for ergonomic call sites:
//! `catalog.find_table(Some("public"), "users")`.

use crate::model::{Catalog, Column, Extension, IndexDef, Policy, Sequence, Table, Trigger, Type};

impl Catalog {
    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.schemas.iter().flat_map(|s| s.tables.iter())
    }

    /// User-defined types (enum / domain / composite). Consumed by
    /// completion and hover to surface custom types alongside built-ins.
    pub fn types(&self) -> impl Iterator<Item = &Type> {
        self.types.iter()
    }

    pub fn find_table(&self, schema: Option<&str>, name: &str) -> Option<&Table> {
        self.tables().find(|t| {
            t.name == name && schema.is_none_or(|s| t.schema == s)
        })
    }

    /// Find every (table, column) pair where the column has the given name.
    /// Multiple results indicate ambiguity (rule sql003).
    pub fn columns_named(&self, name: &str) -> Vec<(&Table, &Column)> {
        let mut out = Vec::new();
        for t in self.tables() {
            for c in &t.columns {
                if c.name == name {
                    out.push((t, c));
                }
            }
        }
        out
    }

    pub fn column_in(&self, schema: Option<&str>, table: &str, column: &str) -> Option<&Column> {
        let t = self.find_table(schema, table)?;
        t.columns.iter().find(|c| c.name == column)
    }

    /// Look up a user-defined type by name (enum / domain / composite).
    pub fn find_type(&self, schema: Option<&str>, name: &str) -> Option<&Type> {
        self.types().find(|t| {
            t.name == name && schema.is_none_or(|s| t.schema == s)
        })
    }

    /// Find a row-level security policy by name, plus its target table.
    /// Policies live on tables in the model, so the lookup scans every
    /// table's policy list.
    pub fn find_policy(&self, name: &str) -> Option<(&Table, &Policy)> {
        for t in self.tables() {
            if let Some(p) = t.policies.iter().find(|p| p.name == name) {
                return Some((t, p));
            }
        }
        None
    }

    /// Find a trigger by name, plus its target table.
    pub fn find_trigger(&self, name: &str) -> Option<(&Table, &Trigger)> {
        for t in self.tables() {
            if let Some(tr) = t.triggers.iter().find(|tr| tr.name == name) {
                return Some((t, tr));
            }
        }
        None
    }

    /// Find an index by name, plus its target table.
    pub fn find_index(&self, name: &str) -> Option<(&Table, &IndexDef)> {
        for t in self.tables() {
            if let Some(i) = t.indexes.iter().find(|i| i.name == name) {
                return Some((t, i));
            }
        }
        None
    }

    /// All known sequences, across schemas.
    pub fn sequences(&self) -> impl Iterator<Item = &Sequence> {
        self.sequences.iter()
    }

    /// Find a sequence by name (and optional schema).
    pub fn find_sequence(&self, schema: Option<&str>, name: &str) -> Option<&Sequence> {
        self.sequences().find(|s| {
            s.name == name && schema.is_none_or(|sch| s.schema == sch)
        })
    }

    /// All installed extensions.
    pub fn extensions(&self) -> impl Iterator<Item = &Extension> {
        self.extensions.iter()
    }

    /// True when an extension with `name` is installed (case-insensitive).
    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|e| e.name.eq_ignore_ascii_case(name))
    }
}
