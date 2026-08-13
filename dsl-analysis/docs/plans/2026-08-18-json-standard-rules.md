# SQL-standard JSON rules (batch 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline, no subagents -- standing project preference). Process and global constraints match `dsl-analysis/docs/plans/2026-08-18-partitioning-ddl-rules.md` (batch 1) exactly; this doc covers only what's specific to batch 2.

**Goal:** Add 6 new dsl-analysis lint rules (sql763-sql768) covering SQL-standard JSON function misuse (`JSON_EXISTS`, `JSON_VALUE`, `JSON_QUERY`, `JSON_TABLE`, `IS JSON`), batch 2 of the SQL rule expansion plan.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 2 section; sql766 changed during planning -- see Deviation note).

## Deviation from spec: sql766

Spec's original sql766 (`json_table_no_columns`, empty `COLUMNS ()`) verified unreachable: `JSON_TABLE(doc, '$[*]' COLUMNS ())` is a hard pg_query grammar rejection (`sql000: syntax error at or near ")"`), same class of problem as batch 1's original sql760. Replacement, verified to parse cleanly: **`json_table_duplicate_column_name`** -- the same output column name defined twice in a `JSON_TABLE ... COLUMNS (...)` list.

Also found and fixed during probing: `JSON_VALUE(... RETURNING <type> ON ERROR NULL)` is wrong clause order -- PG's grammar is `NULL ON ERROR`, not `ON ERROR NULL`. Test snippets below use the corrected order.

## Task 1: sql763 json_exists_bad_path

**Files:** Create `dsl-analysis/src/rules/json_exists_bad_path.rs`; modify `dsl-analysis/src/rules/mod.rs`; test in new `dsl-analysis/tests/rules_json_standard.rs`.

- [ ] Write failing tests, implement, verify pass:

```rust
use dsl_analysis::{Severity, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Schema};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;

fn empty_cat() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &empty_cat())
}

#[test]
fn sql763_bad_path_no_dollar() {
  let d = diags("SELECT JSON_EXISTS(doc, 'not-a-path') FROM t763;");
  assert!(d.iter().any(|x| x.code == "sql763" && x.severity == Severity::Error));
}

#[test]
fn sql763_quiet_on_valid_path() {
  let d = diags("SELECT JSON_EXISTS(doc, '$.a') FROM t763;");
  assert!(!d.iter().any(|x| x.code == "sql763"));
}
```

Implementation:

```rust
//! sql763: `JSON_EXISTS(doc, 'literal')` where the literal path string
//! does not start with `$` -- not a valid SQL/JSON path expression.
//! PostgreSQL raises an error evaluating the path at runtime; the
//! parser accepts any string literal here since path validity isn't
//! checked until execution (verified empirically: no parse rejection).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql763"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_EXISTS") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_EXISTS".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_EXISTS".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((lit_start, lit_end)) = first_string_literal(body, p, close) {
        let content = &body[lit_start + 1..lit_end];
        let trimmed = content.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('$') {
          out.push(Diagnostic {
            code: "sql763",
            severity: Severity::Error,
            message: "JSON_EXISTS path does not start with `$` -- not a valid SQL/JSON path expression".into(),
            range: crate::range_at(start + lit_start, start + lit_end + 1),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// First single-quoted string literal's `(open_quote, close_quote)`
/// byte offsets strictly inside `(open+1, close)`.
fn first_string_literal(body: &str, open: usize, close: usize) -> Option<(usize, usize)> {
  let b = body.as_bytes();
  let mut i = open + 1;
  while i < close {
    if b[i] == b'\'' {
      let s = i;
      i += 1;
      while i < close && b[i] != b'\'' {
        i += 1;
      }
      if i < close {
        return Some((s, i));
      }
      return None;
    }
    i += 1;
  }
  None
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

## Task 2: sql764 json_value_returning_without_on_error

**Files:** Create `dsl-analysis/src/rules/json_value_returning_without_on_error.rs`; modify `mod.rs`; append tests.

```rust
#[test]
fn sql764_returning_without_on_error() {
  let d = diags("SELECT JSON_VALUE(doc, '$.a' RETURNING int) FROM t764;");
  assert!(d.iter().any(|x| x.code == "sql764" && x.severity == Severity::Hint));
}

