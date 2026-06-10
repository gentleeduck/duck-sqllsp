//! Small shared helpers used across the driver introspectors.

/// Strip a single balanced pair of outer parentheses, e.g. `(a + b)` ->
/// `a + b`. Only strips when the opening paren's match is the final char,
/// so `(a) + (b)` is left untouched. Used to normalise the parenthesised
/// `generation_expression` text drivers return into the bare-expression
/// form the catalog stores everywhere else (source scanner, hover render).
pub(crate) fn strip_outer_parens(s: &str) -> &str {
  let t = s.trim();
  let bytes = t.as_bytes();
  if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
    return t;
  }
  let mut depth = 0i32;
  for (i, &b) in bytes.iter().enumerate() {
    match b {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        // The first '(' closes before the end -> not a single wrapping pair.
        if depth == 0 && i != bytes.len() - 1 {
          return t;
        }
      },
      _ => {},
    }
  }
  t[1..t.len() - 1].trim()
}

#[cfg(test)]
mod tests {
  use super::strip_outer_parens;

  #[test]
  fn strips_single_wrapping_pair() {
    assert_eq!(strip_outer_parens("(a + b)"), "a + b");
    assert_eq!(strip_outer_parens("((a + b))"), "(a + b)");
    assert_eq!(strip_outer_parens("  (price * qty)  "), "price * qty");
    assert_eq!(strip_outer_parens("(`price` * `qty`)"), "`price` * `qty`");
  }

  #[test]
  fn leaves_unwrapped_and_partial_untouched() {
    assert_eq!(strip_outer_parens("a + b"), "a + b");
    assert_eq!(strip_outer_parens("(a) + (b)"), "(a) + (b)");
    assert_eq!(strip_outer_parens("upper(name)"), "upper(name)");
    assert_eq!(strip_outer_parens(""), "");
  }
}
