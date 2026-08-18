//! `textDocument/documentLink` handler.
//!
//! Three things in a SQL file are genuinely navigable, and none of them
//! were clickable before:
//!
//!   * **psql include directives** -- `\i`, `\ir`, `\include`,
//!     `\include_relative`. Migration bundles are routinely a driver
//!     file that includes a dozen others; without links the only way to
//!     follow one is to read the path and open it by hand.
//!   * **`COPY ... FROM/TO '<path>'`** -- the CSV/dump file a load step
//!     reads or writes.
//!   * **URLs**, in comments or string literals. `-- see https://...`
//!     is the most common form of documentation in a schema file.
//!
//! # Only linking what actually opens
//!
//! File links are emitted **only when the resolved path exists on
//! disk**. This is the difference between a useful feature and an
//! irritating one: `COPY` paths are interpreted by the *server*, so
//! `/var/lib/postgres/dump.csv` frequently does not exist on the
//! developer's machine, and a link that reliably errors on click is
//! worse than no link. URLs are always linked -- we cannot check them,
//! and the cost of a dead URL is one browser tab.
//!
//! # Path resolution
//!
//! `\ir` / `\include_relative` resolve against the *including file's*
//! directory, which is what psql does. Everything else tries the
//! document's directory first, then the workspace root -- psql resolves
//! plain `\i` against its own working directory, which we cannot know,
//! so both plausible bases get a chance before we give up.

use crate::handlers::position::byte_to_lsp;
use crate::state::ServerState;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{DocumentLink, DocumentLinkParams, Range, Url};

pub fn run(state: &ServerState, params: DocumentLinkParams) -> Option<Vec<DocumentLink>> {
  let uri = &params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("document_link", uri);
  let doc = state.documents.get(uri)?;
  if doc.too_large() {
    return None;
  }

  let doc_dir = uri.to_file_path().ok().and_then(|p| p.parent().map(Path::to_path_buf));
  let ws_root = state.workspace_root.read().clone();

  let mut links: Vec<DocumentLink> = Vec::new();
  for hit in scan(&doc.text) {
    let range = Range { start: byte_to_lsp(&doc.rope, hit.start), end: byte_to_lsp(&doc.rope, hit.end) };
    let (target, tooltip) = match hit.kind {
      HitKind::Url => match Url::parse(&hit.text) {
        Ok(u) => (u, None),
        Err(_) => continue,
      },
      HitKind::Include { relative } => {
        let base = if relative { doc_dir.clone() } else { None };
        match resolve_existing(&hit.text, base.as_deref(), doc_dir.as_deref(), ws_root.as_deref()) {
          Some(p) => match Url::from_file_path(&p) {
            Ok(u) => (u, Some(format!("Included file: {}", p.display()))),
            Err(_) => continue,
          },
          None => continue,
        }
      },
      HitKind::CopyPath => match resolve_existing(&hit.text, None, doc_dir.as_deref(), ws_root.as_deref()) {
        Some(p) => match Url::from_file_path(&p) {
          Ok(u) => (u, Some(format!("COPY data file: {}", p.display()))),
          Err(_) => continue,
        },
        None => continue,
      },
    };
    links.push(DocumentLink { range, target: Some(target), tooltip, data: None });
  }

  if links.is_empty() { None } else { Some(links) }
}

/// Resolve `raw` to an existing file. `forced_base`, when set, is the
/// only base tried (`\ir` semantics); otherwise the document directory
/// and then the workspace root get a turn.
fn resolve_existing(
  raw: &str,
  forced_base: Option<&Path>,
  doc_dir: Option<&Path>,
  ws_root: Option<&Path>,
) -> Option<PathBuf> {
  let p = Path::new(raw);
  if p.is_absolute() {
    return p.is_file().then(|| p.to_path_buf());
  }
  let bases: Vec<&Path> = match forced_base {
    Some(b) => vec![b],
    None => doc_dir.into_iter().chain(ws_root).collect(),
  };
  bases.into_iter().map(|b| b.join(p)).find(|c| c.is_file())
}

#[derive(Debug, PartialEq, Eq)]
enum HitKind {
  Url,
  Include { relative: bool },
  CopyPath,
}

#[derive(Debug)]
struct Hit {
  start: usize,
  end: usize,
  text: String,
  kind: HitKind,
}