#[test]
fn sql764_quiet_with_on_error() {
  let d = diags("SELECT JSON_VALUE(doc, '$.a' RETURNING int NULL ON ERROR) FROM t764;");
  assert!(!d.iter().any(|x| x.code == "sql764"));
}
```

```rust
//! sql764: `JSON_VALUE(... RETURNING <type>)` narrowing the return
//! type with no `ON ERROR` clause. If the extracted value doesn't
//! convert to the target type, JSON_VALUE raises an unhandled runtime
//! error; an explicit `NULL ON ERROR` / `DEFAULT ... ON ERROR` avoids
//! that. Hint-level: valid SQL, just a missing safety net.

use crate::clause_scan::is_word;
use crate::textutil::find_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql764"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_VALUE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_VALUE".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_VALUE".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let call = &upper[p..=close];
      if let Some(ret_rel) = find_word(call, "RETURNING")
        && !call.contains("ON ERROR")
      {
        let abs = p + ret_rel;
        out.push(Diagnostic {
          code: "sql764",
          severity: Severity::Hint,
          message: "JSON_VALUE narrows the return type but has no ON ERROR clause -- a conversion failure raises an unhandled runtime error".into(),
          range: crate::range_at(start + abs, start + abs + "RETURNING".len()),
        });
      }
      i = close + 1;
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

## Task 3: sql765 json_query_wrapper_conflict

**Files:** Create `dsl-analysis/src/rules/json_query_wrapper_conflict.rs`; modify `mod.rs`; append tests.

```rust
#[test]
fn sql765_wrapper_omit_quotes_conflict() {
  let d = diags("SELECT JSON_QUERY(doc, '$.a' WITH WRAPPER OMIT QUOTES) FROM t765;");
  assert!(d.iter().any(|x| x.code == "sql765" && x.severity == Severity::Error));
}

#[test]
fn sql765_quiet_wrapper_alone() {
  let d = diags("SELECT JSON_QUERY(doc, '$.a' WITH WRAPPER) FROM t765;");
  assert!(!d.iter().any(|x| x.code == "sql765"));
}
```

```rust
//! sql765: `JSON_QUERY(... WITH WRAPPER ... OMIT QUOTES)` -- OMIT
//! QUOTES is disallowed together with a wrapper (PostgreSQL raises an
//! error at query time; the parser accepts the combination -- verified
//! empirically). Only the plain `WITH WRAPPER` form is covered --
//! `WITH CONDITIONAL/UNCONDITIONAL [ARRAY] WRAPPER` variants are out
//! of scope to keep detection conservative.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql765"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_QUERY") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_QUERY".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_QUERY".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let call = &upper[p..=close];
      if call.contains("WITH WRAPPER") && call.contains("OMIT QUOTES") {
        out.push(Diagnostic {
          code: "sql765",
          severity: Severity::Error,
          message: "OMIT QUOTES cannot be used together with WITH WRAPPER in JSON_QUERY".into(),
          range: crate::range_at(start + p, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

## Task 4: sql766 json_table_duplicate_column_name

**Files:** Create `dsl-analysis/src/rules/json_table_duplicate_column_name.rs`; modify `mod.rs`; append tests.

```rust
#[test]
fn sql766_duplicate_output_column() {
  let d = diags(
    "SELECT * FROM t766, JSON_TABLE(doc, '$[*]' COLUMNS (a int PATH '$.a', a text PATH '$.b')) AS jt;",
  );
  assert!(d.iter().any(|x| x.code == "sql766" && x.severity == Severity::Warning));
}

