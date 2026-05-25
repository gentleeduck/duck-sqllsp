//! PL/pgSQL function body parser.
//!
//! Wraps `pg_query::parse_plpgsql` (libpg_query FFI). Returns a typed
//! `PlpgsqlBody` so downstream rules can walk the statement list
//! without re-parsing the JSON. Falls back to `Err` on parse failure
//! or when the `pg_query_backend` feature is off -- callers must be
//! tolerant of that.
//!
//! The JSON shape libpg_query emits is documented in
//! `pl_funcs.c::dump_block`; we mirror the subset most rules care
//! about (statement kind + line + body-relative byte offset) and
//! drop the rest into `raw` for opaque inspection.

/// One statement inside a PL/pgSQL function body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlpgsqlStmt {
  pub kind: PlpgsqlStmtKind,
  /// 1-based source line where the statement starts. None when
  /// libpg_query didn't emit a location.
  pub lineno: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlpgsqlStmtKind {
  Block,
  Assign,
  If,
  Case,
  Loop,
  While,
  For,
  ForEach,
  Exit,
  Return,
  ReturnNext,
  ReturnQuery,
  Raise,
  Assert,
  Perform,
  ExecSql,
  ExecuteSql,
  GetDiag,
  Open,
  Fetch,
  Close,
  Commit,
  Rollback,
  Call,
  Other(String),
}

#[derive(Debug, Clone, Default)]
pub struct PlpgsqlBody {
  pub statements: Vec<PlpgsqlStmt>,
}

/// Parse a `CREATE FUNCTION ... LANGUAGE plpgsql AS $$ ... $$` body.
/// Pass the full CREATE FUNCTION text -- libpg_query expects the
/// surrounding DDL because the function header drives parameter +
/// return-type wiring.
#[cfg(feature = "pg_query_backend")]
pub fn parse(create_function_text: &str) -> Result<PlpgsqlBody, String> {
  let json = pg_query::parse_plpgsql(create_function_text).map_err(|e| e.to_string())?;
  Ok(walk_top(&json))
}

#[cfg(not(feature = "pg_query_backend"))]
pub fn parse(_: &str) -> Result<PlpgsqlBody, String> {
  Err("pg_query_backend feature is disabled -- rebuild with default-features".to_string())
}

#[cfg(feature = "pg_query_backend")]
fn walk_top(value: &serde_json::Value) -> PlpgsqlBody {
  let mut out = PlpgsqlBody::default();
  // Top level is an array of function objects: [{"PLpgSQL_function": { action: {...} }}]
  let Some(arr) = value.as_array() else { return out };
  for func in arr {
    let Some(action) = func
      .get("PLpgSQL_function")
      .and_then(|f| f.get("action"))
    else { continue };
    walk_stmt(action, &mut out.statements);
  }
  out
}

#[cfg(feature = "pg_query_backend")]
fn walk_stmt(node: &serde_json::Value, out: &mut Vec<PlpgsqlStmt>) {
  let Some(obj) = node.as_object() else { return };
  // Each wrapper has exactly one key naming the stmt kind.
  for (kind_key, body) in obj {
    let kind = classify(kind_key);
    let lineno = body.get("lineno").and_then(|n| n.as_u64()).map(|n| n as u32);
    out.push(PlpgsqlStmt { kind: kind.clone(), lineno });
    // Recurse into structural children that hold nested statements.
    if let Some(b) = body.get("body").and_then(|b| b.as_array()) {
      for child in b { walk_stmt(child, out); }
    }
    if let Some(b) = body.get("then_body").and_then(|b| b.as_array()) {
      for child in b { walk_stmt(child, out); }
    }
    if let Some(b) = body.get("else_body").and_then(|b| b.as_array()) {
      for child in b { walk_stmt(child, out); }
    }
    if let Some(arms) = body.get("elsif_list").and_then(|a| a.as_array()) {
      for arm in arms {
        if let Some(stmts) = arm.get("stmts").and_then(|s| s.as_array()) {
          for child in stmts { walk_stmt(child, out); }
        }
      }
    }
    if let Some(arms) = body.get("case_when_list").and_then(|a| a.as_array()) {
      for arm in arms {
        if let Some(stmts) = arm.get("stmts").and_then(|s| s.as_array()) {
          for child in stmts { walk_stmt(child, out); }
        }
      }
    }
    if let Some(handlers) = body
      .get("exceptions")
      .and_then(|e| e.get("PLpgSQL_exception_block"))
      .and_then(|b| b.get("exc_list"))
      .and_then(|l| l.as_array())
    {
      for handler in handlers {
        let Some(exc) = handler.get("PLpgSQL_exception") else { continue };
        if let Some(stmts) = exc.get("action").and_then(|s| s.as_array()) {
          for child in stmts { walk_stmt(child, out); }
        }
      }
    }
  }
}

