//! sql513: function call arg-count validation.
//!
//! Text-scans the statement body for `<name>(...)` invocations, looks
//! the function up in the catalog, and warns when the arity doesn't
//! match the declared signature:
//!
//!   - too few args (less than required) -> warning
//!   - too many args (more than declared, non-variadic) -> warning
//!
//! Why text scan instead of AST: the pg_query backend flattens
//! FuncCall args into their column refs and does not emit Expr::Call.
//! A text scan with paren tracking captures every call site reliably,
//! including nested calls and cast-expression contexts.
//!
//! Empty catalog (no live DB + no offline-derived functions) -> silent.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::{Catalog, Function};
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql513"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if catalog.functions.is_empty() {
      return;
    }
    let (start, raw) = crate::stmt_body(stmt, source);
    // We use TWO views of the body:
    //   - `body` (stripped of strings + comments) for keyword detection
    //     and for paren walks that must not treat `'` as syntax.
    //   - `raw_body` (untouched) for literal classification per arg.
    // Without the second view, `pow2('three')` would tokenise as
    // `pow2()` (string body wiped to spaces) and we'd lose the chance
    // to classify the literal at all.
    let body_owned = crate::textutil::strip_comments_strings(raw);
    let body = body_owned.as_str();
    let raw_body = raw;
    let bytes = body.as_bytes();
    let n = bytes.len();
    // Walk every `<ident>(` pattern. Skip if `<ident>` is a keyword
    // already known not to be a function (LIKE keyword shapes).
    let mut i = 0usize;
    while i < n {
      if !is_ident_start(bytes[i]) {
        i += 1;
        continue;
      }
      let id_start = i;
      while i < n && is_ident_char(bytes[i]) {
        i += 1;
      }
      let id_end = i;
      // Support schema-qualified `app.foo(...)` by greedily reading
      // an extra `.ident` if present.
      let mut full_name_end = id_end;
      if i < n && bytes[i] == b'.' {
        let dot = i;
        i += 1;
        if i < n && is_ident_start(bytes[i]) {
          while i < n && is_ident_char(bytes[i]) {
            i += 1;
          }
          full_name_end = i;
        } else {
          i = dot;
        }
      }
      // Skip whitespace before `(`.
      let mut k = i;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        continue;
      }
      let name = &body[id_start..full_name_end];
      // Filter out PG keywords + control structures that take the
      // `kw(...)` shape but are NOT user-callable functions.
      if is_keyword_call(name) {
        i = k + 1;
        continue;
      }
      // Find matching `)`.
      let close = match find_matching_paren(bytes, k) {
        Some(c) => c,
        None => break,
      };
      // Tokenise the arg list at top-level commas using the RAW body
      // so single-quoted strings stay intact for literal classification.
      let raw_arg_body = &raw_body[k + 1..close];
      let call_args = count_top_level_args(raw_arg_body);
      let arg_exprs = split_top_level_args(raw_arg_body);
      // Resolve the function name.
      let bare = name.rsplit('.').next().unwrap_or(name);
      let schema = name.rsplit_once('.').map(|p| p.0);
      let candidates: Vec<&Function> = catalog
        .functions
        .iter()
        .filter(|f| {
          f.name.eq_ignore_ascii_case(bare)
            && match schema {
              Some(s) => f.schema.eq_ignore_ascii_case(s),
              None => true,
            }
        })
        .collect();
      if !candidates.is_empty() {
        // Best-effort: any candidate signature that fits the call
        // arity -> silent.
        let mut any_fit = false;
        for f in &candidates {
          let declared = f.arguments.len();
          let defaults = f.arguments.iter().filter(|a| a.data_type.to_ascii_uppercase().contains("DEFAULT")).count();
          let variadic = f.arguments.iter().any(|a| a.data_type.to_ascii_uppercase().contains("VARIADIC"));
          let min = declared.saturating_sub(defaults);
          let max = if variadic { usize::MAX } else { declared };
          if call_args >= min && call_args <= max {
            any_fit = true;
            break;
          }
        }
        if !any_fit {
          let best = candidates
            .iter()
            .min_by_key(|f| (f.arguments.len() as i64 - call_args as i64).abs())
            .copied()
            .unwrap();
          let declared = best.arguments.len();
          let defaults = best.arguments.iter().filter(|a| a.data_type.to_ascii_uppercase().contains("DEFAULT")).count();
          let min_req = declared.saturating_sub(defaults);
          let msg = if call_args < min_req {
            format!(
              "function `{name}` requires {min_req} argument{}; called with {call_args}",
              if min_req == 1 { "" } else { "s" }
            )
          } else {
            format!(
              "function `{name}` accepts at most {declared} argument{}; called with {call_args}",
              if declared == 1 { "" } else { "s" }
            )
          };
          out.push(Diagnostic {
            code: "sql513",
            severity: Severity::Warning,
            message: msg,
            range: crate::range_at(start + id_start, start + full_name_end),
          });
        } else {
          // Arity fits. Check literal-type mismatch per position.
          // Only fires for unambiguous catalog matches with known types.
          let single_match = candidates.iter().find(|f| {
            let declared = f.arguments.len();
            let defaults = f.arguments.iter().filter(|a| a.data_type.to_ascii_uppercase().contains("DEFAULT")).count();
            let variadic = f.arguments.iter().any(|a| a.data_type.to_ascii_uppercase().contains("VARIADIC"));
            let min = declared.saturating_sub(defaults);
            let max = if variadic { usize::MAX } else { declared };
            call_args >= min && call_args <= max
          });
          if let Some(f) = single_match {
            for (pos, arg_text) in arg_exprs.iter().enumerate() {
              let Some(declared_arg) = f.arguments.get(pos) else { break };
              let kind = classify_literal(arg_text.trim());
              let declared_kind = classify_pg_type(&declared_arg.data_type);
              if let (Some(lit), Some(decl)) = (kind, declared_kind)
                && lit != decl
                && literal_decl_incompatible(lit, decl)
              {
                let arg_label = declared_arg.name.as_deref().unwrap_or("?");
                let msg = format!(
                  "function `{name}` argument {pos_plus_one} (`{arg_label}` {decl_ty}): literal looks like {lit_ty}, expected {decl_ty}",
                  pos_plus_one = pos + 1,
                  decl_ty = lit_kind_label(decl),
                  lit_ty = lit_kind_label(lit),
                );
                out.push(Diagnostic {
                  code: "sql513",
                  severity: Severity::Hint,
                  message: msg,
                  range: crate::range_at(start + id_start, start + full_name_end),
                });
              }
            }
          }
        }
      }
      i = close + 1;
    }
  }
}