/// Find every linkable span. Byte offsets are into `src`.
fn scan(src: &str) -> Vec<Hit> {
  let mut hits = Vec::new();
  scan_urls(src, &mut hits);
  scan_includes(src, &mut hits);
  scan_copy_paths(src, &mut hits);
  hits.sort_by_key(|h| h.start);
  hits
}

fn scan_urls(src: &str, out: &mut Vec<Hit>) {
  let bytes = src.as_bytes();
  let mut i = 0usize;
  while i < src.len() {
    let rest = &src[i..];
    let Some(rel) = rest.find("http") else { break };
    let at = i + rel;
    let tail = &src[at..];
    if !(tail.starts_with("http://") || tail.starts_with("https://")) {
      i = at + 4;
      continue;
    }
    let mut end = at;
    while end < bytes.len() {
      let c = bytes[end];
      if c.is_ascii_whitespace()
        || matches!(c, b'\'' | b'"' | b'`' | b'<' | b'>' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'\\')
      {
        break;
      }
      end += 1;
    }
    // Trailing sentence punctuation is prose, not part of the URL.
    while end > at && matches!(bytes[end - 1], b'.' | b',' | b';' | b':' | b'!' | b'?') {
      end -= 1;
    }
    // `https://` alone is not a link.
    let text = &src[at..end];
    if text.len() > "https://".len() {
      out.push(Hit { start: at, end, text: text.to_string(), kind: HitKind::Url });
    }
    i = end.max(at + 1);
  }
}

/// psql meta-commands are only recognised at the start of a line.
fn scan_includes(src: &str, out: &mut Vec<Hit>) {
  // Longest first: `\include_relative` must not match as `\include`.
  const DIRECTIVES: &[(&str, bool)] =
    &[("\\include_relative", true), ("\\include", false), ("\\ir", true), ("\\i", false)];
  let mut line_start = 0usize;
  for line in src.split_inclusive('\n') {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    for (directive, relative) in DIRECTIVES {
      let Some(rest) = trimmed.strip_prefix(directive) else { continue };
      // Must be followed by whitespace, else `\include` matches
      // `\includefoo`.
      if !rest.starts_with([' ', '\t']) {
        continue;
      }
      let arg = rest.trim_start();
      let arg_offset = line_start + indent + directive.len() + (rest.len() - arg.len());
      let raw = arg.trim_end();
      // Strip an optional quote pair around the path.
      let (raw, quote_shift) = match raw.strip_prefix(['\'', '"']) {
        Some(inner) => (inner.trim_end_matches(['\'', '"']), 1),
        None => (raw, 0),
      };
      if raw.is_empty() {
        break;
      }
      let start = arg_offset + quote_shift;
      out.push(Hit {
        start,
        end: start + raw.len(),
        text: raw.to_string(),
        kind: HitKind::Include { relative: *relative },
      });
      break;
    }
    line_start += line.len();
  }
}

