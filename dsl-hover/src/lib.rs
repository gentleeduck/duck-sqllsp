//! Rich SQL hover.
//!
//! Resolution order (first match wins):
//!   1. Column declaration when cursor sits on the column NAME inside
//!      a CREATE TABLE in the current buffer.
//!   2. Constraint / index / trigger identifier (pk_/fk_/idx_/...) -- catalog
//!      lookup or fallback convention card.
//!   3. Catalog table / column / DB function.
//!   4. Multi-word SQL keyword (sliding window).
//!   5. Single-word knowledge entry (keyword / type / built-in function).

pub mod constraint_id;
pub mod ddl;
pub mod implicit;
pub mod md_table;
pub mod render;
pub mod resolver;
pub mod token;

use dsl_catalog::Catalog;
use text_size::TextSize;

/// How keywords are cased in synthesized DDL fragments shown in hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeywordCase {
  #[default]
  Upper,
  Lower,
  Preserve,
}

impl KeywordCase {
  pub fn apply(self, s: &str) -> String {
    match self {
      KeywordCase::Upper => s.to_ascii_uppercase(),
      KeywordCase::Lower => s.to_ascii_lowercase(),
      KeywordCase::Preserve => s.to_string(),
    }
  }
}

// Thread-local current keyword case so the render fns don't need a new
// parameter on every call site. Set by `hover_with`. Default Upper.
thread_local! {
    static KW_CASE: std::cell::Cell<KeywordCase> = const { std::cell::Cell::new(KeywordCase::Upper) };
}

pub fn current_keyword_case() -> KeywordCase {
  KW_CASE.with(|c| c.get())
}

pub fn hover(source: &str, offset: TextSize, catalog: &Catalog) -> Option<String> {
  hover_with(source, offset, catalog, KeywordCase::Upper)
}

/// Like `hover` but applies the caller's preferred keyword casing to
/// every synthesised DDL fragment.
pub fn hover_with(source: &str, offset: TextSize, catalog: &Catalog, case: KeywordCase) -> Option<String> {
  KW_CASE.with(|c| c.set(case));
  // Respect lexical boundaries: cursor inside `'...'` / `"..."` /
  // `-- comment` / `/* ... */` / `$$ ... $$` body for a non-PL/pgSQL
  // language returns None for keywords. Catches `'illegal order
  // adding...'` so the ORDER keyword card doesn't fire on a literal.
  //
  // Inside a quoted string, surface a small "literal" card naming
  // the inferred destination column type when we can resolve one
  // (INSERT VALUES / UPDATE SET / WHERE col = ...).
  let pos: usize = u32::from(offset) as usize;
  // Double-quoted identifier (`"User Id"`) -- a case-preserved name,
  // never a keyword. Suppress hover entirely so the lookup pipeline
  // doesn't mis-resolve the inner text to a SQL keyword (e.g. cursor
  // inside `"User Id"` mustn't fire the USER keyword card). A future
  // iteration can try a column / table lookup using the unquoted name.
  if cursor_inside_double_quoted_ident(source, pos) {
    return None;
  }
  if inside_string_or_comment(source, pos) {
    // Specific-card paths must run BEFORE the generic string-literal
    // fallback. A cursor inside `'user_id_seq'` of `nextval(...)` wants
    // the sequence card, not "text/string literal"; same for an arg
    // inside `coalesce(name, 'fallback')` -- the function-arg card
    // beats the generic literal card.
    if let Some(md) = sequence_ref_at(source, offset) {
      return Some(md);
    }
    if let Some(md) = function_arg_at(source, offset) {
      return Some(md);
    }
    if let Some(card) = string_literal_hover(source, pos, catalog) {
      return Some(card);
    }
    return None;
  }
  // Cursor sits on a numeric literal -- small card naming the literal
  // kind + the destination column type when resolvable.
  if let Some(card) = numeric_literal_hover(source, pos, catalog) {
    return Some(card);
  }
  let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  if let Some(md) = ddl::column_decl_at(&parsed, source, offset) {
    return Some(md);
  }
  // Sequence reference inside `nextval('seq_name')` / `currval(...)`
  // / `setval(...)` -- when the cursor sits on the literal sequence
  // name, return a card describing it.
  if let Some(md) = sequence_ref_at(source, offset) {
    return Some(md);
  }
  // Cursor on an argument literal inside `fn(arg, arg)`. Surface
  // the function's signature with the current parameter highlighted
  // so the user knows "I'm filling slot 2 of substring(text, int, int)".
  // Gate: only fire when the cursor is NOT sitting on an identifier-
  // shaped byte. Identifier hover (column / alias / table) is handled
  // downstream and must win for cases like `count(u.id)` -- otherwise
  // the function signature swallows the column the user pointed at.
  if !cursor_on_word_byte(source, pos)
    && let Some(md) = function_arg_at(source, offset)
  {
    return Some(md);
  }

  // Cursor on a KNN/distance/phrase operator (<->, <#>, <%>, ...).
  // Must precede the jsonb-`->` lookup; otherwise `<-` would be matched
  // as `->` partial.
  if let Some(md) = distance_operator_hover(source, pos) {
    return Some(md);
  }
  // Geometric / math / range comparison ops (|/, ||/, @-@, ##, &<,
  // &>, &<|, |>>, <<|, &&). Runs after distance so `<->` isn't shadowed.
  if let Some(md) = geometric_math_operator_hover(source, pos) {
    return Some(md);
  }
  // Cursor on the JSON path operators (-> / ->> / #> / #>>). Surface
  // a short card naming the operator's return type.
  if let Some(md) = jsonb_operator_hover(source, pos) {
    return Some(md);
  }

  // Cursor on a comparison / string / range operator (=, <>, !=,
  // <, >, <=, >=, ||, @>, <@, ?, ?|, ?&). Surface a brief explanation.
  if let Some(md) = comparison_operator_hover(source, pos) {
    return Some(md);
  }

  // Cursor on the NULL keyword -- card explains three-valued logic
  // and the destination column nullability when resolvable.
  if let Some(md) = null_keyword_hover(source, pos, catalog) {
    return Some(md);
  }

  // Cursor on a `*` token (SELECT projection / `u.*` / `count(*)`).
  // The lexer-based token_at skips non-word chars so the star never
  // makes it to the normal lookup path. Handle it explicitly.
  if let Some(md) = star_hover(source, offset, catalog) {
    return Some(md);
  }

  // Cursor on the `::` cast operator itself (between expr and type).
  if cursor_on_double_colon(source, offset) {
    return Some(
      "# `::` cast operator\n\n_PG-specific shorthand for `CAST(expr AS type)`_\n\n\
       ```sql\n\
       '123'::int            -- string -> integer\n\
       column_name::text     -- column to text\n\
       now()::date           -- timestamp -> date\n\
       arr::int[]            -- array element type cast\n\
       data::jsonb           -- text -> jsonb\n\
       ```\n\n\
       Equivalent to `CAST(expr AS type)`. Errors at runtime if the value can't be converted; \
       check with `IS NOT NULL` first when in doubt. Inside DDL prefer the standard CAST form."
        .to_string(),
    );
  }

  if let Some(tok) = token::token_at(source, offset) {
    // Dotted token `a.b` -- narrow to the side under the cursor.
    // Cursor on the alias side => table card. Cursor on the column
    // side => single-column card resolved through the alias.
    if tok.contains('.')
      && let Some(part) = dotted_part_under_cursor(source, offset, &tok)
    {
      let last_seg = tok.rsplit('.').next().unwrap_or("");
      let on_right = part == last_seg;
      if on_right {
        if let Some(md) = scope_column_lookup(source, offset, &tok, catalog) {
          return Some(md);
        }
        if let Some(md) = catalog_lookup(&part, catalog) {
          return Some(md);
        }
        // Schema-qualified function call: `gdpr.update_user_gdpr()`.
        // Cursor on the function-name segment -- fire the function
        // card, scoped to the schema when one is provided.
        let left_seg = tok.split('.').next();
        if let Some(md) = db_function_scoped(&part, left_seg, catalog) {
          return Some(md);
        }
      } else {
        if let Some(md) = alias_lookup(source, offset, &part, catalog) {
          return Some(md);
        }
        if let Some(md) = catalog_lookup(&part, catalog) {
          return Some(md);
        }
      }
      if let Some(entry) = dsl_knowledge::lookup(&part) {
        return Some(dsl_knowledge::render_markdown(entry));
      }
    }
    // NEW / OLD trigger row aliases. Inside a CREATE TRIGGER body
    // they bind to the trigger's target table; standalone we still
    // explain what they are.
    let upper_tok = tok.to_ascii_uppercase();
    if upper_tok == "NEW" || upper_tok == "OLD" {
      return Some(new_old_hover(&upper_tok, source));
    }
    // PL/pgSQL function parameter / DECLARE'd local. Cheap text scan
    // of the enclosing CREATE FUNCTION header + DECLARE block.
    if let Some(md) = plpgsql_local_hover(source, offset, &tok) {
      return Some(md);
    }
    // Buffer-defined CREATE FUNCTION / CREATE INDEX / CREATE TRIGGER
    // first -- if the user just wrote it, that's exactly what they
    // want to see, not a generic "no match in catalog" card.
    if let Some(md) = buffer_object(source, &tok) {
      return Some(md);
    }
    // Alias resolution -- if the cursor sits on `u` in `FROM users u`
    // or `JOIN orders AS o`, hover the underlying table.
    if let Some(md) = alias_lookup(source, offset, &tok, catalog) {
      return Some(md);
    }
    // Cursor inside a CREATE TABLE body? Resolve the column against
    // just that table to avoid the "this column lives in 11 tables"
    // ambiguity card -- inside `pk_users_id PRIMARY KEY (id)` the
    // `id` means `users.id`, full stop.
    if let Some(md) = enclosing_table_column(source, offset, &tok, catalog) {
      return Some(md);
    }
    // Same scoping for CREATE INDEX ... ON <table> (col), UPDATE
    // <table> SET col, DELETE FROM <table> WHERE col, INSERT INTO
    // <table> (col, ...) -- if we can name the single target table
    // from the surrounding text, narrow the hover to that table.
    if let Some(md) = scoped_column_in_text(source, offset, &tok, catalog) {
      return Some(md);
    }
    if let Some(md) = constraint_id::render_for(&tok, catalog) {
      return Some(md);
    }
    // Scope-aware column hover: when the cursor sits on `id` or
    // `ur.id` inside a SELECT / UPDATE / DELETE, resolve via the
    // statement's FROM/JOIN bindings so the hover names the actual
    // origin table instead of a "lives in N tables" card.
    if let Some(md) = scope_column_lookup(source, offset, &tok, catalog) {
      return Some(md);
    }
    // Role hover -- only resolves when the cursor sits in a role-name
    // slot (OWNER TO, GRANT/REVOKE TO/FROM, SET ROLE, ...) so that a
    // column / table / etc named the same thing wins elsewhere.
    if let Some(md) = role_hover(source, offset, &tok, catalog) {
      return Some(md);
    }
    if let Some(md) = catalog_lookup(&tok, catalog) {
      return Some(md);
    }
    if let Some(md) = db_function(&tok, catalog) {
      return Some(md);
    }
  }

  let window = token::window_at(source, offset);
  if let Some(md) = resolver::from_window(&window) {
    return Some(md);
  }

  if let Some(tok) = token::token_at(source, offset) {
    // Field-list context priority: when the cursor sits inside a
    // column-list paren (CREATE TABLE / INSERT cols / CREATE INDEX
    // ON t (...) / RETURNING / GROUP BY / ORDER BY / SET / UNIQUE /
    // PRIMARY KEY paren), the token is almost certainly an
    // identifier of a column -- NOT a keyword. Skip the keyword
    // card fallback so `password` in `(name, email, password)`
    // doesn't pop the PASSWORD reserved-word reference.
    if in_field_list_context(source, u32::from(offset) as usize) {
      // Grouping / window / sub-query operators are genuine SQL
      // keywords (not column names) and are useful to hover even
      // when they appear inside a GROUP BY / ORDER BY / RETURNING /
      // SET clause that otherwise suppresses keyword cards.
      //
      // Types (`int`, `text`, `varchar`, `jsonb`, `timestamptz`, ...)
      // appear in the type position of a CREATE TABLE column decl
      // and should also surface their docs -- they are NOT column
      // references. Check the knowledge entry's kind: TYPE entries
      // fall through to the lookup below; KEYWORD entries stay
      // suppressed so `password` in `(name, email, password)`
      // doesn't pop the PASSWORD reserved-word card.
      let utok = tok.to_ascii_uppercase();
      let is_grouping_op = matches!(utok.as_str(), "ROLLUP" | "CUBE" | "LATERAL" | "OVER");
      let is_type = dsl_knowledge::lookup(&tok).is_some_and(|e| e.kind == dsl_knowledge::Kind::Type);
      // Column-definition / table-constraint keywords inside a CREATE TABLE
      // body are real SQL keywords, not column-name uses, so they should
      // surface their docs the same as outside the paren. Cursor inside
      // `id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY` previously got
      // suppressed because the paren-context check assumed every word was
      // an identifier.
      let is_col_decl_kw = matches!(
        utok.as_str(),
        "PRIMARY"
          | "KEY"
          | "UNIQUE"
          | "REFERENCES"
          | "CHECK"
          | "NOT"
          | "NULL"
          | "DEFAULT"
          | "GENERATED"
          | "ALWAYS"
          | "BY"
          | "AS"
          | "STORED"
          | "IDENTITY"
          | "CONSTRAINT"
          | "COLLATE"
          | "DEFERRABLE"
          | "DEFERRED"
          | "INITIALLY"
          | "MATCH"
          | "ON"
          | "DELETE"
          | "UPDATE"
          | "CASCADE"
          | "RESTRICT"
          | "SET"
      );
      if !is_grouping_op && !is_type && !is_col_decl_kw {
        return None;
      }
    }
    if let Some(entry) = dsl_knowledge::lookup(&tok) {
      return Some(dsl_knowledge::render_markdown(entry));
    }
  }
  None
}