#[test]
fn sql766_quiet_when_distinct() {
  let d = diags(
    "SELECT * FROM t766, JSON_TABLE(doc, '$[*]' COLUMNS (a int PATH '$.a', b text PATH '$.b')) AS jt;",
  );
  assert!(!d.iter().any(|x| x.code == "sql766"));
}
```

```rust
//! sql766: `JSON_TABLE(... COLUMNS (a ..., a ...))` -- the same output
//! column name used twice. PostgreSQL rejects duplicate JSON_TABLE
//! column names. Replaces the spec's original sql766 (empty COLUMNS
//! list), which pg_query rejects as a hard parse error before any
//! LintRule sees it -- verified empirically.

use crate::clause_scan::{find_clause, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql766"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_TABLE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_TABLE".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_TABLE".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // Search strictly INSIDE the call's own parens so `COLUMNS`
      // registers at depth 0 relative to this inner slice.
      let inner = &upper[p + 1..close];
      if let Some(cols_rel) = find_clause(inner.as_bytes(), b"COLUMNS") {
        let cols_abs = p + 1 + cols_rel;
        let paren_pos = skip_ws(ub, cols_abs + "COLUMNS".len());
        if ub.get(paren_pos) == Some(&b'(')
          && let Some(cols_close) = match_paren(ub, paren_pos)
        {
          let list = &body[paren_pos + 1..cols_close];
          let mut seen: Vec<String> = Vec::new();
          for (entry, off) in split_top_level(list) {
            let Some(name) = leading_ident(entry) else { continue };
            if seen.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
              let lead = entry.len() - entry.trim_start().len();
              let abs = paren_pos + 1 + off + lead;
              out.push(Diagnostic {
                code: "sql766",
                severity: Severity::Warning,
                message: format!("JSON_TABLE output column `{name}` is defined more than once"),
                range: crate::range_at(start + abs, start + abs + name.len()),
              });
            } else {
              seen.push(name);
            }
          }
        }
      }
      i = close + 1;
    }
  }
}

