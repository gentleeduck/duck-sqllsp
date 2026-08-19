//! Keeps the generated rule reference in step with the rules themselves.
//!
//! Each rule module opens with `//! sqlNNN: <summary> -- <detail>`. That
//! comment sits next to the implementation, so it is the thing that
//! actually gets updated when a rule changes -- which makes it the right
//! source of truth, and makes anything derived from it liable to go
//! stale silently.
//!
//! Two artifacts are derived from it:
//!
//!   * `src/rules/titles.rs`  -- the runtime lookup behind
//!     `duck-sqllsp rules` and `--json`.
//!   * `docs/rules.md`        -- the human reference.
//!
//! These tests fail if either drifts. To regenerate both:
//!
//!     cargo test -p dsl-analysis --test rule_reference -- --ignored
//!
//! Doing this in a test rather than a build script keeps the generated
//! files reviewable in the diff, which matters when the content is
//! user-facing prose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn rules_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

/// `code -> (module, title, description)` harvested from the doc comments,
/// restricted to rules that are actually registered.
///
/// A rule file can exist without being registered -- `sql129`
/// (`alter_table_no_owner`) is commented out of the registry as too
/// noisy. Documenting a rule that can never fire is worse than omitting
/// it, so the generated reference follows the registry, not the
/// directory listing.
fn extract() -> BTreeMap<String, (String, String, String)> {
  let registered: std::collections::BTreeSet<String> =
    dsl_analysis::rules::all().into_iter().map(|r| r.code().to_string()).collect();
  let mut out = BTreeMap::new();
  let entries = std::fs::read_dir(rules_dir()).expect("rules dir");
  for entry in entries.flatten() {
    let path = entry.path();
    if path.extension().and_then(|s| s.to_str()) != Some("rs") {
      continue;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    if stem == "mod" || stem == "titles" {
      continue;
    }
    let text = std::fs::read_to_string(&path).expect("read rule");
    let mut doc = String::new();
    for line in text.lines() {
      if let Some(rest) = line.strip_prefix("//!") {
        if !doc.is_empty() {
          doc.push(' ');
        }
        doc.push_str(rest.trim());
      } else if !doc.is_empty() {
        break;
      }
    }
    let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    // rustdoc intra-doc links (`[`x`](super::y)`) are meaningless in
    // plain markdown and render as broken links. Keep the label, drop
    // the path.
    let doc = strip_intra_doc_links(&doc);
    let Some((code, body)) = doc.split_once(':') else { continue };
    let code = code.trim();
    if !code.starts_with("sql") || !code[3..].chars().all(|c| c.is_ascii_digit()) {
      continue;
    }
    if !registered.contains(code) {
      continue;
    }
    let body = body.trim().to_string();
    out.insert(code.to_string(), (stem, summarise(&body), body));
  }
  out
}

/// Rewrite `[label](super::path)` / `[label](crate::path)` to just
/// `label`. A rustdoc path is not a URL, so leaving it in produces a
/// link that resolves to nothing.
fn strip_intra_doc_links(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let mut rest = s;
  while let Some(open) = rest.find('[') {
    let Some(close) = rest[open..].find("](") else { break };
    let close = open + close;
    let Some(end) = rest[close..].find(')') else { break };
    let end = close + end;
    let target = &rest[close + 2..end];
    out.push_str(&rest[..open]);
    if target.starts_with("super::") || target.starts_with("crate::") || target.starts_with("self::") {
      out.push_str(&rest[open + 1..close]);
    } else {
      out.push_str(&rest[open..=end]);
    }
    rest = &rest[end + 1..];
  }
  out.push_str(rest);
  out
}

/// The one-line summary: everything before the first ` -- `, else the
/// first sentence, capped on a word boundary.
fn summarise(body: &str) -> String {
  const LIMIT: usize = 110;
  let head = match body.split_once(" -- ") {
    Some((h, _)) => h.to_string(),
    None => {
      let mut end = body.len();
      let bytes = body.as_bytes();
      for i in 0..bytes.len() {
        if matches!(bytes[i], b'.' | b'!' | b'?') && bytes.get(i + 1) == Some(&b' ') {
          end = i + 1;
          break;
        }
      }
      body[..end].to_string()
    },
  };
  let head = head.trim().trim_end_matches('.').trim();
  if head.chars().count() <= LIMIT {
    return head.to_string();
  }
  let cut: String = head.chars().take(LIMIT).collect();
  let cut = cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut);
  format!("{}...", cut.trim_end_matches([' ', ',', ';', ':']))
}

fn render_titles(rules: &BTreeMap<String, (String, String, String)>) -> String {
  let mut s = String::from(
    "//! Rule titles, generated from each rule module's `//! sqlNNN: ...`\n\
     //! doc comment.\n//!\n//! DO NOT EDIT BY HAND. Regenerate with:\n//!\n\
     //! ```text\n//! cargo test -p dsl-analysis --test rule_reference -- --ignored\n//! ```\n//!\n\
     //! The doc comment is the single source of truth: it sits next to the\n\
     //! implementation, so it is the thing that actually gets updated when a\n\
     //! rule changes. `rule_reference` fails the build if this file or\n\
     //! `docs/rules.md` drifts away from it.\n\n\
     /// One-line summary for a diagnostic code, or `None` if unknown.\n\
     pub fn title(code: &str) -> Option<&'static str> {\n  \
     TITLES.binary_search_by_key(&code, |(c, _)| c).ok().map(|i| TITLES[i].1)\n}\n\n\
     /// Every rule code paired with its summary, sorted by code so `title`\n\
     /// can binary-search.\npub static TITLES: &[(&str, &str)] = &[\n",
  );
  for (code, (_, title, _)) in rules {
    s.push_str(&format!("  (\"{}\", \"{}\"),\n", code, title.replace('\\', "\\\\").replace('"', "\\\"")));
  }
  s.push_str("];\n");
  s
}