/// True when the cursor lies inside a parenthesised column list of:
///   INSERT INTO t (...)
///   CREATE TABLE t (...)
///   CREATE INDEX ... ON t (...)
///   CREATE UNIQUE INDEX ... ON t (...)
///   PRIMARY KEY (...)
///   UNIQUE (...)
///   FOREIGN KEY (...) REFERENCES other (...)
///   RETURNING ...
///   GROUP BY ...
///   ORDER BY ...
///   UPDATE ... SET ... (the LHS of an assignment)
///
/// Used to suppress the keyword fallback so `password`, `order`,
/// `role`, etc don't pop the reserved-word card when the user
/// clearly wrote them as identifiers.
fn in_field_list_context(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let mut i = pos.min(bytes.len());
  // Walk back finding unbalanced `(` or a clause keyword on the way.
  let mut depth = 0i32;
  while i > 0 {
    let b = bytes[i - 1];
    if !b.is_ascii() {
      i -= 1;
      continue;
    }
    let c = b as char;
    if c == ')' {
      depth += 1;
      i -= 1;
      continue;
    }
    if c == '(' {
      if depth == 0 {
        // Unbalanced `(` -- check what precedes the paren.
        return preceded_by_field_list_intro(src, i - 1);
      }
      depth -= 1;
      i -= 1;
      continue;
    }
    if c == ';' {
      return false;
    }
    // Clause-introducer keywords without needing a paren.
    if depth == 0 {
      for kw in ["RETURNING", "GROUP BY", "ORDER BY"] {
        let len = kw.len();
        // Guard against mid-codepoint slicing: only inspect when the
        // would-be start byte is a UTF-8 char boundary.
        if i >= len && src.is_char_boundary(i - len) {
          let slice = &src[i - len..i];
          if slice.to_ascii_uppercase() == kw {
            let prev_ok = i == len || !is_word_ch(bytes[i - len - 1]);
            if prev_ok {
              return true;
            }
          }
        }
      }
      // UPDATE ... SET <col> = ... : when cursor sits on a column
      // name on the LHS of a SET assignment.
      if i >= 4 && src.is_char_boundary(i - 4) && src[i - 4..i].eq_ignore_ascii_case(" SET") {
        return true;
      }
    }
    i -= 1;
  }
  false
}

fn is_word_ch(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn preceded_by_field_list_intro(src: &str, paren_pos: usize) -> bool {
  // Trim whitespace before `(`.
  let bytes = src.as_bytes();
  let mut k = paren_pos;
  while k > 0 && bytes[k - 1].is_ascii() && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  // Pull preceding word (could be the table name in CREATE TABLE t (...)
  // or the keyword we care about). Accept `.` as part of the word so that
  // schema-qualified names (`public.users`) are consumed as one token and
  // the head-trim lookup sees `CREATE TABLE` ahead of the paren.
  let word_end = k;
  while k > 0 && bytes[k - 1].is_ascii() && (is_word_ch(bytes[k - 1]) || bytes[k - 1] == b'.') {
    k -= 1;
  }
  let last_word = &src[k..word_end];
  let upper_word = last_word.to_ascii_uppercase();
  // INSERT INTO t (   -- the previous word is the table name; check
  // further back for INTO / TABLE / INDEX / KEY / UNIQUE / etc.
  let head_upper = src[..k].to_ascii_uppercase();
  let head_trimmed = head_upper.trim_end();
  for kw in [
    "INSERT INTO",
    "CREATE TABLE",
    "CREATE TABLE IF NOT EXISTS",
    "CREATE TEMP TABLE",
    "CREATE TEMPORARY TABLE",
    "CREATE INDEX",
    "CREATE UNIQUE INDEX",
    "CREATE INDEX IF NOT EXISTS",
    "CREATE UNIQUE INDEX IF NOT EXISTS",
    "PRIMARY KEY",
    "FOREIGN KEY",
    "UNIQUE",
    "REFERENCES",
    "USING",
  ] {
    if head_trimmed.ends_with(kw) {
      return true;
    }
    // Also: ON t ( for CREATE INDEX -- previous word is `t`, before
    // that `ON`. Need to step back another word.
  }
  // Two-word lookback: previous word may be `t` (table); look further.
  let mut k2 = k;
  while k2 > 0 && bytes[k2 - 1].is_ascii() && bytes[k2 - 1].is_ascii_whitespace() {
    k2 -= 1;
  }
  let word_end2 = k2;
  while k2 > 0 && bytes[k2 - 1].is_ascii() && is_word_ch(bytes[k2 - 1]) {
    k2 -= 1;
  }
  if word_end2 > k2 {
    let upper2 = src[k2..word_end2].to_ascii_uppercase();
    if upper2 == "ON" {
      // CREATE INDEX ... ON t (
      return true;
    }
  }
  let _ = upper_word;
  false
}

fn catalog_lookup(token: &str, catalog: &Catalog) -> Option<String> {
  // Schema-qualified 3-segment path: `schema.table.column`.
  let segs: Vec<&str> = token.split('.').collect();
  if segs.len() == 3 {
    let (schema, table, column) = (segs[0], segs[1], segs[2]);
    if let Some(t) = catalog.find_table(Some(schema), table) {
      if let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(column)) {
        return Some(render::column(t, c));
      }
      // 3rd segment is not a column -- still a valid table card.
      return Some(render::table_with_catalog(t, catalog));
    }
  }
  if let Some((left, right)) = token.split_once('.') {
    if let Some(t) = catalog.find_table(Some(left), right) {
      return Some(render::table_with_catalog(t, catalog));
    }
    if let Some(t) = catalog.find_table(None, left)
      && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(right))
    {
      return Some(render::column(t, c));
    }
  }
  if let Some(t) = catalog.find_table(None, token) {
    return Some(render::table(t));
  }
  if let Some(ty) = catalog.find_type(None, token) {
    return Some(render::user_type(ty));
  }
  // Schema card: cursor on a bare identifier that names a schema in
  // the catalog. Schema membership is detected through THREE sources
  // so the card fires even when `CREATE SCHEMA <name>` wasn't seen:
  //   1. catalog.schemas       (declared schemas)
  //   2. catalog.functions[*].schema (function definitions)
  //   3. catalog.types[*].schema     (user-defined types)
  let schema_known = catalog.schemas.iter().any(|s| s.name.eq_ignore_ascii_case(token))
    || catalog.functions.iter().any(|f| f.schema.eq_ignore_ascii_case(token))
    || catalog.types.iter().any(|t| t.schema.eq_ignore_ascii_case(token));
  if schema_known {
    let name = catalog
      .schemas
      .iter()
      .find(|s| s.name.eq_ignore_ascii_case(token))
      .map(|s| s.name.as_str())
      .unwrap_or(token);
    let mut card = format!("# `{name}`\n_schema_\n\n");
    let tables: Vec<&dsl_catalog::Table> = catalog
      .schemas
      .iter()
      .find(|s| s.name.eq_ignore_ascii_case(token))
      .map(|s| s.tables.iter().collect())
      .unwrap_or_default();
    if !tables.is_empty() {
      card.push_str(&format!("**{} table(s):**\n", tables.len()));
      for t in tables.iter().take(20) {
        card.push_str(&format!("- `{}`\n", t.name));
      }
      if tables.len() > 20 {
        card.push_str(&format!("- _\u{2026} +{} more_\n", tables.len() - 20));
      }
    }
    let fns: Vec<&dsl_catalog::Function> = catalog
      .functions
      .iter()
      .filter(|f| f.schema.eq_ignore_ascii_case(token))
      .collect();
    if !fns.is_empty() {
      card.push_str(&format!("\n**{} function(s):**\n", fns.len()));
      for f in fns.iter().take(20) {
        let ret = if f.return_type.is_empty() { String::new() } else { format!(" -> `{}`", f.return_type) };
        card.push_str(&format!("- `{}({} args)`{}\n", f.name, f.arguments.len(), ret));
      }
      if fns.len() > 20 {
        card.push_str(&format!("- _\u{2026} +{} more_\n", fns.len() - 20));
      }
    }
    let types_in_schema: Vec<&dsl_catalog::Type> =
      catalog.types.iter().filter(|t| t.schema.eq_ignore_ascii_case(token)).collect();
    if !types_in_schema.is_empty() {
      card.push_str(&format!("\n**{} type(s):**\n", types_in_schema.len()));
      for t in types_in_schema.iter().take(20) {
        card.push_str(&format!("- `{}`\n", t.name));
      }
    }
    return Some(card);
  }
  if let Some((t, p)) = catalog.find_policy(token) {
    return Some(format!(
      "# `{}`\n_RLS policy on `{}.{}`_\n\n- **command**: `{}`\n- **roles**: `{}`\n- **permissive**: `{}`{}{}\n",
      p.name,
      t.schema,
      t.name,
      p.command,
      p.roles,
      p.permissive,
      p.using_expr.as_ref().map(|e| format!("\n- **USING**: `{e}`")).unwrap_or_default(),
      p.check_expr.as_ref().map(|e| format!("\n- **WITH CHECK**: `{e}`")).unwrap_or_default(),
    ));
  }
  if let Some((t, tr)) = catalog.find_trigger(token) {
    let mut s = format!(
      "# `{}`\n_trigger on `{}.{}`_\n\n- **timing**: `{}`\n- **event**: `{}`\n- **granularity**: `{}`\n- **executes**: `{}`\n",
      tr.name, t.schema, t.name, tr.timing, tr.event, tr.granularity, tr.function,
    );
    // Append the handler function's source if it's in the catalog.
    let fn_name = tr.function.rsplit('.').next().unwrap_or(&tr.function);
    if let Some(f) = catalog.functions.iter().find(|f| f.name.eq_ignore_ascii_case(fn_name))
      && let Some(body) = f.comment.as_ref()
    {
      let trimmed = body.trim();
      if trimmed.to_ascii_uppercase().starts_with("CREATE") {
        s.push_str("\n**Handler function**\n\n```sql\n");
        s.push_str(trimmed);
        s.push_str("\n```\n");
      }
    }
    return Some(s);
  }
  if let Some((t, i)) = catalog.find_index(token) {
    let def = i.definition.as_deref().unwrap_or("");
    let lead = i.columns.first().cloned().unwrap_or_default();
    let mut s = format!(
      "# `{}`\n_index on `{}.{}`_\n\n- **columns**: `{}`\n- **unique**: `{}`\n",
      i.name,
      t.schema,
      t.name,
      i.columns.join(", "),
      i.unique,
    );
    if !def.is_empty() {
      s.push_str(&format!("\n```sql\n{def}\n```\n"));
    }
    // Workload hint: leading column drives B-tree usage. Equality +
    // range on the leading column hits this index; predicates that
    // touch only later columns do NOT use it for index access.
    if !lead.is_empty() {
      s.push_str(&format!(
        "\n**Best for**\n\n- `WHERE {lead} = ...`\n- `WHERE {lead} BETWEEN ... AND ...`\n- `ORDER BY {lead}` (no extra sort)\n",
      ));
      if i.columns.len() > 1 {
        s.push_str(&format!("- Multi-column equality starting from `{lead}` ({})\n", i.columns.join(", ")));
      }
      if i.unique {
        s.push_str(&format!("- `SELECT ... WHERE {lead} = ...` -> at most one row\n"));
      }
    }
    return Some(s);
  }
  if let Some((t, c)) = catalog.find_constraint(token) {
    let kind = match c.kind {
      dsl_catalog::ConstraintKind::PrimaryKey => "PRIMARY KEY",
      dsl_catalog::ConstraintKind::ForeignKey => "FOREIGN KEY",
      dsl_catalog::ConstraintKind::Unique => "UNIQUE",
      dsl_catalog::ConstraintKind::Check => "CHECK",
    };
    let mut s = format!(
      "# `{}`\n_{} constraint on `{}.{}`_\n\n- **columns**: `{}`\n",
      c.name,
      kind,
      t.schema,
      t.name,
      c.columns.join(", "),
    );
    if let Some(r) = &c.references {
      s.push_str(&format!("- **references**: `{}.{}` ({})\n", r.schema, r.table, r.columns.join(", "),));
    }
    if let Some(def) = &c.definition {
      s.push_str(&format!("\n```sql\n{def}\n```\n"));
    }
    return Some(s);
  }
  if let Some(s) = catalog.find_sequence(None, token) {
    let cycle = if s.cycle { "yes" } else { "no" };
    let owner = s.owned_by_column.as_deref().unwrap_or("(standalone)");
    return Some(format!(
      "# `{}.{}`\n_sequence_\n\n- **type**: `{}`\n- **start**: `{}`\n- **min**: `{}`\n- **max**: `{}`\n- **increment**: `{}`\n- **cycle**: `{}`\n- **owned by**: `{}`\n",
      s.schema, s.name, s.data_type, s.start_value, s.min_value, s.max_value, s.increment_by, cycle, owner,
    ));
  }
  if let Some(e) = catalog.extensions().find(|e| e.name.eq_ignore_ascii_case(token)) {
    let comment = e.comment.as_deref().map(|c| format!("\n_{c}_\n")).unwrap_or_default();
    return Some(format!(
      "# `{}`\n_extension installed in `{}`_\n\n- **version**: `{}`\n{}",
      e.name, e.schema, e.version, comment,
    ));
  }
  let cols = catalog.columns_named(token);
  if !cols.is_empty() {
    return Some(render::column_in_tables(&cols));
  }
  None
}