#[cfg(feature = "pg_query_backend")]
fn classify(key: &str) -> PlpgsqlStmtKind {
  match key {
    "PLpgSQL_stmt_block" => PlpgsqlStmtKind::Block,
    "PLpgSQL_stmt_assign" => PlpgsqlStmtKind::Assign,
    "PLpgSQL_stmt_if" => PlpgsqlStmtKind::If,
    "PLpgSQL_stmt_case" => PlpgsqlStmtKind::Case,
    "PLpgSQL_stmt_loop" => PlpgsqlStmtKind::Loop,
    "PLpgSQL_stmt_while" => PlpgsqlStmtKind::While,
    "PLpgSQL_stmt_fors" | "PLpgSQL_stmt_fori" | "PLpgSQL_stmt_forq" => PlpgsqlStmtKind::For,
    "PLpgSQL_stmt_foreach_a" => PlpgsqlStmtKind::ForEach,
    "PLpgSQL_stmt_exit" => PlpgsqlStmtKind::Exit,
    "PLpgSQL_stmt_return" => PlpgsqlStmtKind::Return,
    "PLpgSQL_stmt_return_next" => PlpgsqlStmtKind::ReturnNext,
    "PLpgSQL_stmt_return_query" => PlpgsqlStmtKind::ReturnQuery,
    "PLpgSQL_stmt_raise" => PlpgsqlStmtKind::Raise,
    "PLpgSQL_stmt_assert" => PlpgsqlStmtKind::Assert,
    "PLpgSQL_stmt_perform" => PlpgsqlStmtKind::Perform,
    "PLpgSQL_stmt_execsql" => PlpgsqlStmtKind::ExecSql,
    "PLpgSQL_stmt_dynexecute" => PlpgsqlStmtKind::ExecuteSql,
    "PLpgSQL_stmt_getdiag" => PlpgsqlStmtKind::GetDiag,
    "PLpgSQL_stmt_open" => PlpgsqlStmtKind::Open,
    "PLpgSQL_stmt_fetch" => PlpgsqlStmtKind::Fetch,
    "PLpgSQL_stmt_close" => PlpgsqlStmtKind::Close,
    "PLpgSQL_stmt_commit" => PlpgsqlStmtKind::Commit,
    "PLpgSQL_stmt_rollback" => PlpgsqlStmtKind::Rollback,
    "PLpgSQL_stmt_call" => PlpgsqlStmtKind::Call,
    other => PlpgsqlStmtKind::Other(other.to_string()),
  }
}

#[cfg(test)]
#[cfg(feature = "pg_query_backend")]
mod tests {
  use super::*;

  fn has(body: &PlpgsqlBody, k: PlpgsqlStmtKind) -> bool {
    body.statements.iter().any(|s| s.kind == k)
  }

  fn count_of(body: &PlpgsqlBody, k: PlpgsqlStmtKind) -> usize {
    body.statements.iter().filter(|s| s.kind == k).count()
  }

  #[test]
  fn parses_trivial_body() {
    let body = parse(
      "CREATE FUNCTION f() RETURNS int AS $$
       BEGIN
         RETURN 42;
       END $$ LANGUAGE plpgsql;",
    )
    .expect("parse");
    assert!(has(&body, PlpgsqlStmtKind::Block));
    assert!(has(&body, PlpgsqlStmtKind::Return));
  }

  #[test]
  fn walks_nested_if_branches() {
    let body = parse(
      "CREATE FUNCTION g(x int) RETURNS int AS $$
       BEGIN
         IF x > 0 THEN
           RAISE NOTICE 'pos';
         ELSE
           RAISE EXCEPTION 'neg';
         END IF;
         RETURN x;
       END $$ LANGUAGE plpgsql;",
    )
    .expect("parse");
    assert_eq!(count_of(&body, PlpgsqlStmtKind::Raise), 2, "both RAISE arms must be visited");
    assert!(has(&body, PlpgsqlStmtKind::If));
  }

  #[test]
  fn captures_loop_body() {
    let body = parse(
      "CREATE FUNCTION h() RETURNS int AS $$
       DECLARE i int := 0;
       BEGIN
         LOOP
           i := i + 1;
           EXIT WHEN i > 3;
         END LOOP;
         RETURN i;
       END $$ LANGUAGE plpgsql;",
    )
    .expect("parse");
    assert!(has(&body, PlpgsqlStmtKind::Loop));
    assert!(has(&body, PlpgsqlStmtKind::Assign));
    assert!(has(&body, PlpgsqlStmtKind::Exit));
  }

  #[test]
  fn captures_exception_handlers() {
    let body = parse(
      "CREATE FUNCTION e() RETURNS void AS $$
       BEGIN
         RAISE EXCEPTION 'boom';
       EXCEPTION WHEN OTHERS THEN
         RAISE NOTICE 'caught';
       END $$ LANGUAGE plpgsql;",
    )
    .expect("parse");
    let raise_count = body.statements.iter().filter(|s| s.kind == PlpgsqlStmtKind::Raise).count();
    assert_eq!(raise_count, 2, "main + handler RAISE must both appear");
  }
}