/// Numeric order, so sql9 sorts before sql10 in the human reference.
fn numeric_order(rules: &BTreeMap<String, (String, String, String)>) -> Vec<&String> {
  let mut codes: Vec<&String> = rules.keys().collect();
  codes.sort_by_key(|c| c[3..].parse::<u32>().unwrap_or(u32::MAX));
  codes
}

fn render_docs(rules: &BTreeMap<String, (String, String, String)>) -> String {
  let mut s = String::from(
    "# Lint rule reference\n\n\
     Every diagnostic duck-sqllsp can emit. Generated from each rule's own doc\n\
     comment — see `dsl-analysis/src/rules/`.\n\n\
     Silence or re-level any of these by code:\n\n\
     ```toml\n[duck_sqllsp.rules]\nsql015 = \"off\"      # off / ignore / none\n\
     sql001 = \"hint\"     # or error / warning / info\n```\n\n\
     `duck-sqllsp rules` prints the same list from the command line, and\n\
     `duck-sqllsp rules --json` emits it machine-readably.\n\n",
  );
  s.push_str(&format!("{} rules.\n\n", rules.len()));
  for code in numeric_order(rules) {
    let (module, title, description) = &rules[code];
    s.push_str(&format!("### `{code}` — {title}\n\n"));
    if description != title {
      s.push_str(&format!("{description}\n\n"));
    }
    s.push_str(&format!("<sub>`dsl-analysis/src/rules/{module}.rs`</sub>\n\n"));
  }
  s
}

fn titles_path() -> PathBuf {
  rules_dir().join("titles.rs")
}

fn docs_path() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/rules.md")
}

/// Compare entries rather than bytes: `titles.rs` is library code and so
/// gets rustfmt'd, which would break a byte-exact check against the
/// generator's own line wrapping.
#[test]
fn titles_table_matches_the_rule_doc_comments() {
  let want = extract();
  let have: BTreeMap<String, String> =
    dsl_analysis::rules::TITLES.iter().map(|(c, t)| ((*c).to_string(), (*t).to_string())).collect();

  let stale = "src/rules/titles.rs is stale. \
    Regenerate: cargo test -p dsl-analysis --test rule_reference -- --ignored";
  assert_eq!(have.len(), want.len(), "{stale} (entry count differs)");
  for (code, (_, title, _)) in &want {
    assert_eq!(have.get(code), Some(title), "{stale} (summary for {code})");
  }
}

/// Line endings are normalised first: git checks `.md` out as CRLF on
/// Windows when `core.autocrlf` is on, while the generator always emits
/// LF, so a raw comparison fails there and only there.
#[test]
fn rules_doc_matches_the_rule_doc_comments() {
  let want = render_docs(&extract()).replace("\r\n", "\n");
  let have = std::fs::read_to_string(docs_path()).expect("docs/rules.md must exist").replace("\r\n", "\n");
  assert_eq!(
    have.trim_end(),
    want.trim_end(),
    "docs/rules.md is stale.\nRegenerate: cargo test -p dsl-analysis --test rule_reference -- --ignored"
  );
}

/// The generated table must cover exactly the rules that are actually
/// registered -- a rule with no summary prints a blank line in
/// `duck-sqllsp rules`, and a summary with no rule is a leftover.
#[test]
fn every_registered_rule_has_a_summary_and_vice_versa() {
  let registered: Vec<String> = dsl_analysis::rules::all().into_iter().map(|r| r.code().to_string()).collect();
  for code in &registered {
    let title = dsl_analysis::rules::title(code);
    assert!(title.is_some(), "rule {code} is registered but has no summary in titles.rs");
    assert!(!title.unwrap().trim().is_empty(), "rule {code} has an empty summary");
  }
  for (code, _) in dsl_analysis::rules::TITLES {
    assert!(registered.iter().any(|r| r == code), "titles.rs lists {code}, which is not a registered rule");
  }
}

#[test]
fn summaries_are_single_line_and_bounded() {
  for (code, title) in dsl_analysis::rules::TITLES {
    assert!(!title.contains('\n'), "{code}: summary must be one line");
    assert!(title.chars().count() <= 113, "{code}: summary is {} chars, too long to scan", title.chars().count());
  }
}

/// Regeneration entry point. Ignored by default so a normal `cargo test`
/// only ever *checks*.
#[test]
#[ignore = "writes generated files; run explicitly to regenerate"]
fn regenerate() {
  let rules = extract();
  std::fs::write(titles_path(), render_titles(&rules)).expect("write titles.rs");
  std::fs::create_dir_all(docs_path().parent().unwrap()).expect("docs dir");
  std::fs::write(docs_path(), render_docs(&rules)).expect("write rules.md");
  eprintln!("regenerated {} rules", rules.len());
}