/// Cursor on the NULL keyword. Returns a small card explaining
/// three-valued logic + the destination column's nullability when
/// inferrable from INSERT VALUES / UPDATE SET / WHERE.
fn null_keyword_hover(source: &str, pos: usize, catalog: &Catalog) -> Option<String> {
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return None;
  }
  // Read the word under the cursor.
  let mut s = pos;
  while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_') {
    s -= 1;
  }
  let mut e = pos;
  while e < bytes.len() && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_') {
    e += 1;
  }
  if s == e {
    return None;
  }
  if !source[s..e].eq_ignore_ascii_case("NULL") {
    return None;
  }
  let mut card = "# `NULL`\n\n_unknown / absent value_\n\nSQL uses three-valued logic. \
     Any expression involving `NULL` (except `IS NULL` / `IS NOT NULL` / \
     `IS [NOT] DISTINCT FROM`) returns `NULL`, not true/false. Use \
     `COALESCE(x, default)` or `x IS NULL` for safe comparisons.\n"
    .to_string();
  if let Some((schema, table, col, ty)) = infer_assigned_column(source, s.saturating_sub(1), catalog) {
    card.push_str(&format!("\n- **assigned to:** `{schema}.{table}.{col}`\n- **column type:** `{ty}`\n",));
    // Find nullability flag in catalog.
    if let Some(t) = catalog.find_table(Some(&schema), &table)
      && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(&col))
      && !c.nullable
    {
      card.push_str(
            "\n_Warning:_ destination column is **NOT NULL** -- PG will reject this INSERT with `null value in column \"...\" violates not-null constraint`.\n",
          );
    }
  }
  Some(card)
}

/// Hover card for the four JSONB path operators -- ->, ->>, #>, #>>.
/// Each returns a different type so users get caught out by mixing
/// them (especially -> vs ->>). Fires when the cursor sits on any
/// byte of the operator.
/// Hover for common comparison / string / range operators that aren't
/// covered by the JSON path or `::` cast paths. Returns a brief
/// markdown card explaining the operator's meaning.
fn comparison_operator_hover(source: &str, pos: usize) -> Option<String> {
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return None;
  }
  // Find the maximal operator span (consecutive ASCII-punctuation chars
  // commonly used in PG operators) containing `pos`.
  let is_op_byte = |b: u8| matches!(b, b'=' | b'<' | b'>' | b'!' | b'|' | b'@' | b'?' | b'&' | b'~' | b'^' | b'#');
  if !is_op_byte(bytes[pos]) {
    return None;
  }
  let mut s = pos;
  while s > 0 && is_op_byte(bytes[s - 1]) {
    s -= 1;
  }
  let mut e = pos;
  while e < bytes.len() && is_op_byte(bytes[e]) {
    e += 1;
  }
  let op = &source[s..e];
  // Reject operators handled elsewhere (json-path `->` / `->>` / `#>` /
  // `#>>`, cast `::`). Bare `#` (bitwise XOR) is fine, only multi-char
  // forms involving `#` go to jsonb_operator_hover.
  if op.contains('-') || op.contains(':') || (op.contains('#') && op.len() > 1) {
    return None;
  }
  let card = match op {
    "=" => "# `=`\n\n_Equality comparison_\n\nReturns `true` when both operands are equal, \
            `false` when not, and `NULL` if either operand is `NULL` (three-valued logic). \
            Use `IS NOT DISTINCT FROM` to treat `NULL = NULL` as true.\n",
    "<>" | "!=" => {
      "# `<>` (also `!=`)\n\n_Inequality comparison_\n\nReturns `true` when operands differ. \
            Like `=`, returns `NULL` when either side is `NULL`. Use `IS DISTINCT FROM` for \
            NULL-aware inequality.\n"
    },
    "<" => "# `<`\n\n_Less-than comparison_\n\nStandard ordering on the operand type. \
            Uses lexicographic order for strings, byte order for `bytea`, and the type's natural \
            ordering for numbers / dates.\n",
    ">" => "# `>`\n\n_Greater-than comparison_\n\nMirror of `<`.\n",
    "<=" => "# `<=`\n\n_Less-than-or-equal comparison_\n",
    ">=" => "# `>=`\n\n_Greater-than-or-equal comparison_\n",
    "||" => {
      "# `||`\n\n_String / array concatenation_\n\nFor text operands: \
            `'a' || 'b'` -> `'ab'`. For arrays: `ARRAY[1,2] || ARRAY[3]` -> `{1,2,3}`. \
            Returns `NULL` if either operand is `NULL` (text); for arrays, prepending or \
            appending `NULL` returns the other array.\n"
    },
    "@>" => "# `@>`\n\n_Contains operator_\n\nReturns `true` when the left operand contains the \
            right (jsonb / arrays / ranges). `'{\"a\":1, \"b\":2}'::jsonb @> '{\"a\":1}'::jsonb` -> `true`.\n",
    "<@" => "# `<@`\n\n_Contained-by operator_\n\nMirror of `@>`. The left operand is contained by the right.\n",
    "?" => "# `?`\n\n_jsonb key existence_\n\nReturns `true` when the left jsonb has a top-level \
            key (or array element) equal to the right text. `'{\"a\":1}'::jsonb ? 'a'` -> `true`.\n",
    "?|" => "# `?|`\n\n_jsonb any-key existence_\n\nReturns `true` when ANY key in the right `text[]` is present in the left jsonb.\n",
    "?&" => "# `?&`\n\n_jsonb all-keys existence_\n\nReturns `true` when ALL keys in the right `text[]` are present in the left jsonb.\n",
    "@@" => "# `@@`\n\n_Full-text search match_\n\nReturns `true` when the right-hand `tsquery` matches the left-hand `tsvector`. \
            `to_tsvector('quick brown fox') @@ to_tsquery('fox & jump:*')`. Index with GIN on the tsvector for fast lookups.\n",
    "~" => "# `~`\n\n_POSIX regex match (case-sensitive)_\n\nReturns `true` when the left text matches the right regex. \
            Pair with `||` to build dynamic patterns; for case-insensitive use `~*`. Escape regex metachars with `\\`.\n",
    "~*" => "# `~*`\n\n_POSIX regex match (case-insensitive)_\n\n`name ~* '^a'` matches both `Alice` and `aaron`.\n",
    "!~" => "# `!~`\n\n_POSIX regex NON-match (case-sensitive)_\n\nInverse of `~`. NULL-propagates like the other comparison operators.\n",
    "!~*" => "# `!~*`\n\n_POSIX regex NON-match (case-insensitive)_\n\nInverse of `~*`.\n",
    "~~" => "# `~~`\n\n_Internal name of the `LIKE` operator_\n\n`x LIKE 'a%'` is syntactic sugar for `x ~~ 'a%'`. Prefer `LIKE` in source.\n",
    "~~*" => "# `~~*`\n\n_Internal name of the `ILIKE` operator_\n\n`x ILIKE 'a%'` is syntactic sugar for `x ~~* 'a%'`. Prefer `ILIKE` in source.\n",
    "!~~" => "# `!~~`\n\n_Internal name of `NOT LIKE`_\n",
    "!~~*" => "# `!~~*`\n\n_Internal name of `NOT ILIKE`_\n",
    "^" => "# `^`\n\n_Exponentiation_\n\n`2 ^ 10` -> `1024`. Returns `double precision` or `numeric` depending on operand types.\n",
    "&" => "# `&`\n\n_Bitwise AND_ (integer / bit / inet)\n\n`5 & 3` -> `1`. For inet: \
            address-bit AND between the two networks. For bit strings: per-position AND.\n",
    "|" => "# `|`\n\n_Bitwise OR_ (integer / bit / inet)\n\n`5 | 2` -> `7`. \
            (Do not confuse with `||` -- text/array concatenation.)\n",
    "#" => "# `#`\n\n_Bitwise XOR_ (integer / bit)\n\n`5 # 3` -> `6`. Returns the per-bit exclusive-or.\n",
    _ => return None,
  };
  Some(card.to_string())
}

fn jsonb_operator_hover(source: &str, pos: usize) -> Option<String> {
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return None;
  }
  let b = bytes[pos];
  if b != b'-' && b != b'>' && b != b'#' && b != b'@' && b != b'?' {
    return None;
  }
  // Find the operator span this byte belongs to.
  let mut s = pos;
  while s > 0 && matches!(bytes[s - 1], b'-' | b'>' | b'#' | b'@' | b'?') {
    s -= 1;
  }
  let mut e = pos;
  while e < bytes.len() && matches!(bytes[e], b'-' | b'>' | b'#' | b'@' | b'?') {
    e += 1;
  }
  let op = &source[s..e];
  match op {
    "->" => Some(
      "# `->`\n\n_JSON / JSONB path operator_\n\n\
       Returns the **JSON value** at the given key (or array index).\
       Result type matches the operand type -- `json -> 'k'` returns json, \
       `jsonb -> 'k'` returns jsonb.\n\n\
       ```sql\n\
       data -> 'profile'        -- jsonb\n\
       items -> 0               -- first array element as jsonb\n\
       data -> 'a' -> 'b'       -- chain\n\
       ```\n"
        .into(),
    ),
    "->>" => Some(
      "# `->>`\n\n_JSON path operator returning TEXT_\n\n\
       Same as `->` but always returns **text**, not json. Use this when \
       you need the unquoted string value.\n\n\
       ```sql\n\
       data ->> 'email'         -- text (not jsonb)\n\
       items ->> 0              -- first element coerced to text\n\
       ```\n\n\
       Compares with `=` against a string literal directly: `data ->> 'k' = 'x'`. \
       Compare with `->` against a json literal: `data -> 'k' = '\"x\"'::jsonb`.\n"
        .into(),
    ),
    "#>" => Some(
      "# `#>`\n\n_JSON path lookup (array form)_\n\n\
       Returns the **JSON value** at the path specified by an array of keys.\n\n\
       ```sql\n\
       data #> '{a,b,0}'        -- equivalent to data -> 'a' -> 'b' -> 0\n\
       ```\n"
        .into(),
    ),
    "#>>" => Some(
      "# `#>>`\n\n_JSON path lookup returning TEXT_\n\n\
       Same as `#>` but coerces the final value to text.\n\n\
       ```sql\n\
       data #>> '{profile,email}'   -- text\n\
       ```\n"
        .into(),
    ),
    "@?" => Some(
      "# `@?`\n\n_jsonpath predicate match_\n\n\
       Returns `true` when the right-side `jsonpath` matches *any* value in the left jsonb. \
       Companion of `@@` which returns the boolean of the path's filter expression.\n\n\
       ```sql\n\
       data @? '$.items[*] ? (@.qty > 0)'\n\
       ```\n"
        .into(),
    ),
    _ => None,
  }
}

