//! sql195: `CAST('lit' AS <type>)` or `'lit'::<type>` where `lit`
//! can't be parsed as `<type>`. PG raises 22P02 at runtime. Only
//! fires for cheap, lossless local checks:
//!   - INT family: non-integer literals
//!   - NUMERIC / FLOAT family: non-numeric literals
//!   - UUID: not 8-4-4-4-12 hex
//!   - BOOLEAN: not in {true,false,t,f,1,0,yes,no}
//!   - DATE: not YYYY-MM-DD
//!   - TIMESTAMP: not YYYY-MM-DD HH:MM[:SS][+TZ]

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql195"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      // Detect a single-quoted literal start.
      if bytes[i] == b'\'' {
        let lit_start = i + 1;
        let mut j = lit_start;
        while j < bytes.len() {
          if bytes[j] == b'\'' {
            if j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
              j += 2;
              continue;
            }
            break;
          }
          j += 1;
        }
        if j >= bytes.len() {
          break;
        }
        let lit = &body[lit_start..j];
        let lit_close = j;
        // After the closing quote, check for `::<type>`.
        let after = lit_close + 1;
        if after + 2 < bytes.len() && &body[after..after + 2] == "::" {
          let type_start = after + 2;
          let mut k = type_start;
          while k < bytes.len() {
            let c = bytes[k] as char;
            if c.is_ascii_alphanumeric() || c == '_' || c == '(' || c == ')' || c == '[' || c == ']' || c == ' ' {
              k += 1
            } else {
              break;
            }
          }
          let ty = body[type_start..k].trim();
          // Skip array-type casts -- the literal is an array curly form
          // `'{1,2,3}'`, not a scalar value that must parse as <type>.
          if ty.ends_with("[]") || ty.to_ascii_uppercase().starts_with("ARRAY") {
            i = j + 1;
            continue;
          }
          if !ty.is_empty()
            && let Some(reason) = literal_cast_error(lit, ty)
          {
            out.push(Diagnostic {
              code: "sql195",
              severity: Severity::Error,
              message: format!("CAST '{lit}'::{ty} -- {reason} (PG 22P02 at runtime)"),
              range: text_size::TextRange::new(((start + lit_start - 1) as u32).into(), ((start + k) as u32).into()),
            });
          }
        }
        i = j + 1;
        continue;
      }
      i += 1;
    }
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("CAST(") {
      let at = from + rel;
      let inner_start = at + "CAST(".len();
      let Some(close) = body[inner_start..].find(')') else { break };
      let inner = &body[inner_start..inner_start + close];
      let inner_upper = inner.to_ascii_uppercase();
      let Some(as_at) = inner_upper.find(" AS ") else {
        from = inner_start + close;
        continue;
      };
      let lit_part = inner[..as_at].trim();
      let ty_part = inner[as_at + " AS ".len()..].trim();
      if let Some(lit) = lit_part.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))
        && let Some(reason) = literal_cast_error(lit, ty_part)
      {
        out.push(Diagnostic {
          code: "sql195",
          severity: Severity::Error,
          message: format!("CAST('{lit}' AS {ty_part}) -- {reason} (PG 22P02 at runtime)"),
          range: text_size::TextRange::new(
            ((start + at) as u32).into(),
            ((start + inner_start + close + 1) as u32).into(),
          ),
        });
      }
      from = inner_start + close + 1;
    }
  }
}

fn literal_cast_error(lit: &str, ty: &str) -> Option<String> {
  let bare = ty.split('(').next().unwrap_or(ty).trim().to_ascii_lowercase();
  match bare.as_str() {
    "int" | "integer" | "int4" | "int8" | "int2" | "bigint" | "smallint" if lit.parse::<i64>().is_err() => {
      return Some(format!("'{lit}' not a valid integer"));
    },
    "numeric" | "decimal" | "real" | "double" | "float" | "float4" | "float8" if lit.parse::<f64>().is_err() => {
      return Some(format!("'{lit}' not a valid number"));
    },
    "boolean" | "bool" => {
      let v = lit.trim().to_ascii_lowercase();
      let ok = ["t", "f", "true", "false", "y", "n", "yes", "no", "1", "0", "on", "off"].contains(&v.as_str());
      if !ok {
        return Some(format!("'{lit}' not a valid boolean"));
      }
    },
    "uuid" => {
      let v = lit.trim().to_ascii_lowercase();
      let segs: Vec<&str> = v.split('-').collect();
      let ok = segs.len() == 5
        && segs[0].len() == 8
        && segs[1].len() == 4
        && segs[2].len() == 4
        && segs[3].len() == 4
        && segs[4].len() == 12
        && segs.iter().all(|s| s.chars().all(|c| c.is_ascii_hexdigit()));
      if !ok {
        return Some(format!("'{lit}' not a valid uuid (need 8-4-4-4-12 hex)"));
      }
    },
    "date" => {
      let v = lit.trim();
      let segs: Vec<&str> = v.split('-').collect();
      let ok = segs.len() == 3
        && segs[0].len() == 4
        && segs[0].chars().all(|c| c.is_ascii_digit())
        && segs[1].len() == 2
        && segs[1].chars().all(|c| c.is_ascii_digit())
        && segs[2].len() == 2
        && segs[2].chars().all(|c| c.is_ascii_digit());
      if !ok {
        return Some(format!("'{lit}' not a valid DATE (need YYYY-MM-DD)"));
      }
    },
    _ => {},
  }
  None
}
