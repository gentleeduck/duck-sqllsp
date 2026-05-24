//! Name resolution and scope inference for duck-sqllsp.
//!
//! Given a parsed [`Statement`](dsl_parse::Statement), build a [`Scope`]
//! describing which tables and aliases are visible inside it. Downstream
//! completion + analysis crates consume scopes instead of re-deriving
//! alias maps every time.
//!
//! Surface stays intentionally small:
//!   - [`Scope`], [`Binding`] -- the resolved bindings inside one statement.
//!   - [`resolve`] -- resolve every statement in a parsed file.

pub mod binding;
pub mod scope;
pub mod resolver;

pub use binding::Binding;
pub use resolver::{resolve, resolve_with_source};
pub use scope::Scope;