/// Cursor on a geometric / math / range comparison operator that
/// the comparison_operator_hover byte set doesn't cover: `|/`, `||/`,
/// `@-@`, `##`, `&<`, `&>`, `&<|`, `|>>`, `<<|`, `&&`.
fn geometric_math_operator_hover(source: &str, pos: usize) -> Option<String> {
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return None;
  }
  let is_op_byte = |b: u8| matches!(b, b'|' | b'/' | b'@' | b'-' | b'#' | b'&' | b'<' | b'>' | b'?' | b'~' | b'=');
  if !is_op_byte(bytes[pos]) {
    return None;
  }
  let mut s = pos;
  while s > 0 && is_op_byte(bytes[s - 1]) {
    s -= 1;
  }
  let mut e = pos;
  while e < bytes.len() && is_op_byte(bytes[e]) {
    e += 1;
  }
  let op = &source[s..e];
  let card = match op {
    "|/" => "# `|/`\n\n_Square root_\n\n`|/ 25.0` -> `5`. Prefix operator. Prefer the SQL-standard `sqrt()` for readability.\n",
    "||/" => "# `||/`\n\n_Cube root_\n\n`||/ 27.0` -> `3`. Prefix operator. Use `cbrt()` for clarity.\n",
    "@-@" => "# `@-@`\n\n_Geometric length / circumference_\n\nReturns the length of a path / lseg / line, or the circumference of a circle. \
              `@-@ lseg '((0,0),(3,4))'` -> `5`.\n",
    "##" => "# `##`\n\n_Closest-point operator_\n\nReturns the point on the first object closest to the second.\n",
    "&<" => "# `&<`\n\n_Overlaps-to-left_ (geometric / range)\n\nReturns `true` when the left operand does not extend to the right of the right operand.\n",
    "&>" => "# `&>`\n\n_Overlaps-to-right_ (geometric / range)\n\nMirror of `&<`.\n",
    "&<|" => "# `&<|`\n\n_Overlaps-below_ (geometric)\n\nLeft operand does not extend above the right.\n",
    "|>>" => "# `|>>`\n\n_Strictly above_ (geometric / range)\n\nLeft operand is strictly above the right.\n",
    "<<|" => "# `<<|`\n\n_Strictly below_ (geometric / range)\n\nLeft operand is strictly below the right.\n",
    "&&" => "# `&&`\n\n_Overlaps_ (range / array / multirange)\n\nReturns `true` when the two ranges (or arrays) share at least one element. \
              `int4range(1,10) && int4range(5,20)` -> `true`. Index with GiST/SP-GiST for fast lookups.\n",
    "<<" => "# `<<`\n\n_Strictly left of_ (range / multirange / inet)\n\n`int4range(1,5) << int4range(10,20)` -> `true`. \
              For inet: subnet is strictly to the left of the other.\n",
    ">>" => "# `>>`\n\n_Strictly right of_ (range / multirange / inet)\n\nMirror of `<<`.\n",
    "-|-" => "# `-|-`\n\n_Adjacent to_ (range / multirange)\n\nReturns `true` when the two ranges abut but do not overlap. \
              `int4range(1,5) -|- int4range(5,10)` -> `true`.\n",
    "?-" => "# `?-`\n\n_Horizontal alignment_ (geometric)\n\nReturns `true` when the operands are on the same horizontal line. \
              `point '(1,0)' ?- point '(5,0)'` -> `true`.\n",
    "?|" => "# `?|`\n\n_Vertical alignment_ (geometric -- shared with jsonb any-key existence)\n\nFor points/lines: \
              true when the operands are on the same vertical line. For jsonb: see jsonb path operator card.\n",
    "?-|" => "# `?-|`\n\n_Perpendicular_ (line/lseg)\n\nReturns `true` when the two segments / lines are perpendicular.\n",
    "?||" => "# `?||`\n\n_Parallel_ (line/lseg)\n\nReturns `true` when the two segments / lines are parallel.\n",
    "?#" => "# `?#`\n\n_Intersects_ (geometric)\n\nReturns `true` when the two objects intersect at a point or share an edge.\n",
    "@" => "# `@`\n\n_Center of_ / contained-by (geometric)\n\nUnary: `@ box '(...)'` returns its center point. Binary: `<obj> @ <obj>` -- left contained by right (geometric).\n",
    "~=" => "# `~=`\n\n_Same as_ (geometric)\n\nReturns `true` when the two geometric objects are equal (point-wise).\n",
    ">>=" => "# `>>=`\n\n_Contains-or-equals_ (inet)\n\n`192.168.0.0/16 >>= 192.168.1.0/24` -> `true`. \
              Left network contains, or equals, the right. Use for CIDR membership checks.\n",
    "<<=" => "# `<<=`\n\n_Contained-by-or-equals_ (inet)\n\nMirror of `>>=`. The left network is contained by, \
              or equals, the right.\n",
    _ => return None,
  };
  Some(card.to_string())
}

/// Cursor on `<->`, `<#>`, `<%>`, `<<->>`, `<<#>>` operators (KNN
/// distance / phrase / pg_trgm). Surface a brief explanation.
fn distance_operator_hover(source: &str, pos: usize) -> Option<String> {
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return None;
  }
  let b = bytes[pos];
  if !matches!(b, b'<' | b'>' | b'-' | b'#' | b'%') {
    return None;
  }
  let mut s = pos;
  while s > 0 && matches!(bytes[s - 1], b'<' | b'>' | b'-' | b'#' | b'%') {
    s -= 1;
  }
  let mut e = pos;
  while e < bytes.len() && matches!(bytes[e], b'<' | b'>' | b'-' | b'#' | b'%') {
    e += 1;
  }
  let op = &source[s..e];
  let card = match op {
    "<->" => "# `<->`\n\n_KNN distance / phrase distance_\n\nGeometric distance (point/box/circle), \
              vector L2 distance (pgvector), pg_trgm similarity-distance, or FTS phrase distance \
              (immediately adjacent). Index with GiST/SP-GiST/ivfflat for KNN ordering.\n\n\
              ```sql\n\
              ORDER BY position <-> point '(0,0)' LIMIT 10;\n\
              SELECT * FROM articles WHERE doc @@ to_tsquery('quick <-> brown');\n\
              ```\n",
    "<#>" => "# `<#>`\n\n_pgvector inner-product distance_\n\nNegative dot product. Use with ivfflat / hnsw \
              vector indexes for fast approximate nearest neighbour.\n",
    "<%>" => "# `<%>`\n\n_pg_trgm trigram word-similarity distance_\n\n`1 - word_similarity(a, b)`. \
              Pair with `ORDER BY a <%> b LIMIT 10` for fuzzy lookup.\n",
    "<<->>" => "# `<<->>`\n\n_GiST KNN distance for ranges_\n\nDistance between two range values.\n",
    "<<#>>" => "# `<<#>>`\n\n_Index-only KNN distance variant_\n",
    _ => return None,
  };
  Some(card.to_string())
}

/// Numeric literal hover. Cursor on `123` / `12.5` / `-7` returns a
/// tiny card naming kind (integer / float) + the assigned column
/// type when in INSERT VALUES / UPDATE SET / WHERE.
fn numeric_literal_hover(source: &str, pos: usize, catalog: &Catalog) -> Option<String> {
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return None;
  }
  let here = bytes[pos];
  if !here.is_ascii_digit() && here != b'.' {
    return None;
  }
  // Walk back over digits / `.` / `-+`.
  let mut s = pos;
  while s > 0 && (bytes[s - 1].is_ascii_digit() || bytes[s - 1] == b'.') {
    s -= 1;
  }
  if s > 0 && (bytes[s - 1] == b'-' || bytes[s - 1] == b'+') {
    // Only treat as sign when preceded by space / operator / `(` /
    // `,` -- not when it's a subtraction in `a-1`.
    if s == 1 || matches!(bytes[s - 2], b' ' | b'\t' | b'\n' | b',' | b'(' | b'=' | b'<' | b'>') {
      s -= 1;
    }
  }
  let mut e = pos;
  while e < bytes.len() && (bytes[e].is_ascii_digit() || bytes[e] == b'.') {
    e += 1;
  }
  let lit = &source[s..e];
  if lit.is_empty() || lit == "-" || lit == "+" || lit == "." {
    return None;
  }
  let kind = if lit.contains('.') { "float / numeric" } else { "integer" };
  let mut card = format!("# `{lit}`\n\n_{kind} literal_\n");
  if let Some((schema, table, col, ty)) = infer_assigned_column(source, s.saturating_sub(1), catalog) {
    card.push_str(&format!("\n- **assigned to:** `{schema}.{table}.{col}`\n- **column type:** `{ty}`\n",));
  }
  Some(card)
}

/// String-literal hover card. Resolves the literal's destination
/// column when the cursor sits inside a `'...'` in INSERT VALUES /
/// UPDATE SET / WHERE / ON predicates, and surfaces the column's
/// declared type. Falls back to a generic "text/string literal"
/// card when no destination can be inferred.
fn string_literal_hover(source: &str, pos: usize, catalog: &Catalog) -> Option<String> {
  let bytes = source.as_bytes();
  // Find the bounds of the enclosing single-quoted literal.
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' {
    s -= 1;
  }
  if s == 0 {
    return None;
  }
  let lit_start = s; // first byte inside the quotes
  let mut e = pos;
  while e < bytes.len() && bytes[e] != b'\'' {
    e += 1;
  }
  let lit_end = e;
  let lit = if lit_end > lit_start { &source[lit_start..lit_end] } else { "" };

  // Walk back from the opening quote to find the destination column.
  let dest_col = infer_assigned_column(source, lit_start - 1, catalog);
  let header = format!("# `'{}'`\n\n_text/string literal_\n", lit.chars().take(60).collect::<String>());
  let mut body = header;
  if let Some((schema, table, col, ty)) = dest_col {
    body.push_str(&format!("\n- **assigned to:** `{schema}.{table}.{col}`\n- **column type:** `{ty}`\n",));
    // Compatibility heuristic.
    let lower = ty.to_ascii_lowercase();
    if lower.contains("int")
      || lower.contains("numeric")
      || lower.contains("decimal")
      || lower.contains("real")
      || lower.contains("double")
      || lower.contains("float")
    {
      body.push_str(
        "\n_Warning:_ destination is numeric -- the literal will be cast at runtime; \
         malformed strings raise an `invalid input syntax` error.\n",
      );
    } else if lower.contains("bool") {
      body.push_str(
        "\n_Warning:_ destination is boolean -- PG accepts only `'t'`/`'f'`/`'true'`/\
         `'false'`/`'yes'`/`'no'`/`'y'`/`'n'`/`'on'`/`'off'`/`'1'`/`'0'`.\n",
      );
    }
  }
  Some(body)
}

