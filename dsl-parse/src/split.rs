//! Split a SQL document on top-level semicolons.
//!
//! Why we slice ourselves rather than letting the parser do it: `sqlparser`
//! aborts on the first syntax error, which would poison the rest of the
//! file. By slicing first and feeding each piece in independently we
//! contain failures to a single statement and keep editing other parts of
//! the file fully analysed.
//!
//! Edge cases handled:
//!   - Single-quoted strings: semicolons inside are part of the literal.
//!   - Double-quoted identifiers: same.
//!   - Postgres dollar-quoted blocks: `$tag$ ... $tag$` (any tag, including
//!     the empty tag `$$ ... $$`).
//!   - Backslash escapes inside quoted strings.
//!
//! Block comments and line comments are not recognised; they are passed
//! through to the parser unchanged. The parser handles them.

use text_size::{TextRange, TextSize};

/// Returns `(trimmed_chunk, range_in_source)` for each top-level statement.
/// Whitespace-only chunks are dropped.
pub fn split_statements(src: &str) -> Vec<(String, TextRange)> {
    let mut out: Vec<(String, TextRange)> = Vec::new();
    let bytes = src.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut dollar_tag: Option<String> = None;

    while i < bytes.len() {
        let c = bytes[i] as char;

        if let Some(tag) = &dollar_tag {
            let closer = format!("${tag}$");
            if src[i..].starts_with(&closer) {
                i += closer.len();
                dollar_tag = None;
                continue;
            }
            i += 1;
            continue;
        }

        if !in_single && !in_double && c == '$' {
            // Try to read an opening dollar tag: $ident$ or $$.
            let rest = &src[i + 1..];
            if let Some(end) = rest.find('$') {
                let tag = &rest[..end];
                if tag.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                    dollar_tag = Some(tag.to_string());
                    i += 1 + end + 1; // skip past $tag$
                    continue;
                }
            }
        }

        if !in_double && c == '\'' && (i == 0 || bytes[i - 1] != b'\\') {
            in_single = !in_single;
        } else if !in_single && c == '"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_double = !in_double;
        } else if !in_single && !in_double && c == ';' {
            push_chunk(src, start, i, &mut out);
            start = i + 1;
        }
        i += 1;
    }

    push_chunk(src, start, src.len(), &mut out);
    out
}

fn push_chunk(src: &str, start: usize, end: usize, out: &mut Vec<(String, TextRange)>) {
    let chunk = src[start..end].trim().to_string();
    if chunk.is_empty() {
        return;
    }
    out.push((
        chunk,
        TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32)),
    ));
}
