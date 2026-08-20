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
     `duck-sqllsp rules` prints the same list from the command line.\n\
     `duck-sqllsp rules --search partition` narrows it, and `--json`\n\
     emits it machine-readably.\n\n",
  );
  s.push_str(&format!("{} rules.\n\n", rules.len()));

  // Index first. The detail below runs to thousands of lines, and
  // without a scannable list the only way to find a rule is to already
  // know its code.
  s.push_str("## Index\n\n| Code | Summary |\n| --- | --- |\n");
  for code in numeric_order(rules) {
    let (_, title, _) = &rules[code];
    // Pipes would break the table; no summary currently contains one,
    // but escaping is cheaper than finding out later.
    s.push_str(&format!("| [`{code}`](#{}) | {} |\n", anchor(code, title), title.replace('|', "\\|")));
  }
  s.push_str("\n## Rules\n\n");

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

/// GitHub's heading anchor: lowercase, punctuation dropped, spaces to
/// hyphens. Matches the `### \`code\` — title` headings below.
fn anchor(code: &str, title: &str) -> String {
  let heading = format!("{code} — {title}");
  let mut out = String::new();
  for ch in heading.chars() {
    if ch.is_alphanumeric() {
      out.extend(ch.to_lowercase());
    } else if ch == ' ' || ch == '-' || ch == '_' {
      out.push('-');
    }
  }
  out
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

/// Every index link must point at a heading that exists.
///
/// The index is generated, but the anchor format is GitHub's, not
/// ours -- so a change to the heading layout can silently orphan all
/// 701 links while the file still looks fine.
#[test]
fn every_index_link_resolves_to_a_heading() {
  let doc = std::fs::read_to_string(docs_path()).expect("docs/rules.md");
  let headings: std::collections::BTreeSet<String> =
    doc.lines().filter_map(|l| l.strip_prefix("### ")).map(github_anchor).collect();

  let mut links = 0usize;
  let mut broken = Vec::new();
  for line in doc.lines() {
    let Some(rest) = line.split_once("](#") else { continue };
    let Some((target, _)) = rest.1.split_once(')') else { continue };
    links += 1;
    if !headings.contains(target) {
      broken.push(target.to_string());
    }
  }
  assert_eq!(links, headings.len(), "index should link every rule exactly once");
  assert!(broken.is_empty(), "index links with no matching heading: {broken:?}");
}

/// GitHub's heading-anchor rules, mirroring `anchor` in the generator.
fn github_anchor(heading: &str) -> String {
  let mut out = String::new();
  for ch in heading.chars() {
    if ch.is_alphanumeric() {
      out.extend(ch.to_lowercase());
    } else if ch == ' ' || ch == '-' || ch == '_' {
      out.push('-');
    }
  }
  out
}

/// Every documented rule count must match the registry.
///
/// The VS Code marketplace listing advertised "150+ analysis rules" for
/// long enough that the real number passed 700 -- a claim off by nearly
/// five times, in the first thing a prospective user reads. Nothing
/// checked it, so nothing caught it.
///
/// Deliberately excludes CHANGELOG.md: historical entries cite the count
/// at the time and should not be rewritten.
#[test]
fn documented_rule_counts_match_the_registry() {
  let actual = dsl_analysis::rules::all().len();
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");

  let docs = ["README.md", "vscode-extension/README.md", "dsl-analysis/docs/rules.md", "CONTRIBUTING.md"];

  let mut checked = 0usize;
  for rel in docs {
    let path = repo.join(rel);
    let Ok(text) = std::fs::read_to_string(&path) else { continue };
    for (i, line) in text.lines().enumerate() {
      for claim in rule_counts_in(line) {
        assert_eq!(claim, actual, "{rel}:{} claims {claim} rules, registry has {actual}:\n  {}", i + 1, line.trim());
        checked += 1;
      }
    }
  }
  assert!(checked > 0, "expected at least one documented rule count to verify");
}

/// Numbers written as "<n> rules" / "<n> lint rules" / "<n> analysis
/// rules", with an optional trailing `+`. Three digits or more, so
/// ordinary prose numbers are not mistaken for a count.
fn rule_counts_in(line: &str) -> Vec<usize> {
  let words: Vec<&str> = line.split_whitespace().collect();
  let mut out = Vec::new();
  for (i, w) in words.iter().enumerate() {
    // Strip markdown emphasis and punctuation so `**701` and `(701`
    // are still seen. Missing this made an earlier version of this test
    // pass against the very "**150+ lint rules**" it was written for.
    let w = w.trim_start_matches(['*', '_', '(', '[', '`', '~']);
    let digits: String = w.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 3 {
      continue;
    }
    // Allow "701", "701+", and "**701" from bold markdown.
    let tail = &w[digits.len()..];
    if !tail.is_empty() && tail != "+" {
      continue;
    }
    let follows = words.get(i + 1).copied().unwrap_or("");
    let follows2 = words.get(i + 2).copied().unwrap_or("");
    let is_count = follows.starts_with("rule")
      || ((follows == "lint" || follows == "analysis" || follows == "diagnostic") && follows2.starts_with("rule"))
      || follows.starts_with("diagnostics");
    if is_count {
      out.push(digits.parse().unwrap());
    }
  }
  out
}