/// Walk back from the byte just before the literal's opening quote
/// to figure out which column the literal is being assigned to.
/// Returns `(schema, table, col, type)` when resolvable.
fn infer_assigned_column(
  source: &str,
  before_quote: usize,
  catalog: &Catalog,
) -> Option<(String, String, String, String)> {
  let bytes = source.as_bytes();
  // Strip whitespace + commas back.
  let mut i = before_quote;
  while i > 0 && bytes[i - 1].is_ascii_whitespace() {
    i -= 1;
  }
  // Three contexts: INSERT VALUES (positional), UPDATE SET col = '...',
  // WHERE/ON col = '...'.
  // For SET/WHERE: previous non-ws should be `=`; before that an
  // identifier (possibly qualified).
  if i > 0 && bytes[i - 1] == b'=' {
    let mut k = i - 1;
    while k > 0 && bytes[k - 1].is_ascii_whitespace() {
      k -= 1;
    }
    let id_end = k;
    while k > 0
      && (bytes[k - 1].is_ascii_alphanumeric() || bytes[k - 1] == b'_' || bytes[k - 1] == b'.' || bytes[k - 1] == b'"')
    {
      k -= 1;
    }
    let raw = &source[k..id_end];
    let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"');
    if bare.is_empty() {
      return None;
    }
    // Look back further for the FROM / UPDATE / INSERT context to pin
    // down which table the column lives in.
    if let Some(table) = enclosing_table_name(source, k)
      && let Some(t) = catalog.find_table(None, &table)
      && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(bare))
    {
      return Some((t.schema.clone(), t.name.clone(), c.name.clone(), c.data_type.clone()));
    }
    // Fall back: search every catalog table for the column name.
    let hits = catalog.columns_named(bare);
    if hits.len() == 1 {
      let (t, c) = hits[0];
      return Some((t.schema.clone(), t.name.clone(), c.name.clone(), c.data_type.clone()));
    }
    return None;
  }
  None
}

/// Best-effort: walk back through statement and return the latest
/// table name following UPDATE / INSERT INTO / FROM keywords.
fn enclosing_table_name(source: &str, before: usize) -> Option<String> {
  let stmt_start = source[..before].rfind(';').map(|i| i + 1).unwrap_or(0);
  let slice = &source[stmt_start..before];
  let upper = slice.to_ascii_uppercase();
  for kw in ["UPDATE ", "INSERT INTO ", "FROM "] {
    if let Some(at) = upper.rfind(kw) {
      let after = &slice[at + kw.len()..];
      let lead = after.len() - after.trim_start().len();
      let raw = &after[lead..];
      let id_end =
        raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
      if id_end == 0 {
        continue;
      }
      let bare = raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"');
      return Some(bare.to_string());
    }
  }
  None
}

/// True when `pos` sits strictly inside a `"..."` double-quoted
/// identifier (cursor on the opening or closing quote returns false).
/// Walks byte-by-byte from BOF skipping single-quoted strings, line
/// comments, block comments, and dollar-quoted bodies so a `"` byte
/// nested in any of those doesn't open a phantom identifier.
fn cursor_inside_double_quoted_ident(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let cap = pos.min(n);
  let mut i = 0usize;
  while i < n {
    let c = bytes[i];
    // Skip `-- line comment`.
    if c == b'-' && i + 1 < n && bytes[i + 1] == b'-' {
      let mut j = i + 2;
      while j < n && bytes[j] != b'\n' {
        j += 1;
      }
      i = j;
      continue;
    }
    // Skip `/* block comment */`.
    if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
      let mut j = i + 2;
      while j + 1 < n && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
        j += 1;
      }
      i = (j + 2).min(n);
      continue;
    }
    // Skip single-quoted string `'...'` with `''` escape.
    if c == b'\'' {
      let mut j = i + 1;
      while j < n {
        if bytes[j] == b'\'' {
          if j + 1 < n && bytes[j + 1] == b'\'' {
            j += 2;
            continue;
          }
          j += 1;
          break;
        }
        j += 1;
      }
      i = j;
      continue;
    }
    // Skip dollar-quoted body `$tag$ ... $tag$`.
    if c == b'$' {
      let mut j = i + 1;
      while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if j < n && bytes[j] == b'$' {
        let tag = &bytes[i..=j];
        let body_start = j + 1;
        let mut k = body_start;
        while k + tag.len() <= n && &bytes[k..k + tag.len()] != tag {
          k += 1;
        }
        i = (k + tag.len()).min(n);
        continue;
      }
    }
    // Double-quoted identifier `"..."` with `""` escape.
    if c == b'"' {
      let open = i;
      let mut j = i + 1;
      while j < n {
        if bytes[j] == b'"' {
          if j + 1 < n && bytes[j + 1] == b'"' {
            j += 2;
            continue;
          }
          break;
        }
        j += 1;
      }
      let close = j; // index of closing `"` or n if unterminated.
      if cap > open && cap < close {
        return true;
      }
      i = (close + 1).min(n);
      continue;
    }
    i += 1;
  }
  false
}

/// True when `pos` sits inside a single-quoted string literal, a
/// double-quoted identifier, a `-- line comment`, or a `/* block
/// comment */`. Walk byte-by-byte from BOF tracking the current
/// lex mode; cheap and exact.
fn inside_string_or_comment(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let n = bytes.len().min(pos.max(pos));
  let mut i = 0usize;
  let cap = pos.min(bytes.len());
  while i < cap {
    let c = bytes[i];
    // Line comment runs to end of line.
    if c == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
      let mut j = i + 2;
      while j < bytes.len() && bytes[j] != b'\n' {
        j += 1;
      }
      if pos > i && pos <= j {
        return true;
      }
      i = j + 1;
      continue;
    }
    // Block comment.
    if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
      let start = i;
      let mut j = i + 2;
      while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
        j += 1;
      }
      let end = (j + 2).min(bytes.len());
      if pos > start && pos < end {
        return true;
      }
      i = end;
      continue;
    }
    // Single-quoted string. `''` is an escape; treat as continued.
    if c == b'\'' {
      let start = i;
      i += 1;
      while i < bytes.len() {
        if bytes[i] == b'\'' {
          if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
            i += 2;
            continue;
          }
          i += 1;
          break;
        }
        i += 1;
      }
      if pos > start && pos < i {
        return true;
      }
      continue;
    }
    // Dollar-quoted body `$tag$ ... $tag$`. PL/pgSQL function bodies
    // and DO blocks live in this form. They contain *code*, not
    // literal text, so the body itself is NOT inert -- hovering on a
    // PL/pgSQL keyword like RAISE / EXCEPTION / PERFORM inside the
    // body should still surface its knowledge card. We only treat
    // string literals / comments WITHIN the body as inert; recurse
    // with the body slice so they're detected at the inner cursor.
    if c == b'$' {
      let tag_start = i;
      let mut j = i + 1;
      while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if j < bytes.len() && bytes[j] == b'$' {
        let tag = &src[tag_start..=j];
        let body_start = j + 1;
        let mut k = body_start;
        while k + tag.len() <= bytes.len() && &src[k..k + tag.len()] != tag {
          k += 1;
        }
        let body_end = k;
        if pos >= body_start && pos < body_end {
          // Cursor inside the body -- recurse so an inner `'literal'`
          // / `-- comment` / `/* ... */` still suppresses hover, but
          // keyword positions stay live.
          return inside_string_or_comment(&src[body_start..body_end], pos - body_start);
        }
        i = (k + tag.len()).min(bytes.len());
        continue;
      }
    }
    i += 1;
  }
  let _ = n;
  false
}

/// True when the cursor sits exactly on a `:` byte that's part of
/// the `::` cast operator (either the first or second colon).
fn cursor_on_double_colon(source: &str, offset: TextSize) -> bool {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  if pos >= bytes.len() {
    return false;
  }
  if bytes[pos] != b':' {
    return false;
  }
  let prev = if pos > 0 { bytes[pos - 1] } else { 0 };
  let next = if pos + 1 < bytes.len() { bytes[pos + 1] } else { 0 };
  prev == b':' || next == b':'
}

/// Hover handler for the `*` token. Three flavours:
///   * `SELECT *` (no qualifier) -> list every column of every FROM
///     table in the current statement.
///   * `<alias>.*` -> list every column of `alias`'s bound table.
///   * `count(*)` / `count(*)`-like aggregates -> render the function
///     card explaining the implicit-all semantics.
///
/// Returns None when the cursor is not on a star.
fn star_hover(source: &str, offset: TextSize, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  if pos >= bytes.len() || bytes[pos] != b'*' {
    return None;
  }
  // Walk back over whitespace; if the preceding non-ws char is `.`,
  // grab the qualifier (alias / table).
  let mut k = pos;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k > 0 && bytes[k - 1] == b'.' {
    let id_end = k - 1;
    let mut id_start = id_end;
    while id_start > 0 {
      let c = bytes[id_start - 1] as char;
      if c.is_ascii_alphanumeric() || c == '_' || c == '"' {
        id_start -= 1;
      } else {
        break;
      }
    }
    if id_start < id_end {
      let qualifier = source[id_start..id_end].trim_matches('"');
      if let Some(md) = qualified_star_hover(source, offset, qualifier, catalog) {
        return Some(md);
      }
    }
    let _ = id_end;
  }
  // Walk back over whitespace; if preceded by `(`, look for the
  // function call name before the paren (`count(`, `avg(`, etc).
  let mut k = pos;
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k > 0 && bytes[k - 1] == b'(' {
    let paren = k - 1;
    let mut fn_end = paren;
    while fn_end > 0 && bytes[fn_end - 1].is_ascii_whitespace() {
      fn_end -= 1;
    }
    let mut fn_start = fn_end;
    while fn_start > 0 {
      let c = bytes[fn_start - 1] as char;
      if c.is_ascii_alphanumeric() || c == '_' {
        fn_start -= 1;
      } else {
        break;
      }
    }
    if fn_start < fn_end {
      let fname = &source[fn_start..fn_end];
      let mut card = format!("# `{fname}(*)`\n\n_aggregate-over-all-rows form_\n\n",);
      if let Some(entry) = dsl_knowledge::lookup(fname) {
        card.push_str(&dsl_knowledge::render_markdown(entry));
        card.push_str("\n\n");
      }
      card.push_str(
        "`*` here means \"count every row\" (no column referenced). \
         Equivalent to `count(1)` for COUNT; for other aggregates the \
         expansion depends on the function.",
      );
      return Some(card);
    }
  }
  // Bare `*` in a SELECT projection.
  unqualified_star_hover(source, offset, catalog)
}

fn qualified_star_hover(source: &str, offset: TextSize, alias: &str, catalog: &Catalog) -> Option<String> {
  // Resolve alias -> bound table via the existing alias_lookup path,
  // but we want the column list, not the table card. Find the
  // binding directly.
  let pos: usize = u32::from(offset) as usize;
  let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve(&parsed.statements);
  let idx = parsed.statements.iter().position(|s| {
    let lo: u32 = s.range.start().into();
    let hi: u32 = s.range.end().into();
    pos >= lo as usize && pos <= hi as usize
  });
  let table_name = if let Some(i) = idx {
    scopes.get(i).and_then(|scope| {
      scope.bindings.iter().find_map(|(k, v)| {
        if v.alias.eq_ignore_ascii_case(alias) || k.eq_ignore_ascii_case(alias) {
          Some(v.table.name.clone())
        } else {
          None
        }
      })
    })
  } else {
    None
  };
  let table_name = table_name.unwrap_or_else(|| alias.to_string());
  let table = catalog.find_table(None, &table_name)?;
  let cols: Vec<String> = table.columns.iter().map(|c| format!("{alias}.{}", c.name)).collect();
  if cols.is_empty() {
    return None;
  }
  Some(format!(
    "# `{alias}.*`\n\n_expands to every column of `{}.{}`_\n\n```sql\n{}\n```\n",
    table.schema,
    table.name,
    cols.join(",\n"),
  ))
}

fn unqualified_star_hover(source: &str, offset: TextSize, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  // Find the enclosing SELECT statement.
  let stmt = parsed.statements.iter().find(|s| {
    let lo: u32 = s.range.start().into();
    let hi: u32 = s.range.end().into();
    pos >= lo as usize && pos <= hi as usize
  })?;
  let dsl_parse::StatementKind::Select(sel) = &stmt.kind else { return None };
  let mut lines: Vec<String> = Vec::new();
  for from in &sel.from {
    let alias = from.alias.as_deref().unwrap_or(&from.name);
    if let Some(table) = catalog.find_table(from.schema.as_deref(), &from.name) {
      for c in &table.columns {
        lines.push(format!("{alias}.{}", c.name));
      }
    }
  }
  for join in &sel.joins {
    let alias = join.table.alias.as_deref().unwrap_or(&join.table.name);
    if let Some(table) = catalog.find_table(join.table.schema.as_deref(), &join.table.name) {
      for c in &table.columns {
        lines.push(format!("{alias}.{}", c.name));
      }
    }
  }
  if lines.is_empty() {
    return None;
  }
  Some(format!("# `*`\n\n_expands to every column of every FROM/JOIN table_\n\n```sql\n{}\n```\n", lines.join(",\n"),))
}