/// `COPY ... FROM '<path>'` / `COPY ... TO '<path>'`. `PROGRAM` and
/// `STDIN`/`STDOUT` targets are not files and are skipped.
fn scan_copy_paths(src: &str, out: &mut Vec<Hit>) {
  let upper = src.to_ascii_uppercase();
  let bytes = src.as_bytes();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("COPY") {
    let at = from + rel;
    let prev_ok = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
    if !prev_ok {
      from = at + 4;
      continue;
    }
    // Scan forward within this statement for FROM/TO followed by a
    // single-quoted literal.
    let stmt_end = src[at..].find(';').map(|e| at + e).unwrap_or(src.len());
    let mut j = at + 4;
    while j < stmt_end {
      let kw = if upper[j..stmt_end].starts_with("FROM") {
        4
      } else if upper[j..stmt_end].starts_with("TO") {
        2
      } else {
        j += 1;
        continue;
      };
      // Word boundaries on both sides.
      let before_ok = !(bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_');
      let after = j + kw;
      if !before_ok || after >= stmt_end || bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_' {
        j += 1;
        continue;
      }
      let mut k = after;
      while k < stmt_end && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // `FROM PROGRAM '...'` runs a command; `FROM STDIN` has no path.
      if k < stmt_end && bytes[k] == b'\'' {
        let path_start = k + 1;
        if let Some(close_rel) = src[path_start..stmt_end].find('\'') {
          let path_end = path_start + close_rel;
          let raw = &src[path_start..path_end];
          if !raw.is_empty() {
            out.push(Hit { start: path_start, end: path_end, text: raw.to_string(), kind: HitKind::CopyPath });
          }
          j = path_end + 1;
          continue;
        }
      }
      j = after;
    }
    from = stmt_end.max(at + 4);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn kinds(src: &str) -> Vec<(&'static str, String)> {
    scan(src)
      .into_iter()
      .map(|h| {
        let k = match h.kind {
          HitKind::Url => "url",
          HitKind::Include { relative: true } => "include_rel",
          HitKind::Include { relative: false } => "include",
          HitKind::CopyPath => "copy",
        };
        (k, h.text)
      })
      .collect()
  }

  #[test]
  fn finds_url_in_a_line_comment() {
    let got = kinds("-- see https://example.com/docs for details\nSELECT 1;");
    assert_eq!(got, vec![("url", "https://example.com/docs".to_string())]);
  }

  #[test]
  fn trailing_sentence_punctuation_is_not_part_of_the_url() {
    let got = kinds("-- ref https://example.com/a.html.");
    assert_eq!(got, vec![("url", "https://example.com/a.html".to_string())]);
  }

  #[test]
  fn url_inside_a_string_literal_is_still_found_and_stops_at_the_quote() {
    let got = kinds("INSERT INTO t VALUES ('https://example.com/x');");
    assert_eq!(got, vec![("url", "https://example.com/x".to_string())]);
  }

  #[test]
  fn bare_scheme_is_not_a_link() {
    assert!(kinds("-- https://").is_empty());
  }

  #[test]
  fn recognises_every_include_directive_with_the_right_base() {
    let got = kinds("\\i a.sql\n\\ir b.sql\n\\include c.sql\n\\include_relative d.sql\n");
    assert_eq!(
      got,
      vec![
        ("include", "a.sql".to_string()),
        ("include_rel", "b.sql".to_string()),
        ("include", "c.sql".to_string()),
        ("include_rel", "d.sql".to_string()),
      ]
    );
  }

  #[test]
  fn include_longest_directive_wins() {
    // `\include_relative` must not be parsed as `\include` with the
    // argument `_relative`.
    let got = kinds("\\include_relative sub/x.sql\n");
    assert_eq!(got, vec![("include_rel", "sub/x.sql".to_string())]);
  }

  #[test]
  fn include_requires_whitespace_after_the_directive() {
    assert!(kinds("\\include_stuff x.sql\n").is_empty());
  }

  #[test]
  fn include_strips_surrounding_quotes() {
    let got = kinds("\\i 'some file.sql'\n");
    assert_eq!(got, vec![("include", "some file.sql".to_string())]);
  }

  #[test]
  fn include_only_matches_at_the_start_of_a_line() {
    assert!(kinds("SELECT 1; \\i a.sql\n").is_empty());
  }

  #[test]
  fn finds_copy_from_and_to_paths() {
    let got = kinds("COPY t FROM '/tmp/in.csv';\nCOPY t TO '/tmp/out.csv';");
    assert_eq!(got, vec![("copy", "/tmp/in.csv".to_string()), ("copy", "/tmp/out.csv".to_string())]);
  }

  #[test]
  fn copy_from_program_and_stdin_are_not_paths() {
    assert!(kinds("COPY t FROM PROGRAM 'cat /etc/passwd';").is_empty());
    assert!(kinds("COPY t FROM STDIN;").is_empty());
  }

  #[test]
  fn copy_word_boundary_is_respected() {
    assert!(kinds("SELECT copy_thing FROM t;").is_empty());
  }

  #[test]
  fn offsets_point_at_the_path_not_the_quotes() {
    let src = "COPY t FROM '/tmp/in.csv';";
    let hit = &scan(src)[0];
    assert_eq!(&src[hit.start..hit.end], "/tmp/in.csv");
  }

  #[test]
  fn hits_come_back_in_document_order() {
    let src = "-- https://a.example\n\\i b.sql\nCOPY t FROM '/tmp/c.csv';";
    let starts: Vec<usize> = scan(src).iter().map(|h| h.start).collect();
    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted);
  }

  #[test]
  fn nonexistent_relative_path_resolves_to_nothing() {
    assert!(resolve_existing("definitely/not/here.sql", None, None, None).is_none());
  }

  #[test]
  fn absolute_path_resolves_only_when_it_exists() {
    assert!(resolve_existing("/definitely/not/here.sql", None, None, None).is_none());
    // Something that reliably exists on a Linux CI box.
    assert!(resolve_existing("/etc/hostname", None, None, None).is_some() || !Path::new("/etc/hostname").is_file());
  }
}