fn is_ident_start(b: u8) -> bool {
  b.is_ascii_alphabetic() || b == b'_'
}
fn is_ident_char(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

/// PG reserved keywords used in `kw(...)` form that are NOT user
/// functions. These are language constructs (CAST, COALESCE, etc.
/// ARE catalog functions and SHOULD be checked, so they're not here).
fn is_keyword_call(name: &str) -> bool {
  let u = name.to_ascii_uppercase();
  matches!(
    u.as_str(),
    "SELECT"
      | "FROM"
      | "WHERE"
      | "IF"
      | "CASE"
      | "WHEN"
      | "AND"
      | "OR"
      | "NOT"
      | "IN"
      | "EXISTS"
      | "BETWEEN"
      | "LIKE"
      | "ILIKE"
      | "IS"
      | "AS"
      | "ON"
      | "DECIMAL"
      | "NUMERIC"
      | "VARCHAR"
      | "CHAR"
      | "TIMESTAMP"
      | "TIMESTAMPTZ"
      | "INTERVAL"
      | "BIT"
      | "ARRAY"
  )
}

fn find_matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 1i32;
  let mut i = open + 1;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn count_top_level_args(body: &str) -> usize {
  if body.trim().is_empty() {
    return 0;
  }
  let bytes = body.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut in_single = false;
  for &b in bytes {
    if b == b'\'' {
      in_single = !in_single;
      continue;
    }
    if in_single {
      continue;
    }
    match b {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => commas += 1,
      _ => {},
    }
  }
  commas + 1
}

fn split_top_level_args(body: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut in_single = false;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    let b = bytes[i];
    if b == b'\'' {
      in_single = !in_single;
    } else if !in_single {
      match b {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b',' if depth == 0 => {
          out.push(body[start..i].to_string());
          start = i + 1;
        },
        _ => {},
      }
    }
    i += 1;
  }
  if start < n {
    out.push(body[start..n].to_string());
  } else if body.trim_end().ends_with(',') {
    out.push(String::new());
  }
  out
}