fn role_hover(source: &str, offset: TextSize, token: &str, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  if !near_role_slot(source, pos) {
    return None;
  }
  let role_norm = token.to_ascii_lowercase();
  let in_catalog = catalog.roles.iter().any(|r| r.eq_ignore_ascii_case(&role_norm));
  let is_builtin_postgres = role_norm == "postgres";
  let is_pg_internal = role_norm.starts_with("pg_");
  let is_pseudo = role_norm == "public";
  let is_session_kw = matches!(role_norm.as_str(), "current_user" | "session_user" | "current_role");

  let label = if is_pseudo {
    "_pseudo-role_ -- every existing role and every future role"
  } else if is_session_kw {
    "_session built-in_ -- resolves to the role that owns this connection"
  } else if is_builtin_postgres {
    "_bootstrap superuser_ -- created at initdb time"
  } else if is_pg_internal {
    "_built-in group role_ -- created by initdb / Postgres extensions"
  } else if in_catalog {
    "_role_ -- present in pg_roles"
  } else if catalog.roles.is_empty() {
    "_role_ -- catalog not loaded (offline?), cannot verify"
  } else {
    "_role_ -- **not found** in pg_roles (sql169 will flag this)"
  };
  Some(format!("# `{token}`\n{label}\n"))
}

/// True when the cursor's surrounding tokens look like a role-name slot
/// in a DDL or session statement. Inspects up to ~60 chars before the
/// cursor for one of the role-introducing keyword phrases.
fn near_role_slot(source: &str, pos: usize) -> bool {
  let take = 60usize.min(pos);
  let start = pos.saturating_sub(take);
  // Snap to a char boundary so the slice doesn't panic on multi-byte
  // chars in a free-form comment / string above the cursor.
  let mut start = start;
  while start < pos && !source.is_char_boundary(start) {
    start += 1;
  }
  let mut end = pos;
  while end > start && !source.is_char_boundary(end) {
    end -= 1;
  }
  let window = source[start..end].to_ascii_uppercase();
  // Bare role-introducing keyword phrases.
  for kw in ["OWNER TO", "GRANT", "REVOKE", "SET ROLE", "RESET ROLE", "POLICY", " TO "] {
    if window.contains(kw) {
      return true;
    }
  }
  // `FROM` is a role-list slot only when paired with REVOKE
  // (`REVOKE ... FROM <role>`). Bare `FROM` appears in every SELECT
  // statement and must NOT collapse hover into the role-card path.
  // We still cover the explicit REVOKE case via the loop above; this
  // branch is just a no-op now.
  false
}

/// Find the CREATE TABLE body that encloses `offset` and resolve the
/// column reference against it (using either the live catalog row when
/// available, or the parsed buffer ColumnDef when not).
fn enclosing_table_column(source: &str, offset: TextSize, token: &str, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  for stmt in &parsed.statements {
    let s: u32 = stmt.range.start().into();
    let e: u32 = stmt.range.end().into();
    if pos < s as usize || pos > e as usize {
      continue;
    }
    if let dsl_parse::StatementKind::CreateTable(ct) = &stmt.kind {
      // Prefer the live catalog row so we get types / nullability /
      // FK info. Fall back to the buffer ColumnDef when the table
      // isn't yet in the catalog.
      if let Some(t) = catalog.find_table(None, &ct.table.name)
        && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token))
      {
        return Some(render::column(t, c));
      }
      if let Some(col) = ct.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token)) {
        return Some(render::column_decl(&ct.table.name, col));
      }
    }
  }
  None
}

/// If `token` is a FROM / JOIN alias in the enclosing statement, render
/// the hover for the actual underlying table instead of "no match". When
/// the cursor sits inside a `$$ ... $$` function body, the body text is
/// re-parsed standalone so the alias resolves against its inner SELECT.
fn alias_lookup(source: &str, offset: TextSize, token: &str, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  // First try resolving in the top-level statement at this offset.
  if let Some(t) = resolve_alias_in(source, pos, token, catalog) {
    return Some(t);
  }
  // Fallback: re-parse the dollar-quoted body the cursor sits in.
  if let Some((body_start, body_end)) = enclosing_dollar_body(source, pos) {
    let body = &source[body_start..body_end];
    let body_pos = pos.saturating_sub(body_start);
    if let Some(t) = resolve_alias_in(body, body_pos, token, catalog) {
      return Some(t);
    }
  }
  None
}

/// When `tok` is a dotted identifier (`a.b` or longer), return only the
/// segment the cursor sits in. Returns `None` when `tok` has no `.`.
/// Cursor exactly on the dot snaps to the left side.
fn dotted_part_under_cursor(source: &str, offset: TextSize, tok: &str) -> Option<String> {
  if !tok.contains('.') {
    return None;
  }
  // Re-derive the token's byte range from the source so the cursor
  // offset is interpreted in source coordinates, not token-local.
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  let mut start = pos;
  while start > 0 {
    let c = bytes[start - 1] as char;
    if c.is_alphanumeric() || c == '_' || c == '.' {
      start -= 1;
    } else {
      break;
    }
  }
  let mut end = pos;
  while end < bytes.len() {
    let c = bytes[end] as char;
    if c.is_alphanumeric() || c == '_' || c == '.' {
      end += 1;
    } else {
      break;
    }
  }
  if pos < start || pos > end {
    return None;
  }
  // Walk segments left-to-right; return the one containing the cursor.
  let mut seg_start = start;
  for (i, b) in source[start..end].bytes().enumerate() {
    if b == b'.' {
      let abs = start + i;
      if pos <= abs {
        return Some(source[seg_start..abs].to_string());
      }
      seg_start = abs + 1;
    }
  }
  Some(source[seg_start..end].to_string())
}

/// Scope-aware column hover for `tok` (bare column or `alias.col`).
///
/// Parses + resolves the statement enclosing `offset`, looks up `tok`
/// against its FROM/JOIN bindings, and returns a single-column hover
/// when the column belongs to exactly one in-scope table. Falls back to
/// `None` so the catalog-wide hover (which shows the "in N tables"
/// table when ambiguous) still runs.
fn scope_column_lookup(source: &str, offset: TextSize, tok: &str, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&parsed.statements, source);
  let idx = parsed.statements.iter().position(|s| {
    let lo: u32 = s.range.start().into();
    let hi: u32 = s.range.end().into();
    pos >= lo as usize && pos <= hi as usize
  })?;
  let scope = scopes.get(idx)?;

  // Qualified form: `alias.col` -- resolve alias, then look up col.
  if let Some((left, right)) = tok.split_once('.') {
    if right.is_empty() {
      return None;
    }
    // NEW / OLD virtual row aliases inside a CREATE TRIGGER ... WHEN
    // predicate or a trigger function body. The bare alias has no
    // FROM binding; instead it stands for the trigger's target table
    // row. Resolve via the same path as the completion engine: walk
    // back to the enclosing `CREATE [OR REPLACE | CONSTRAINT] TRIGGER
    // ... ON <tbl>` (or, in a function body, the trigger declared to
    // run that function) and render the column card.
    let upper_left = left.to_ascii_uppercase();
    if upper_left == "NEW" || upper_left == "OLD" {
      if let Some(target) = trigger_target_for_hover(source, pos, catalog)
        && let Some(t) = catalog.find_table(None, &target)
        && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(right))
      {
        return Some(render::column(t, c));
      }
      return None;
    }
    // CTE-qualified column? Render a CTE card showing the alias and
    // the projected column name.
    if let Some(cte_cols) = scope.cte_columns_of(left)
      && cte_cols.iter().any(|c| c.eq_ignore_ascii_case(right))
    {
      let cols_md = if cte_cols.is_empty() {
        String::from("_columns not parsed_")
      } else {
        cte_cols.iter().map(|c| format!("- `{c}`")).collect::<Vec<_>>().join("\n")
      };
      return Some(format!("# `{left}.{right}`\n_CTE column_\n\n**`{left}`** projects:\n\n{cols_md}\n"));
    }
    if let Some(binding) = scope.bindings.iter().find_map(|(k, v)| {
      if v.alias.eq_ignore_ascii_case(left) || k.eq_ignore_ascii_case(left) { Some(v) } else { None }
    }) && let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name)
      && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(right))
    {
      return Some(render::column(t, c));
    }
    return None;
  }

  // Bare column: scan every in-scope table for a unique match.
  let mut hit: Option<(&dsl_catalog::Table, &dsl_catalog::Column)> = None;
  for b in scope.tables() {
    if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name)
      && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(tok))
    {
      if hit.is_some() {
        // Ambiguous within scope -- let catalog_lookup render
        // the multi-table disambiguation card.
        return None;
      }
      hit = Some((t, c));
    }
  }
  hit.map(|(t, c)| render::column(t, c))
}

/// Run the parse / resolve pipeline against `src`, find the statement
/// containing `pos`, and return the underlying-table hover for `token`
/// when it's an alias in that statement.
fn resolve_alias_in(src: &str, pos: usize, token: &str, catalog: &Catalog) -> Option<String> {
  let parsed = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&parsed.statements, src);
  let idx = parsed.statements.iter().position(|s| {
    let lo: u32 = s.range.start().into();
    let hi: u32 = s.range.end().into();
    pos >= lo as usize && pos <= hi as usize
  })?;
  let scope = scopes.get(idx)?;
  let binding = scope.bindings.iter().find_map(|(k, v)| {
    if v.alias.eq_ignore_ascii_case(token) || k.eq_ignore_ascii_case(token) {
      // For real catalog tables, "alias == table name" means the user
      // is hovering the table itself (not an alias) -- the catalog
      // path handles that. Synthetic bindings (subquery alias / CTE /
      // function-call FROM) have table.name == alias by construction,
      // so skip this filter for them.
      let is_synthetic = v.table.schema.as_deref().is_some_and(|s| s.starts_with('<'))
        || scope.cte_columns_of(&v.table.name).is_some();
      if !is_synthetic && v.table.name.eq_ignore_ascii_case(token) {
        return None;
      }
      Some(v)
    } else {
      None
    }
  })?;
  if let Some(t) = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name) {
    return Some(render::table(t));
  }
  // Synthetic bindings: subquery alias (`<subq>`), function-call FROM
  // alias (`<func>`), or CTE name -- render a brief card so hovering
  // the alias name doesn't silently return None.
  if let Some(card) = synthetic_alias_card(scope, binding, token) {
    return Some(card);
  }
  buffer_object(src, &binding.table.name)
}

fn synthetic_alias_card(scope: &dsl_resolve::Scope, binding: &dsl_resolve::binding::Binding, token: &str) -> Option<String> {
  // CTE: the binding name matches a cte_columns_of entry. Render the
  // projected columns so the user can see what `t.<col>` would be.
  if let Some(cols) = scope.cte_columns_of(&binding.table.name) {
    let cols_md = if cols.is_empty() {
      String::from("_columns not parsed_")
    } else {
      cols.iter().map(|c| format!("- `{c}`")).collect::<Vec<_>>().join("\n")
    };
    return Some(format!("# `{token}`\n\n_CTE alias_\n\n**Projects:**\n\n{cols_md}\n"));
  }
  match binding.table.schema.as_deref() {
    Some("<subq>") => Some(format!(
      "# `{token}`\n\n_Subquery alias_\n\nThe inner SELECT projects its columns through this alias. Hover the column on the right side of `{token}.col` to inspect each one."
    )),
    Some("<func>") => Some(format!(
      "# `{token}`\n\n_Function-call alias_\n\nBound to a set-returning function in the FROM clause."
    )),
    Some(other) if other.starts_with('<') => Some(format!("# `{token}`\n\n_{}_", other.trim_matches(|c| c == '<' || c == '>'))),
    _ => None,
  }
}

