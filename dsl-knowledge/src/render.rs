//! Markdown renderer for knowledge-base entries.
//!
//! Produces the body shown in cmp documentation popovers and LSP hover.
//! Structure:
//!
//!   header  : `# {label}` plus an italic kind tag (Keyword / Function / Type)
//!   block   : signature (functions only) in a fenced sql block
//!   body    : one-line description
//!   example : fenced sql block with a worked example
//!   footer  : link to the canonical Postgres docs

use crate::entry::{Entry, Kind};

pub fn render_markdown(entry: &Entry) -> String {
  // Use blank lines (`\n\n`) between sections so CommonMark / nvim
  // markdown floats render each as its own paragraph instead of one
  // collapsed block. Order: heading, kind tag, signature, body
  // paragraph(s), example, docs link.
  let mut sections: Vec<String> = Vec::new();

  // Heading: `# label` followed by an italic kind tag on its own line.
  sections.push(format!("# {}\n\n_{}_", entry.label, kind_label(entry.kind)));

  if let Some(sig) = entry.signature {
    sections.push(format!("```sql\n{}\n```", sig.trim_end()));
  }

  if !entry.doc.is_empty() {
    // Preserve any internal blank lines the entry author wrote --
    // they signal paragraph breaks in long descriptions. Wrap each
    // paragraph at ~72 chars so the hover float stays narrow.
    sections.push(wrap_paragraphs(entry.doc.trim_end(), 72));
  }

  if !entry.example.is_empty() {
    sections.push(format!("**Example**\n\n```sql\n{}\n```", entry.example.trim_end()));
  }

  if !entry.url.is_empty() {
    sections.push(format!("[Postgres docs]({})", entry.url));
  }

  // Join with a blank line so markdown sees real paragraph breaks.
  let mut out = sections.join("\n\n");
  out.push('\n');
  out
}

fn kind_label(kind: Kind) -> &'static str {
  match kind {
    Kind::Keyword => "Keyword",
    Kind::Function => "Function",
    Kind::Type => "Type",
  }
}

/// Greedy word-wrap each paragraph at `width` columns. Preserves blank
/// lines (paragraph separators), leaves fenced code blocks
/// (triple-backtick) and indented lines untouched.
pub fn wrap_paragraphs(text: &str, width: usize) -> String {
  let mut out = String::with_capacity(text.len() + 16);
  let mut in_fence = false;
  let mut first_line = true;
  for line in text.split('\n') {
    if !first_line {
      out.push('\n');
    }
    first_line = false;
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
      in_fence = !in_fence;
      out.push_str(line);
      continue;
    }
    // Don't wrap: inside code fence, indented lines, markdown list
    // items (let the renderer handle nested wrapping), blank lines.
    if in_fence
      || line.starts_with("    ")
      || line.starts_with('\t')
      || line.starts_with('|')
      || line.starts_with('-')
      || line.starts_with('*')
      || line.starts_with('>')
      || line.trim().is_empty()
    {
      out.push_str(line);
      continue;
    }
    out.push_str(&wrap_one(line, width));
  }
  out
}

fn wrap_one(line: &str, width: usize) -> String {
  let mut out = String::with_capacity(line.len() + 8);
  let mut col = 0usize;
  let mut first = true;
  for word in line.split_whitespace() {
    let w = word.chars().count();
    if first {
      out.push_str(word);
      col = w;
      first = false;
      continue;
    }
    if col + 1 + w > width {
      out.push('\n');
      out.push_str(word);
      col = w;
    } else {
      out.push(' ');
      out.push_str(word);
      col += 1 + w;
    }
  }
  out
}