/// Leading identifier of a JSON_TABLE column-def entry (bare or
/// double-quoted), ignoring the rest of the entry (`int PATH '...'`).
fn leading_ident(s: &str) -> Option<String> {
  let t = s.trim_start();
  if let Some(rest) = t.strip_prefix('"') {
    let end = rest.find('"')?;
    return Some(rest[..end].to_string());
  }
  let end = t.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(t.len());
  if end == 0 {
    return None;
  }
  Some(t[..end].to_string())
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !crate::clause_scan::is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !crate::clause_scan::is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

## Task 5: sql767 is_json_redundant_with_jsonb_column

**Files:** Create `dsl-analysis/src/rules/is_json_redundant_with_jsonb_column.rs`; modify `mod.rs`; append tests. Uses a catalog fixture (not `empty_cat()`) since this rule needs a real column type -- add a second local helper in the test file.

```rust
fn cat_with_jsonb_col() -> Catalog {
  use dsl_catalog::{Column, Table, TableKind};
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema {
      name: "public".into(),
      tables: vec![Table {
        schema: "public".into(),
        name: "t767".into(),
        kind: TableKind::Table,
        columns: vec![Column {
          name: "doc".into(),
          data_type: "jsonb".into(),
          nullable: true,
          default: None,
          comment: None,
          generated: None,
          json_keys: None,
        }],
        constraints: vec![],
        indexes: vec![],
        triggers: vec![],
        policies: vec![],
        comment: None,
        row_estimate: None,
        owner: None,
        definition: None,
        strict: false,
        options: None,
      }],
    }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags_with_cat(src: &str, cat: &Catalog) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, cat)
}

#[test]
fn sql767_redundant_on_jsonb_column() {
  let d = diags_with_cat("SELECT doc FROM t767 WHERE doc IS JSON;", &cat_with_jsonb_col());
  assert!(d.iter().any(|x| x.code == "sql767" && x.severity == Severity::Hint));
}

#[test]
fn sql767_quiet_without_catalog() {
  // No catalog info for `doc` -- rule must stay quiet, not guess.
  let d = diags("SELECT doc FROM t767 WHERE doc IS JSON;");
  assert!(!d.iter().any(|x| x.code == "sql767"));
}

#[test]
fn sql767_quiet_on_is_json_object() {
  // Narrower IS JSON OBJECT/ARRAY/SCALAR checks are out of scope.
  let d = diags_with_cat("SELECT doc FROM t767 WHERE doc IS JSON OBJECT;", &cat_with_jsonb_col());
  assert!(!d.iter().any(|x| x.code == "sql767"));
}
```

```rust
//! sql767: `col IS JSON` where the catalog already types `col` as
//! `json` or `jsonb` -- always true, the predicate is redundant.
//! `IS JSON OBJECT`/`ARRAY`/`SCALAR` are narrower checks and are left
//! alone (being jsonb-typed doesn't guarantee a specific JSON kind).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql767"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() || catalog.tables().next().is_none() {
      return;
    }
    let (start, body) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(body);
    let bytes = cleaned.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      let Some(rel) = find_word_ci(bytes, i, b"IS") else { break };
      let after = skip_ws(bytes, rel + 2);
      if !word_at(bytes, after, b"JSON") {
        i = rel + 2;
        continue;
      }
      let after_json = skip_ws(bytes, after + 4);
      if word_at(bytes, after_json, b"OBJECT") || word_at(bytes, after_json, b"ARRAY") || word_at(bytes, after_json, b"SCALAR") {
        i = rel + 2;
        continue;
      }
      let mut left_end = rel;
      while left_end > 0 && bytes[left_end - 1].is_ascii_whitespace() {
        left_end -= 1;
      }
      let mut left_start = left_end;
      while left_start > 0 {
        let b = bytes[left_start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
          left_start -= 1;
        } else {
          break;
        }
      }
      let col_text = &cleaned[left_start..left_end];
      if !col_text.is_empty() {
        let (qualifier, name) = match col_text.split_once('.') {
          Some((q, nm)) => (Some(q), nm),
          None => (None, col_text),
        };
        if let Some(ty) = lookup_column_type(scope, catalog, qualifier, name) {
          let bare = ty.trim().to_ascii_lowercase();
          if bare == "json" || bare == "jsonb" {
            out.push(Diagnostic {
              code: "sql767",
              severity: Severity::Hint,
              message: format!("`{col_text} IS JSON` is always true -- column is already `{ty}`"),
              range: crate::range_at(start + left_start, start + after + 4),
            });
          }
        }
      }
      i = after + 4;
    }
  }
}

fn lookup_column_type(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> Option<String> {
  if let Some(q) = qualifier {
    if let Some((schema, table)) = q.split_once('.')
      && let Some(t) = catalog.find_table(Some(schema), table)
    {
      return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.data_type.clone());
    }
    if let Some(b) = scope.get(q)
      && let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name)
    {
      return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.data_type.clone());
    }
    return None;
  }
  let mut hit: Option<String> = None;
  for b in scope.tables() {
    let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) else { continue };
    for c in &t.columns {
      if c.name.eq_ignore_ascii_case(name) {
        if hit.is_some() {
          return None;
        }
        hit = Some(c.data_type.clone());
      }
    }
  }
  hit
}

fn find_word_ci(bytes: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= bytes.len() {
    if bytes[i..i + w.len()].eq_ignore_ascii_case(w) && word_at(bytes, i, w) {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn word_at(bytes: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= bytes.len()
    && bytes[i..i + w.len()].eq_ignore_ascii_case(w)
    && (i == 0 || !is_word(bytes[i - 1] as char))
    && (i + w.len() == bytes.len() || !is_word(bytes[i + w.len()] as char))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

## Task 6: sql768 is_json_scalar_object_conflict

**Files:** Create `dsl-analysis/src/rules/is_json_scalar_object_conflict.rs`; modify `mod.rs`; append tests.

```rust
#[test]
fn sql768_object_and_array_conflict() {
  let d = diags("SELECT doc FROM t768 WHERE doc IS JSON OBJECT AND doc IS JSON ARRAY;");
  assert!(d.iter().any(|x| x.code == "sql768" && x.severity == Severity::Warning));
}

#[test]
fn sql768_quiet_single_check() {
  let d = diags("SELECT doc FROM t768 WHERE doc IS JSON OBJECT;");
  assert!(!d.iter().any(|x| x.code == "sql768"));
}
```

```rust
//! sql768: `<expr> IS JSON OBJECT AND <same expr> IS JSON ARRAY` (or
//! any two different IS JSON kinds directly ANDed together) -- a JSON
//! value is exactly one of object/array/scalar, so requiring two
//! different kinds of the same expression is always false. Only the
//! direct `x IS JSON K1 AND x IS JSON K2` adjacency is matched
//! (nothing else between the two checks) to stay conservative.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const KINDS: &[&str] = &["OBJECT", "ARRAY", "SCALAR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql768"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      let Some(hit1) = match_expr_is_json(ub, i) else { break };
      let mut j = skip_ws(ub, hit1.end);
      if word_at(ub, j, b"AND") {
        j = skip_ws(ub, j + 3);
        if let Some(hit2) = match_expr_is_json_at(ub, j)
          && hit1.expr(ub).eq_ignore_ascii_case(hit2.expr(ub))
          && hit1.kind != hit2.kind
        {
          out.push(Diagnostic {
            code: "sql768",
            severity: Severity::Warning,
            message: format!(
              "`{}` cannot be both IS JSON {} and IS JSON {} -- always false",
              hit1.expr(ub),
              hit1.kind,
              hit2.kind
            ),
            range: crate::range_at(start + hit1.expr_start, start + hit2.end),
          });
        }
      }
      i = hit1.end;
    }
  }
}