/// Return (open_after, close_before) byte offsets when `pos` is inside a
/// dollar-quoted `$$ ... $$` block. Used to extract the body for
/// re-parsing.
fn enclosing_dollar_body(source: &str, pos: usize) -> Option<(usize, usize)> {
  let bytes = source.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if bytes[i] == b'$' {
      // Read tag.
      let tag_start = i;
      let mut j = i + 1;
      while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if j < n && bytes[j] == b'$' {
        let tag = &source[tag_start..=j];
        let body_start = j + 1;
        // Find matching close.
        let mut k = body_start;
        while k + tag.len() <= n {
          if &source.as_bytes()[k..k + tag.len()] == tag.as_bytes() {
            if pos >= body_start && pos <= k {
              return Some((body_start, k));
            }
            i = k + tag.len();
            break;
          }
          k += 1;
        }
        if k + tag.len() > n {
          if pos >= body_start {
            return Some((body_start, n));
          }
          return None;
        }
        continue;
      }
    }
    i += 1;
  }
  None
}

/// Find the target table for the statement enclosing the cursor when
/// it's a CREATE INDEX / UPDATE / DELETE FROM / INSERT INTO -- and look
/// up `token` as a column of that single table.
fn scoped_column_in_text(source: &str, offset: TextSize, token: &str, catalog: &Catalog) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  // Walk back to the last `;` (or start) to bound the current statement.
  let stmt_start = source[..pos].rfind(';').map(|i| i + 1).unwrap_or(0);
  let slice = &source[stmt_start..pos];
  let upper = slice.to_ascii_uppercase();

  let table = if upper.contains("CREATE INDEX") || upper.contains("CREATE UNIQUE INDEX") {
    // After `ON `.
    let on_pos = find_kw(&upper, "ON")?;
    read_table_after(slice, on_pos + 2)
  } else if upper.starts_with("UPDATE") {
    let after = slice["UPDATE".len()..].trim_start();
    Some(read_ident(after))
  } else if upper.contains("DELETE FROM") {
    let p = upper.find("DELETE FROM")?;
    let after = slice[p + "DELETE FROM".len()..].trim_start();
    Some(read_ident(after))
  } else if upper.contains("INSERT INTO") {
    let p = upper.find("INSERT INTO")?;
    let after = slice[p + "INSERT INTO".len()..].trim_start();
    Some(read_ident(after))
  } else if upper.contains("ALTER TABLE") {
    let p = upper.find("ALTER TABLE")?;
    let after = slice[p + "ALTER TABLE".len()..].trim_start();
    Some(read_ident(after))
  } else {
    None
  }?;
  if table.is_empty() {
    return None;
  }

  if let Some(t) = catalog.find_table(None, &table)
    && let Some(c) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token))
  {
    return Some(render::column(t, c));
  }
  // Buffer-defined fallback (table being declared in same file).
  let parsed = dsl_parse::parse(source, dsl_parse::Dialect::Postgres);
  for stmt in &parsed.statements {
    if let dsl_parse::StatementKind::CreateTable(ct) = &stmt.kind
      && ct.table.name.eq_ignore_ascii_case(&table)
      && let Some(col) = ct.columns.iter().find(|c| c.name.eq_ignore_ascii_case(token))
    {
      return Some(render::column_decl(&ct.table.name, col));
    }
  }
  None
}

/// Whole-word search for `kw` in `upper`. Returns the byte offset.
fn find_kw(upper: &str, kw: &str) -> Option<usize> {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(kw) {
    let i = from + rel;
    let after = i + kw.len();
    let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
    let next_ok = after >= n || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
    if prev_ok && next_ok {
      return Some(i);
    }
    from = after;
  }
  None
}

fn read_table_after(src: &str, from: usize) -> Option<String> {
  let rest = src[from..].trim_start();
  Some(read_ident(rest))
}

/// Resolve the target table for a `NEW.<col>` / `OLD.<col>` hover.
/// Walks the buffer for the enclosing `CREATE [OR REPLACE | CONSTRAINT]
/// TRIGGER ... ON <table>` (closest match before the cursor). When the
/// cursor sits inside a function body, falls back to finding any
/// trigger declared to EXECUTE the surrounding function.
fn trigger_target_for_hover(source: &str, pos: usize, catalog: &Catalog) -> Option<String> {
  // First try: a CREATE TRIGGER clause in the buffer before the cursor.
  let upper = source.to_ascii_uppercase();
  let before = &upper[..pos.min(upper.len())];
  let idx = [
    "CREATE OR REPLACE TRIGGER",
    "CREATE CONSTRAINT TRIGGER",
    "CREATE TRIGGER",
  ]
  .iter()
  .filter_map(|kw| before.rfind(kw))
  .max();
  if let Some(idx) = idx
    && let Some(on_idx) = upper[idx..].find(" ON ")
  {
    let after_on = idx + on_idx + 4;
    let tail = &source[after_on..];
    let tok: String = tail
      .trim_start()
      .split(|c: char| c.is_whitespace() || c == '(' || c == ';' || c == ',')
      .find(|s| !s.is_empty())
      .unwrap_or("")
      .to_string();
    let tok = if tok.eq_ignore_ascii_case("ONLY") {
      tail
        .trim_start()
        .split_ascii_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string()
    } else {
      tok
    };
    let bare = tok.split('.').next_back().unwrap_or(&tok).trim_matches('"').to_string();
    if !bare.is_empty() {
      return Some(bare);
    }
  }
  // Fallback: cursor inside a CREATE FUNCTION body whose function is
  // wired as a trigger handler -- find that trigger and read its table.
  let fn_name = enclosing_create_function_name(source, pos)?;
  for f in &catalog.functions {
    let _ = f;
  }
  // Search the buffer's CREATE TRIGGER statements for one that
  // EXECUTEs <fn_name>.
  let fn_upper = fn_name.to_ascii_uppercase();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("CREATE") {
    let at = from + rel;
    let chunk = &upper[at..(at + 200).min(upper.len())];
    if chunk.contains("TRIGGER")
      && let Some(on_idx) = chunk.find(" ON ")
    {
      // Look for ` EXECUTE ` followed by FUNCTION/PROCEDURE <fn_name>
      let stmt_end = upper[at..].find(';').map(|p| at + p).unwrap_or(upper.len());
      let stmt_upper = &upper[at..stmt_end];
      if stmt_upper.contains(&fn_upper) {
        let after_on = at + on_idx + 4;
        let tail = &source[after_on..];
        let tok: String = tail
          .trim_start()
          .split(|c: char| c.is_whitespace() || c == '(' || c == ';' || c == ',')
          .find(|s| !s.is_empty())
          .unwrap_or("")
          .to_string();
        let bare = tok.split('.').next_back().unwrap_or(&tok).trim_matches('"').to_string();
        if !bare.is_empty() {
          return Some(bare);
        }
      }
    }
    from = at + 6;
  }
  None
}

/// Return the name of the CREATE FUNCTION whose body the cursor sits
/// in, or None when the cursor isn't inside any function definition.
fn enclosing_create_function_name(source: &str, pos: usize) -> Option<String> {
  let upper = source.to_ascii_uppercase();
  let before = &upper[..pos.min(upper.len())];
  let idx = ["CREATE OR REPLACE FUNCTION", "CREATE FUNCTION"]
    .iter()
    .filter_map(|kw| before.rfind(kw))
    .max()?;
  let after = source[idx..].split_once(char::is_whitespace)?.1;
  let after = after.trim_start();
  // Skip optional `OR REPLACE FUNCTION` head.
  let after = if after.to_ascii_uppercase().starts_with("OR REPLACE FUNCTION") {
    after.splitn(3, char::is_whitespace).nth(2).unwrap_or(after)
  } else if after.to_ascii_uppercase().starts_with("FUNCTION") {
    after.split_once(char::is_whitespace).map(|x| x.1).unwrap_or(after)
  } else {
    after
  };
  let tok: String = after
    .trim_start()
    .chars()
    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
    .collect();
  let bare = tok.split('.').next_back().unwrap_or(&tok).to_string();
  if bare.is_empty() { None } else { Some(bare) }
}

fn read_ident(s: &str) -> String {
  let name: String = s.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
  name.rsplit('.').next().unwrap_or(&name).to_string()
}

/// Render hover for the NEW / OLD trigger row aliases. If the buffer
/// contains a `CREATE TRIGGER ... ON <table>` clause, mention the table
/// the alias resolves against; otherwise show generic docs.
fn new_old_hover(name: &str, source: &str) -> String {
  let target = {
    let upper = source.to_ascii_uppercase();
    let idx = upper.find("CREATE TRIGGER");
    idx.and_then(|i| {
      let rest_upper = &upper[i..];
      rest_upper.find(" ON ").map(|p| {
        let after = &source[i + p + 4..];
        let n: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
        n
      })
    })
  };
  let desc = if name == "NEW" {
    "The row being inserted or updated. Inside an INSERT trigger \
         it holds the new tuple; inside UPDATE it holds the new column \
         values. NULL inside a DELETE / statement-level trigger."
  } else {
    "The row before UPDATE / DELETE. NULL inside an INSERT trigger \
         or a statement-level trigger."
  };
  let mut s = format!("# `{name}`\n*Trigger row variable*\n{desc}\n");
  if let Some(t) = target.filter(|s| !s.is_empty()) {
    s.push_str(&format!("\nResolves to a row of `{t}` (from the enclosing CREATE TRIGGER).\n"));
    s.push_str(&format!("```sql\n{name}.<column>\n```\n"));
  } else {
    s.push_str(
      "```sql\nNEW.<column>   -- access a field of the new row\nOLD.<column>   -- access a field of the old row\n```\n",
    );
  }
  s
}

/// Hover for a PL/pgSQL function parameter or DECLARE'd local. Uses
/// the same extraction logic as completion so the two stay in sync.
fn plpgsql_local_hover(source: &str, offset: TextSize, token: &str) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let locals = dsl_completion::plpgsql_locals::extract(source, pos);
  if let Some((_, ty)) = locals.params.iter().find(|(n, _)| n.eq_ignore_ascii_case(token)) {
    return Some(format!("# `{token}`\n*function parameter*\n```sql\n{token} {ty}\n```\n"));
  }
  if let Some((_, ty)) = locals.decls.iter().find(|(n, _)| n.eq_ignore_ascii_case(token)) {
    return Some(format!("# `{token}`\n*local (DECLARE)*\n```sql\n{token} {ty}\n```\n"));
  }
  None
}

/// Like [`db_function`] but biases the lookup by schema when one is
/// available -- for `schema.fn` calls the user wants the function
/// declared under THAT schema, not an unrelated overload elsewhere.
fn db_function_scoped(name: &str, schema: Option<&str>, catalog: &Catalog) -> Option<String> {
  let f = catalog.functions.iter().find(|f| {
    f.name.eq_ignore_ascii_case(name) && schema.is_none_or(|s| f.schema.eq_ignore_ascii_case(s))
  })?;
  let mut s = render::function_full(f);
  let (outgoing, incoming) = call_graph(&f.name, catalog);
  if !outgoing.is_empty() {
    s.push_str("\n**Calls**\n\n");
    for callee in outgoing.iter().take(20) {
      s.push_str(&format!("- `{callee}`\n"));
    }
  }
  if !incoming.is_empty() {
    s.push_str("\n**Called by**\n\n");
    for caller in incoming.iter().take(20) {
      s.push_str(&format!("- `{caller}`\n"));
    }
  }
  Some(s)
}

fn db_function(token: &str, catalog: &Catalog) -> Option<String> {
  let f = catalog.functions.iter().find(|f| f.name.eq_ignore_ascii_case(token))?;
  let mut s = render::function_full(f);
  // Append fan-in/fan-out from the workspace catalog. The function
  // body lives in f.comment (CREATE OR REPLACE FUNCTION ... $$ ...
  // $$); we scan it for other function calls and walk every other
  // catalog function's body for calls to this one.
  let (outgoing, incoming) = call_graph(&f.name, catalog);
  if !outgoing.is_empty() {
    s.push_str("\n**Calls**\n\n");
    for callee in outgoing.iter().take(20) {
      s.push_str(&format!("- `{callee}`\n"));
    }
    if outgoing.len() > 20 {
      s.push_str(&format!("- _… and {} more_\n", outgoing.len() - 20));
    }
  }
  if !incoming.is_empty() {
    s.push_str("\n**Called by**\n\n");
    for caller in incoming.iter().take(20) {
      s.push_str(&format!("- `{caller}`\n"));
    }
    if incoming.len() > 20 {
      s.push_str(&format!("- _… and {} more_\n", incoming.len() - 20));
    }
  }
  Some(s)
}

