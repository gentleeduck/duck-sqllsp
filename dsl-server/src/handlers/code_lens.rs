//! `textDocument/codeLens` handler.
//!
//! Emits a `Run` and `EXPLAIN` lens above every SELECT / INSERT /
//! UPDATE / DELETE statement. The lens command name (`duck-sqllsp.runQuery`,
//! `duck-sqllsp.explainQuery`) is what the editor binds to a Lua handler;
//! the LSP itself doesn't execute the query -- the editor uses dadbod
//! (or whatever the user has bound) to run the statement text returned
//! in `arguments`.

use crate::state::ServerState;
use dsl_parse::StatementKind;
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{CodeLens, CodeLensParams, Command, Position, Range};

pub fn run(state: &ServerState, params: CodeLensParams) -> Option<Vec<CodeLens>> {
  let _g = crate::handlers::perf::Guard::with_uri("code_lens", &params.text_document.uri);
  let doc = state.documents.get(&params.text_document.uri)?;
  let cache = doc.parsed();
  let live_catalog = state.catalog.read().clone();

  // Run / EXPLAIN / + LIMIT / EXPLAIN ANALYZE require a client-side
  // command handler. Only VS Code (+ its forks) ships one today; on
  // nvim / helix / etc the click pops "command not found". Suppress
  // those lenses on unsupported clients but keep the purely informational
  // `~N rows on <table>` row-count lens which uses `duck-sqllsp.noop`.
  let emit_runnable = state.client_supports_runnable_codelens();

  let mut out = Vec::new();
  for stmt in &cache.file.statements {
    let dml = matches!(
      &stmt.kind,
      StatementKind::Select(_) | StatementKind::Insert(_) | StatementKind::Update(_) | StatementKind::Delete(_)
    );
    // EXPLAIN only makes sense for DML; CREATE/ALTER/DROP can't be
    // explained. Run is now offered for every parsed statement so the
    // user can click any block (DDL, transaction control, DO blocks,
    // etc.) and execute it through the editor command pipeline.
    let range = to_lsp_range(&doc.rope, stmt.range);
    let text = slice_of(&doc.text, stmt.range);
    if emit_runnable {
      out.push(CodeLens {
        range,
        command: Some(Command {
          title: "Run".into(),
          command: "duck-sqllsp.runQuery".into(),
          arguments: Some(vec![serde_json::json!(text)]),
        }),
        data: None,
      });
      if dml {
        out.push(CodeLens {
          range,
          command: Some(Command {
            title: "EXPLAIN".into(),
            command: "duck-sqllsp.explainQuery".into(),
            arguments: Some(vec![serde_json::json!(text)]),
          }),
          data: None,
        });
      }
    }
    // Row-count + slow-query nudges only apply to DML; skip them on
    // CREATE / ALTER / DROP / etc.
    if !dml {
      continue;
    }
    // Slow-query nudge: a SELECT with 3+ JOINs and no LIMIT clause
    // is likely to scan a lot of rows. Surface inline LIMIT 100 and
    // EXPLAIN ANALYZE shortcuts so the user can quickly bound the
    // scope or get a cost estimate.
    //
    // Row-count lens: catalog-known table in the stmt -> "~N rows on
    // <table>". The lens describes *what the statement returns to the
    // caller*, so:
    //
    //   - SELECT          -> always emit (the FROM-side table is what
    //                        you read).
    //   - INSERT/UPDATE/DELETE -> only emit when the statement has a
    //                        RETURNING clause, because without it those
    //                        statements return no rows to the client.
    //
    // Also skip zero or negative `reltuples` (Postgres sets -1 when the
    // table has never been analysed) -- "~0 rows" is just noise.
    let upper_text = text.to_ascii_uppercase();
    let has_returning = upper_text
      .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
      .any(|w| w == "RETURNING");
    let is_select = matches!(&stmt.kind, StatementKind::Select(_));
    if is_select || has_returning {
      for (table_name, est) in find_row_estimates(&text, &live_catalog) {
        if est <= 0.0 {
          continue;
        }
        let label = format!("~{} rows on {}", fmt_count(est), table_name);
        out.push(CodeLens {
          range,
          command: Some(Command { title: label, command: "duck-sqllsp.noop".into(), arguments: None }),
          data: None,
        });
      }
    }
    if emit_runnable
      && let StatementKind::Select(_) = &stmt.kind
      && is_slow_select(&text)
    {
      out.push(CodeLens {
        range,
        command: Some(Command {
          title: "+ LIMIT 100".into(),
          command: "duck-sqllsp.addLimit".into(),
          arguments: Some(vec![serde_json::json!(text), serde_json::json!(100)]),
        }),
        data: None,
      });
      out.push(CodeLens {
        range,
        command: Some(Command {
          title: "EXPLAIN ANALYZE".into(),
          command: "duck-sqllsp.explainAnalyzeQuery".into(),
          arguments: Some(vec![serde_json::json!(text)]),
        }),
        data: None,
      });
    }
  }
  if out.is_empty() { None } else { Some(out) }
}