struct Hit {
  expr_start: usize,
  expr_end: usize,
  kind: &'static str,
  end: usize,
}

impl Hit {
  fn expr<'a>(&self, ub: &'a [u8]) -> &'a str {
    std::str::from_utf8(&ub[self.expr_start..self.expr_end]).unwrap_or("")
  }
}

/// Scan forward from `from` for the next `<ident> IS JSON <KIND>`.
fn match_expr_is_json(ub: &[u8], from: usize) -> Option<Hit> {
  let mut i = from;
  while i < ub.len() {
    if is_ident_byte(ub[i]) && (i == 0 || !is_ident_byte(ub[i - 1])) {
      if let Some(h) = match_expr_is_json_at(ub, i) {
        return Some(h);
      }
    }
    i += 1;
  }
  None
}

/// `<ident> IS JSON <KIND>` starting exactly at `at`, or None.
fn match_expr_is_json_at(ub: &[u8], at: usize) -> Option<Hit> {
  if !ub.get(at).is_some_and(|b| is_ident_byte(*b)) {
    return None;
  }
  let mut e = at;
  while e < ub.len() && is_ident_byte(ub[e]) {
    e += 1;
  }
  let after = skip_ws(ub, e);
  if !word_at(ub, after, b"IS") {
    return None;
  }
  let after_is = skip_ws(ub, after + 2);
  if !word_at(ub, after_is, b"JSON") {
    return None;
  }
  let after_json = skip_ws(ub, after_is + 4);
  let kind = KINDS.iter().find(|k| word_at(ub, after_json, k.as_bytes())).copied()?;
  Some(Hit { expr_start: at, expr_end: e, kind, end: after_json + kind.len() })
}

fn is_ident_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
```

## Final step: verify and commit

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/json_exists_bad_path.rs \
        dsl-analysis/src/rules/json_value_returning_without_on_error.rs \
        dsl-analysis/src/rules/json_query_wrapper_conflict.rs \
        dsl-analysis/src/rules/json_table_duplicate_column_name.rs \
        dsl-analysis/src/rules/is_json_redundant_with_jsonb_column.rs \
        dsl-analysis/src/rules/is_json_scalar_object_conflict.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_json_standard.rs \
        dsl-analysis/docs/plans/2026-08-18-json-standard-rules.md
git commit -m "feat(analysis): flag SQL-standard JSON function misuse"
```