/// Coarse type categories used to detect mismatched literal calls.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LitKind {
  Int,
  Float,
  Text,
  Bool,
  Null,
}

fn lit_kind_label(k: LitKind) -> &'static str {
  match k {
    LitKind::Int => "integer",
    LitKind::Float => "numeric",
    LitKind::Text => "text",
    LitKind::Bool => "boolean",
    LitKind::Null => "null",
  }
}

/// Classify a single argument expression as a recognised literal. Only
/// the trivial shapes (numeric, single-quoted string, TRUE/FALSE/NULL)
/// fire; anything else (column ref, function call, cast, paren expr)
/// returns None so the rule stays silent.
fn classify_literal(s: &str) -> Option<LitKind> {
  let t = s.trim();
  if t.is_empty() {
    return None;
  }
  let upper = t.to_ascii_uppercase();
  if upper == "NULL" {
    return Some(LitKind::Null);
  }
  if upper == "TRUE" || upper == "FALSE" {
    return Some(LitKind::Bool);
  }
  // Single-quoted string. Must start AND end with `'`, no embedded `'`
  // except an escaped `''` pair. Includes E'...' and U&'...' prefix.
  let body = t.strip_prefix("E'").or_else(|| t.strip_prefix("e'")).or_else(|| t.strip_prefix('\'')).unwrap_or(t);
  if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
    let _ = body;
    return Some(LitKind::Text);
  }
  // Integer / float.
  if t.parse::<i64>().is_ok() {
    return Some(LitKind::Int);
  }
  if t.parse::<f64>().is_ok() {
    return Some(LitKind::Float);
  }
  None
}

/// Classify a PG type name into a coarse category.
fn classify_pg_type(ty: &str) -> Option<LitKind> {
  // Strip everything after a `(` (modifiers like varchar(255)).
  let bare = ty.split('(').next().unwrap_or(ty).trim();
  let bare_upper = bare.to_ascii_uppercase();
  // Strip DEFAULT/VARIADIC/IN/OUT/INOUT noise the offline parser may have left in.
  let bare = bare_upper
    .strip_prefix("VARIADIC ").unwrap_or(&bare_upper)
    .split_whitespace()
    .next()
    .unwrap_or("");
  match bare {
    "INT" | "INT4" | "INT2" | "INT8" | "INTEGER" | "BIGINT" | "SMALLINT" | "SERIAL" | "BIGSERIAL" | "OID" => {
      Some(LitKind::Int)
    },
    "NUMERIC" | "DECIMAL" | "REAL" | "DOUBLE" | "FLOAT" | "FLOAT4" | "FLOAT8" | "MONEY" => Some(LitKind::Float),
    "TEXT" | "VARCHAR" | "CHAR" | "CHARACTER" | "CITEXT" | "NAME" => Some(LitKind::Text),
    "BOOLEAN" | "BOOL" => Some(LitKind::Bool),
    _ => None,
  }
}

/// True when calling with `lit` for a `decl`-typed arg is a definite
/// type error PG would reject. Numeric->numeric and any->null are
/// compatible. Text<->int is the canonical "wrong literal" case.
fn literal_decl_incompatible(lit: LitKind, decl: LitKind) -> bool {
  if lit == LitKind::Null {
    return false;
  }
  matches!(
    (lit, decl),
    (LitKind::Text, LitKind::Int)
      | (LitKind::Text, LitKind::Float)
      | (LitKind::Text, LitKind::Bool)
      | (LitKind::Int, LitKind::Text)
      | (LitKind::Float, LitKind::Text)
      | (LitKind::Bool, LitKind::Text)
      | (LitKind::Bool, LitKind::Int)
      | (LitKind::Int, LitKind::Bool)
  )
}

// Re-use the textutil is_word indirectly to suppress unused warning.
#[allow(dead_code)]
fn _u(c: char) -> bool {
  is_word(c)
}