fn call_graph(name: &str, catalog: &Catalog) -> (Vec<String>, Vec<String>) {
  let mut outgoing = std::collections::BTreeSet::new();
  let mut incoming = std::collections::BTreeSet::new();
  for f in &catalog.functions {
    let Some(body) = f.comment.as_ref() else { continue };
    let upper_body = body.to_ascii_uppercase();
    if f.name.eq_ignore_ascii_case(name) {
      // Outgoing: identifiers in this function's body followed by `(`.
      for callee in extract_call_targets(body) {
        if !callee.eq_ignore_ascii_case(name) && !is_keyword(&callee) {
          outgoing.insert(callee);
        }
      }
    } else if upper_body.contains(&name.to_ascii_uppercase()) {
      // Incoming: if some other function's body mentions `name(`.
      let needle = format!("{}(", name.to_ascii_lowercase());
      if body.to_ascii_lowercase().contains(&needle) {
        incoming.insert(f.name.clone());
      }
    }
  }
  (outgoing.into_iter().collect(), incoming.into_iter().collect())
}

fn extract_call_targets(body: &str) -> std::collections::BTreeSet<String> {
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut out = std::collections::BTreeSet::new();
  let mut i = 0;
  while i < n {
    if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
      i += 1;
      continue;
    }
    let s = i;
    while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
      i += 1;
    }
    let mut k = i;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k < n && bytes[k] == b'(' {
      out.insert(body[s..i].to_string());
    }
  }
  out
}

fn is_keyword(s: &str) -> bool {
  matches!(
    s.to_ascii_uppercase().as_str(),
    "IF"
      | "WHILE"
      | "FOR"
      | "CASE"
      | "WHEN"
      | "RETURN"
      | "RAISE"
      | "SELECT"
      | "VALUES"
      | "INSERT"
      | "UPDATE"
      | "DELETE"
      | "PERFORM"
      | "EXECUTE"
      | "DECLARE"
      | "BEGIN"
      | "EXCEPTION"
      | "LOOP"
      | "EXIT"
      | "CONTINUE"
      | "ALL"
      | "ANY"
      | "OR"
      | "AND"
      | "NOT"
      | "IN"
      | "NULLIF"
      | "COALESCE"
      | "GREATEST"
      | "LEAST"
      | "CAST"
  )
}

/// Locate a CREATE FUNCTION / CREATE TRIGGER / CREATE INDEX defining
/// `token` in the buffer text and render it as a SQL code block. Lets
/// hover work before the DB schema has been refreshed.
fn buffer_object(source: &str, token: &str) -> Option<String> {
  for (heading, kind) in [
    ("CREATE OR REPLACE FUNCTION ", "Function"),
    ("CREATE FUNCTION ", "Function"),
    ("CREATE OR REPLACE PROCEDURE ", "Procedure"),
    ("CREATE PROCEDURE ", "Procedure"),
    ("CREATE TRIGGER ", "Trigger"),
    ("CREATE INDEX ", "Index"),
    ("CREATE UNIQUE INDEX ", "Unique index"),
    ("CREATE MATERIALIZED VIEW ", "Materialised view"),
    ("CREATE VIEW ", "View"),
    ("CREATE OR REPLACE VIEW ", "View"),
    ("CREATE SEQUENCE ", "Sequence"),
    ("CREATE TYPE ", "Type"),
    ("CREATE DOMAIN ", "Domain"),
    ("CREATE EXTENSION ", "Extension"),
    ("CREATE SCHEMA ", "Schema"),
    ("CREATE POLICY ", "Policy"),
  ] {
    if let Some(body) = find_def(source, heading, token) {
      return Some(format!(
        "# `{token}`\n_{kind} (current buffer)_\n\n```sql\n{}\n```\n",
        body.trim_end_matches(|c: char| c.is_whitespace())
      ));
    }
  }
  None
}

/// Walk `source` for `<heading> <token>` and, when found, return the
/// statement text up to the next top-level `;` (or EOF). Skips strings
/// and dollar-quoted bodies so the terminator is the real statement end.
fn find_def(source: &str, heading: &str, token: &str) -> Option<String> {
  let upper = source.to_ascii_uppercase();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(heading) {
    let start = from + rel;
    let after = start + heading.len();
    let rest = &source[after..];
    let trim_lead = rest.len() - rest.trim_start().len();
    let name_start = after + trim_lead;
    let name_end = name_start
      + source[name_start..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .map(|c| c.len_utf8())
        .sum::<usize>();
    let name = source[name_start..name_end].rsplit('.').next().unwrap_or("");
    if name.eq_ignore_ascii_case(token) {
      let end = stmt_end(source, name_end);
      return Some(source[start..end].to_string());
    }
    from = after;
  }
  None
}

/// Return the byte offset just past the next top-level `;`. Respects
/// `$$ ... $$` and single-quoted strings so we don't terminate mid-body.
fn stmt_end(source: &str, mut i: usize) -> usize {
  let bytes = source.as_bytes();
  let n = bytes.len();
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n {
          if bytes[i] == b'\'' {
            i += 1;
            break;
          }
          i += 1;
        }
      },
      b'$' => {
        // Dollar-quoted body. Read the tag.
        let tag_start = i;
        let mut j = i + 1;
        while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
          j += 1;
        }
        if j < n && bytes[j] == b'$' {
          let tag = &source[tag_start..=j];
          i = j + 1;
          // Walk until matching tag.
          while i + tag.len() <= n {
            if &source.as_bytes()[i..i + tag.len()] == tag.as_bytes() {
              i += tag.len();
              break;
            }
            i += 1;
          }
        } else {
          i += 1;
        }
      },
      b';' => return i + 1,
      _ => i += 1,
    }
  }
  n
}

/// Cursor on a literal / identifier inside a function-call argument
/// list. Walks back to the enclosing `(`, reads the function name,
/// looks up the signature in the knowledge base, and renders a card
/// pointing at the active parameter slot. Returns None when the
/// cursor is not inside a call OR the function is unknown.
/// True when the cursor sits on (or just past) an *identifier-shaped*
/// token -- one whose first character is `[A-Za-z_]`. Numeric-only
/// tokens (`1`, `42`) are NOT treated as identifiers since there's no
/// column / alias named after a bare number; that lets the function-
/// signature card still fire on numeric literal args. Used as a gate
/// so the function-arg card defers to column / alias / table hover
/// paths when the cursor is on a real name.
fn cursor_on_word_byte(source: &str, pos: usize) -> bool {
  let bytes = source.as_bytes();
  // Clamp: a caller may pass an offset past EOF (editor sometimes
  // sends an end-of-buffer pos that hasn't been re-validated).
  let pos = pos.min(bytes.len());
  let on = pos < bytes.len() && is_word_byte(bytes[pos]);
  let after = pos > 0 && is_word_byte(bytes[pos - 1]);
  if !(on || after) {
    return false;
  }
  // Walk back to the start of the word and inspect its first char.
  let mut start = pos.min(bytes.len());
  while start > 0 && is_word_byte(bytes[start - 1]) {
    start -= 1;
  }
  if start >= bytes.len() {
    return false;
  }
  let first = bytes[start];
  first.is_ascii_alphabetic() || first == b'_'
}

fn is_word_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn function_arg_at(source: &str, offset: TextSize) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  let n = bytes.len();
  if pos > n {
    return None;
  }
  // First, determine whether the cursor is currently inside a `'...'`
  // string by counting unescaped single-quotes from the start of the
  // statement up to `pos`. Odd count = inside a string -- walk back
  // past the opening quote before the depth/comma scan.
  let mut quotes = 0usize;
  let mut j = 0;
  while j < pos {
    if bytes[j] == b'\'' {
      quotes += 1;
    }
    j += 1;
  }
  let mut i = pos;
  if quotes % 2 == 1 {
    while i > 0 && bytes[i - 1] != b'\'' {
      i -= 1;
    }
    i = i.saturating_sub(1); // skip the opening `'`
  }
  // Now walk back through non-string content, counting commas at the
  // enclosing depth and looking for the unmatched `(`.
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut in_string: Option<u8> = None;
  while i > 0 {
    i -= 1;
    let b = bytes[i];
    if let Some(q) = in_string {
      if b == q {
        in_string = None;
      }
      continue;
    }
    match b {
      b')' => depth += 1,
      b'(' => {
        if depth == 0 {
          break;
        }
        depth -= 1;
      },
      b',' if depth == 0 => commas += 1,
      b'\'' => in_string = Some(b'\''),
      _ => {},
    }
  }
  if i == 0 && bytes.first() != Some(&b'(') {
    return None;
  }
  // i is at the `(`. Find identifier immediately before it.
  let mut end = i;
  while end > 0 && bytes[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  let mut start = end;
  while start > 0 {
    let c = bytes[start - 1] as char;
    if c.is_alphanumeric() || c == '_' || c == '.' {
      start -= 1;
    } else {
      break;
    }
  }
  if start == end {
    return None;
  }
  let name = source[start..end].to_string();
  let bare = name.rsplit('.').next().unwrap_or(&name).to_string();
  let entry = dsl_knowledge::functions().get(bare.to_ascii_lowercase().as_str())?;
  let sig = entry.signature?;
  // Parse the signature `fn(p1 t1, p2 t2) -> ret` to surface each
  // parameter name on its own line and mark the active one.
  let params = parse_signature_params(sig);
  let mut lines = Vec::new();
  lines.push(format!("-- function call: {bare}"));
  lines.push(format!("-- signature: {sig}"));
  lines.push(String::new());
  for (idx, p) in params.iter().enumerate() {
    let marker = if idx == commas { ">>" } else { "  " };
    lines.push(format!("{marker} {p}"));
  }
  Some(format!("```sql\n{}\n```\n", lines.join("\n")))
}

fn parse_signature_params(sig: &str) -> Vec<String> {
  let open = match sig.find('(') {
    Some(i) => i + 1,
    None => return Vec::new(),
  };
  let close = match sig.rfind(')') {
    Some(i) => i,
    None => sig.len(),
  };
  if close <= open {
    return Vec::new();
  }
  let body = &sig[open..close];
  let mut out = Vec::new();
  let bytes = body.as_bytes();
  let mut depth = 0i32;
  let mut start = 0usize;
  for (i, &b) in bytes.iter().enumerate() {
    match b {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(body[start..i].trim().to_string());
        start = i + 1;
      },
      _ => {},
    }
  }
  let tail = body[start..].trim();
  if !tail.is_empty() {
    out.push(tail.to_string());
  }
  out
}

/// `nextval('seq')`, `currval('seq')`, `setval('seq', 1)` -- when the
/// cursor sits inside the single-quoted sequence-name literal, return
/// a sequence card. Returns None when not in such a context.
fn sequence_ref_at(source: &str, offset: TextSize) -> Option<String> {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  let n = bytes.len();
  if pos >= n {
    return None;
  }
  // Walk back to the opening `'` of the string the cursor is in.
  let mut s = pos;
  while s > 0 && bytes[s - 1] != b'\'' {
    s -= 1;
  }
  if s == 0 || bytes[s - 1] != b'\'' {
    return None;
  }
  // Walk forward to the closing `'`.
  let mut e = pos;
  while e < n && bytes[e] != b'\'' {
    e += 1;
  }
  if e == n {
    return None;
  }
  let literal = &source[s..e];
  if literal.is_empty() {
    return None;
  }
  // The literal must look like an identifier (sequence name).
  if !literal.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
    return None;
  }
  // Walk back from the opening `'` over whitespace, expect `(`, then
  // walk back over whitespace, expect `NEXTVAL` / `CURRVAL` / `SETVAL`.
  let mut k = s.saturating_sub(1);
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  if k == 0 || bytes[k - 1] != b'(' {
    return None;
  }
  k -= 1;
  let kw_end = k;
  while k > 0 && (bytes[k - 1].is_ascii_alphabetic() || bytes[k - 1] == b'_') {
    k -= 1;
  }
  let kw = source[k..kw_end].to_ascii_uppercase();
  if !matches!(kw.as_str(), "NEXTVAL" | "CURRVAL" | "SETVAL" | "LASTVAL") {
    return None;
  }
  let qualified = literal;
  let bare = qualified.rsplit('.').next().unwrap_or(qualified);
  Some(format!(
    "```sql\n-- Sequence: {qualified}\n-- referenced by {kw_lc}('{qualified}')\n\nCREATE SEQUENCE {bare};\n```\n",
    kw_lc = kw.to_ascii_lowercase(),
  ))
}