/// SELECT with >= 3 JOIN tokens and no LIMIT (case-insensitive, word-
/// bounded). Crude but covers the cases worth nudging on.
fn is_slow_select(text: &str) -> bool {
  let upper = text.to_ascii_uppercase();
  let join_count = upper.split_whitespace().filter(|w| *w == "JOIN").count();
  if join_count < 3 {
    return false;
  }
  // Reject if any LIMIT keyword survives, as a whole word.
  !upper.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|w| w == "LIMIT")
}

fn slice_of(text: &str, r: TextRange) -> String {
  let s: u32 = r.start().into();
  let e: u32 = r.end().into();
  let end = (e as usize).min(text.len());
  text[s as usize..end].to_string()
}

fn to_lsp_range(rope: &Rope, r: TextRange) -> Range {
  let s: u32 = r.start().into();
  let e: u32 = r.end().into();
  Range { start: byte_to_position(rope, s as usize), end: byte_to_position(rope, (e as usize).min(rope.len_bytes())) }
}

/// Pull every catalog-known table named after a `FROM` / `JOIN` / `UPDATE`
/// / `DELETE FROM` / `INTO` keyword inside `text`, and return its
/// row_estimate when present. Deduplicates by table name.
fn find_row_estimates(text: &str, catalog: &dsl_catalog::Catalog) -> Vec<(String, f64)> {
  let upper = text.to_ascii_uppercase();
  let bytes = text.as_bytes();
  let mut out: Vec<(String, f64)> = Vec::new();
  // Match `FROM` / `JOIN` / `UPDATE` / `INTO` followed by *any whitespace*
  // (incl. newlines) -- the older "FROM " prefix missed multi-line
  // formatted SELECTs like:
  //
  //   SELECT
  //     *
  //   FROM
  //     sensor_data;
  //
  // `INTO` covers `INSERT INTO t` so INSERT statements get the same
  // top-of-statement row-count lens as SELECT/UPDATE/DELETE. The outer
  // caller in `code_lens::run` gates emission on RETURNING for
  // INSERT/UPDATE/DELETE so the chip never lies about rows that don't
  // actually flow back to the client.
  for kw in ["FROM", "JOIN", "UPDATE", "INTO"] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(kw) {
      let at = from + rel;
      let after_kw = at + kw.len();
      let prev_ok = at == 0 || !is_word(bytes[at - 1] as char);
      let next_ok = after_kw < bytes.len() && (bytes[after_kw] as char).is_whitespace();
      if !prev_ok || !next_ok {
        from = after_kw;
        continue;
      }
      let mut k = after_kw;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1
      }
      // Skip ONLY keyword (PG syntax for inheritance-aware DML).
      if upper[k..].starts_with("ONLY") && k + 4 < bytes.len() && (bytes[k + 4] as char).is_whitespace() {
        k += 4;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
          k += 1
        }
      }
      let id_start = k;
      while k < bytes.len()
        && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"')
      {
        k += 1;
      }
      if k == id_start {
        from = after_kw;
        continue;
      }
      let raw = &text[id_start..k];
      let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"').to_string();
      if bare.is_empty() {
        from = k;
        continue;
      }
      if let Some(t) = catalog.find_table(None, &bare)
        && let Some(est) = t.row_estimate
        && !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(&bare))
      {
        out.push((bare.clone(), est));
      }
      from = k;
    }
  }
  out
}

fn fmt_count(n: f64) -> String {
  if n < 1_000.0 {
    return format!("{:.0}", n);
  }
  if n < 1_000_000.0 {
    return format!("{:.1}k", n / 1_000.0);
  }
  if n < 1_000_000_000.0 {
    return format!("{:.1}M", n / 1_000_000.0);
  }
  format!("{:.1}B", n / 1_000_000_000.0)
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn byte_to_position(rope: &Rope, byte: usize) -> Position {
  let byte = byte.min(rope.len_bytes());
  let line = rope.byte_to_line(byte);
  let line_start_byte = rope.line_to_byte(line);
  let line_slice = rope.line(line);
  let mut utf16 = 0u32;
  let mut bytes_seen = 0usize;
  let bytes_in_line = byte.saturating_sub(line_start_byte);
  for c in line_slice.chars() {
    if bytes_seen >= bytes_in_line {
      break;
    }
    utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line as u32, character: utf16 }
}
