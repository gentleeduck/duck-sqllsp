//! SQL keyword table. Each entry has a doc, example, and canonical
//! Postgres docs URL. New keywords are added by appending to the `k!`
//! sequence below.

use crate::entry::{Entry, Kind, pg};
use std::collections::HashMap;

pub fn build() -> HashMap<&'static str, Entry> {
  let mut m = HashMap::new();
  macro_rules! k {
    ($label:expr, $doc:expr, $example:expr, $url:expr) => {
      m.insert(
        $label,
        Entry { label: $label, kind: Kind::Keyword, doc: $doc, signature: None, example: $example, url: $url },
      );
    };
  }

  // SELECT family
  k!(
    "SELECT",
    "Retrieve rows from one or more tables, views, or subqueries.",
    "SELECT id, name FROM users WHERE active = true;",
    pg("sql-select.html")
  );
  k!(
    "FROM",
    "List the tables, views, or subqueries the SELECT reads from.",
    "SELECT * FROM users u JOIN orders o ON o.user_id = u.id;",
    pg("sql-select.html#SQL-FROM")
  );
  k!(
    "WHERE",
    "Filter rows by a boolean expression. Applied before GROUP BY.",
    "SELECT * FROM users WHERE created_at > now() - interval '7 days';",
    pg("sql-select.html#SQL-WHERE")
  );
  k!(
    "GROUP BY",
    "Group rows that share values in the listed columns.",
    "SELECT user_id, count(*) FROM orders GROUP BY user_id;",
    pg("sql-select.html#SQL-GROUPBY")
  );
  k!(
    "HAVING",
    "Filter groups produced by GROUP BY. Like WHERE but on aggregates.",
    "SELECT user_id, count(*) FROM orders GROUP BY user_id HAVING count(*) > 5;",
    pg("sql-select.html#SQL-HAVING")
  );
  k!(
    "ORDER BY",
    "Sort the result set. ASC or DESC per key.",
    "SELECT * FROM users ORDER BY created_at DESC NULLS LAST;",
    pg("sql-select.html#SQL-ORDERBY")
  );
  k!(
    "LIMIT",
    "Restrict the number of returned rows.",
    "SELECT * FROM users ORDER BY id LIMIT 50 OFFSET 100;",
    pg("sql-select.html#SQL-LIMIT")
  );
  k!(
    "OFFSET",
    "Skip the first N rows. Combine with LIMIT for pagination.",
    "SELECT * FROM users ORDER BY id LIMIT 50 OFFSET 100;",
    pg("sql-select.html#SQL-LIMIT")
  );
  k!(
    "DISTINCT",
    "Eliminate duplicate rows.",
    "SELECT DISTINCT country FROM users;",
    pg("sql-select.html#SQL-DISTINCT")
  );
  k!(
    "UNION",
    "Combine result sets, removing duplicates. UNION ALL is faster.",
    "SELECT id FROM a UNION ALL SELECT id FROM b;",
    pg("queries-union.html")
  );
  k!(
    "WITH",
    "Common Table Expression (CTE). Define a named temporary result set.",
    "WITH recent AS (SELECT * FROM users WHERE active) SELECT count(*) FROM recent;",
    pg("queries-with.html")
  );

  // JOINs
  k!(
    "INNER JOIN",
    "Return rows where the ON predicate matches in both tables.",
    "SELECT u.name, o.total FROM users u INNER JOIN orders o ON o.user_id = u.id;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "LEFT JOIN",
    "Keep every row from the left table. Unmatched right side is NULL.",
    "SELECT u.id, o.id FROM users u LEFT JOIN orders o ON o.user_id = u.id;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "RIGHT JOIN",
    "Keep every row from the right table. Unmatched left side is NULL.",
    "SELECT u.id, o.id FROM users u RIGHT JOIN orders o ON o.user_id = u.id;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "FULL OUTER JOIN",
    "Keep every row from both sides. Unmatched columns become NULL.",
    "SELECT * FROM a FULL OUTER JOIN b ON a.id = b.a_id;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "CROSS JOIN",
    "Cartesian product. Every row of left paired with every row of right.",
    "SELECT * FROM colors CROSS JOIN sizes;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "LATERAL",
    "Allow a subquery in FROM to reference columns from earlier FROM items.",
    "SELECT u.id FROM users u, LATERAL (SELECT title FROM posts WHERE user_id = u.id LIMIT 1) p;",
    pg("queries-table-expressions.html#QUERIES-LATERAL")
  );
  k!(
    "ON",
    "JOIN predicate. Boolean expression evaluated per row pair.",
    "JOIN orders o ON o.user_id = u.id",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "AS",
    "Alias a column, expression, table, or CTE.",
    "SELECT count(*) AS n FROM users AS u;",
    pg("sql-select.html")
  );

  // DML
  k!(
    "INSERT INTO",
    "Insert new rows. Combine with RETURNING to get generated columns back.",
    "INSERT INTO users (name) VALUES ('alice') RETURNING id;",
    pg("sql-insert.html")
  );
  k!(
    "VALUES",
    "List of literal row tuples to insert.",
    "INSERT INTO users (name) VALUES ('a'), ('b');",
    pg("sql-insert.html")
  );
  k!(
    "UPDATE",
    "Modify existing rows. Skipping WHERE updates every row.",
    "UPDATE users SET active = false WHERE last_seen < now() - interval '1 year';",
    pg("sql-update.html")
  );
  k!(
    "SET",
    "List column = expression pairs for UPDATE.",
    "UPDATE users SET name = 'bob' WHERE id = $1;",
    pg("sql-update.html")
  );
  k!(
    "DELETE FROM",
    "Remove rows. RETURNING shows what was deleted.",
    "DELETE FROM sessions WHERE expires_at < now();",
    pg("sql-delete.html")
  );
  k!(
    "RETURNING",
    "Get column values from INSERT / UPDATE / DELETE.",
    "INSERT INTO users (name) VALUES ('alice') RETURNING id, created_at;",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );
  k!(
    "ON CONFLICT",
    "Upsert. Specify what to do when a unique constraint would be violated.",
    "INSERT INTO users (email) VALUES ('a') ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name;",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );

  // DDL
  k!(
    "CREATE TABLE",
    "Create a new table. Add IF NOT EXISTS to make idempotent.",
    "CREATE TABLE IF NOT EXISTS users (id UUID PRIMARY KEY DEFAULT gen_random_uuid());",
    pg("sql-createtable.html")
  );
  k!(
    "CREATE INDEX",
    "Create a B-tree index by default.",
    "CREATE INDEX idx_users_email ON users (email);",
    pg("sql-createindex.html")
  );
  k!(
    "CREATE VIEW",
    "Define a virtual table backed by a SELECT.",
    "CREATE VIEW active_users AS SELECT * FROM users WHERE deleted_at IS NULL;",
    pg("sql-createview.html")
  );
  k!(
    "ALTER TABLE",
    "Modify a table: add/drop columns, rename, change types.",
    "ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user';",
    pg("sql-altertable.html")
  );
  k!(
    "DROP TABLE",
    "Remove a table. CASCADE removes dependent objects.",
    "DROP TABLE IF EXISTS users CASCADE;",
    pg("sql-droptable.html")
  );

  // Constraints
  k!(
    "PRIMARY KEY",
    "Implicit UNIQUE + NOT NULL.",
    "id UUID PRIMARY KEY DEFAULT gen_random_uuid()",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-PRIMARY-KEYS")
  );
  k!(
    "FOREIGN KEY",
    "Constrain a column to match the PRIMARY KEY of another table.",
    "CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );
  k!(
    "REFERENCES",
    "Short form of FOREIGN KEY when used inline on a column.",
    "user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );
  k!(
    "UNIQUE",
    "No two rows share the same value in the listed column(s).",
    "email TEXT UNIQUE NOT NULL",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-UNIQUE-CONSTRAINTS")
  );
  k!(
    "CHECK",
    "Row-level constraint expressed as a boolean expression.",
    "CONSTRAINT ch_email CHECK (lower(email) = email)",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-CHECK-CONSTRAINTS")
  );
  k!("NOT NULL", "Column may never be NULL.", "name TEXT NOT NULL", pg("ddl-constraints.html#id-1.5.4.6.6"));
  k!(
    "DEFAULT",
    "Value inserted when the column is omitted.",
    "created_at TIMESTAMPTZ NOT NULL DEFAULT now()",
    pg("ddl-default.html")
  );
  k!(
    "CASCADE",
    "ON DELETE / ON UPDATE action: also delete / update referencing rows.",
    "REFERENCES users(id) ON DELETE CASCADE",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );

  // Logical / predicates
  k!("AND", "Logical AND. Short-circuits.", "WHERE active AND email IS NOT NULL", pg("functions-logical.html"));
  k!("OR", "Logical OR. Short-circuits.", "WHERE role = 'admin' OR is_staff", pg("functions-logical.html"));
  k!("NOT", "Logical NOT. Negates a boolean.", "WHERE NOT (deleted_at IS NULL)", pg("functions-logical.html"));
  k!("IS NULL", "True when expression is NULL.", "WHERE deleted_at IS NULL", pg("functions-comparison.html"));
  k!("IS NOT NULL", "True when expression is not NULL.", "WHERE email IS NOT NULL", pg("functions-comparison.html"));
  k!(
    "IN",
    "True if value equals any in the list / subquery result.",
    "WHERE role IN ('admin', 'staff')",
    pg("functions-comparisons.html")
  );
  k!(
    "EXISTS",
    "True if subquery returns at least one row.",
    "WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = u.id)",
    pg("functions-subquery.html")
  );
  k!(
    "BETWEEN",
    "Shorthand for value >= low AND value <= high.",
    "WHERE age BETWEEN 18 AND 65",
    pg("functions-comparison.html")
  );
  k!(
    "LIKE",
    "Case-sensitive pattern match. % matches any string, _ matches one char.",
    "WHERE email LIKE '%@example.com'",
    pg("functions-matching.html#FUNCTIONS-LIKE")
  );
  k!(
    "ILIKE",
    "Case-insensitive LIKE (Postgres extension).",
    "WHERE name ILIKE '%john%'",
    pg("functions-matching.html#FUNCTIONS-LIKE")
  );
  k!(
    "CASE",
    "Conditional expression. SQL's if/else.",
    "SELECT CASE WHEN role = 'admin' THEN 1 ELSE 0 END FROM users;",
    pg("functions-conditional.html#FUNCTIONS-CASE")
  );

  // Windows
  k!(
    "OVER",
    "Window specification for a window function.",
    "row_number() OVER (PARTITION BY user_id ORDER BY created_at DESC)",
    pg("tutorial-window.html")
  );
  k!(
    "PARTITION BY",
    "Split rows into groups within a window function.",
    "rank() OVER (PARTITION BY country ORDER BY score DESC)",
    pg("tutorial-window.html")
  );

  // Transactions
  k!("BEGIN", "Start an explicit transaction.", "BEGIN; ... COMMIT;", pg("sql-begin.html"));
  k!("COMMIT", "Persist the current transaction.", "COMMIT;", pg("sql-commit.html"));
  k!("ROLLBACK", "Discard the current transaction.", "ROLLBACK;", pg("sql-rollback.html"));

  // EXPLAIN
  k!(
    "EXPLAIN",
    "Show the planner's execution plan without running.",
    "EXPLAIN SELECT * FROM users WHERE id = $1;",
    pg("sql-explain.html")
  );
  k!(
    "EXPLAIN ANALYZE",
    "Run the query and show the actual plan with timings.",
    "EXPLAIN ANALYZE SELECT * FROM users WHERE id = $1;",
    pg("sql-explain.html")
  );

  // -----------------------------------------------------------------------
  // Standalone single-word forms.
  //
  // We already cover multi-word keywords above (INNER JOIN, ON CONFLICT,
  // CREATE TABLE, ...). When the user hovers on just one of the words,
  // we want a docs page too -- otherwise common tokens like JOIN, INTO,
  // BY, USING render no hover at all.
  // -----------------------------------------------------------------------

  // JOIN and modifiers
  k!(
    "JOIN",
    "Combine rows from two tables. Prefix with INNER / LEFT / RIGHT / FULL / CROSS to control behaviour.",
    "SELECT u.name, o.total FROM users u JOIN orders o ON o.user_id = u.id;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "INNER",
    "Inner join modifier. INNER JOIN returns only matching rows.",
    "INNER JOIN orders o ON o.user_id = u.id",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "LEFT",
    "Left join modifier. LEFT JOIN keeps every row from the left side.",
    "LEFT JOIN orders o ON o.user_id = u.id",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "RIGHT",
    "Right join modifier. RIGHT JOIN keeps every row from the right side.",
    "RIGHT JOIN orders o ON o.user_id = u.id",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "FULL",
    "Full join modifier. FULL OUTER JOIN keeps every row from both sides.",
    "FULL OUTER JOIN b ON a.id = b.a_id",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "OUTER",
    "Outer-join modifier. LEFT / RIGHT / FULL OUTER JOIN keep unmatched rows.",
    "LEFT OUTER JOIN orders o ON o.user_id = u.id",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "CROSS",
    "CROSS JOIN produces the Cartesian product of two tables.",
    "SELECT * FROM colors CROSS JOIN sizes;",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );
  k!(
    "USING",
    "JOIN shorthand when the join columns share a name. The joined column appears once.",
    "SELECT * FROM users JOIN orders USING (user_id);",
    pg("queries-table-expressions.html#QUERIES-JOIN")
  );

  // Sub-statement keywords
  k!(
    "INTO",
    "Destination keyword in INSERT INTO and SELECT INTO.",
    "INSERT INTO users (name) VALUES ('a')",
    pg("sql-insert.html")
  );
  k!(
    "BY",
    "Modifier used after GROUP, ORDER, PARTITION. Specifies the keys.",
    "ORDER BY created_at DESC",
    pg("sql-select.html#SQL-ORDERBY")
  );
  k!(
    "IS",
    "Predicate for IS NULL / IS NOT NULL / IS TRUE / IS DISTINCT FROM.",
    "WHERE deleted_at IS NULL",
    pg("functions-comparison.html")
  );
  k!(
    "ALL",
    "Quantifier. UNION ALL keeps duplicates; ALL with comparisons matches every row of subquery.",
    "SELECT id FROM a UNION ALL SELECT id FROM b;",
    pg("queries-union.html")
  );
  k!(
    "ANY",
    "Quantifier. x = ANY (SELECT ...) matches at least one row.",
    "WHERE id = ANY (SELECT user_id FROM admins)",
    pg("functions-comparisons.html")
  );
  k!("SOME", "Synonym for ANY.", "WHERE id = SOME (SELECT user_id FROM admins)", pg("functions-comparisons.html"));
  k!(
    "DO",
    "Action verb after ON CONFLICT. DO UPDATE or DO NOTHING.",
    "ON CONFLICT (id) DO NOTHING",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );
  k!(
    "DO UPDATE",
    "Upsert action: merge values. Use EXCLUDED.<col> for the failed insert row.",
    "ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );
  k!(
    "DO NOTHING",
    "Upsert action: silently skip the row.",
    "ON CONFLICT DO NOTHING",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );
  k!(
    "EXCLUDED",
    "Synthetic alias in DO UPDATE referencing the row that would have been inserted.",
    "ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );

  // DDL atoms
  k!(
    "CREATE",
    "Define a new database object: table, index, view, sequence, schema, function.",
    "CREATE TABLE ...",
    pg("sql-createtable.html")
  );
  k!(
    "TABLE",
    "The TABLE keyword. Used by CREATE / ALTER / DROP / TRUNCATE.",
    "CREATE TABLE users (...)",
    pg("sql-createtable.html")
  );
  k!(
    "INDEX",
    "The INDEX keyword. Used by CREATE / DROP / REINDEX.",
    "CREATE INDEX idx_users_email ON users (email);",
    pg("sql-createindex.html")
  );
  k!(
    "VIEW",
    "The VIEW keyword. Used by CREATE / ALTER / DROP.",
    "CREATE VIEW active_users AS SELECT * FROM users;",
    pg("sql-createview.html")
  );
  k!("SCHEMA", "Namespace for tables, views, functions.", "CREATE SCHEMA app;", pg("sql-createschema.html"));
  k!(
    "SEQUENCE",
    "Number generator for serial columns.",
    "CREATE SEQUENCE users_id_seq START 1000;",
    pg("sql-createsequence.html")
  );
  k!(
    "ALTER",
    "Modify an existing database object.",
    "ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user';",
    pg("sql-altertable.html")
  );
  k!(
    "DROP",
    "Remove a database object. Add IF EXISTS to suppress missing-object errors.",
    "DROP TABLE IF EXISTS users CASCADE;",
    pg("sql-droptable.html")
  );
  k!(
    "RENAME",
    "Rename a table or column inside ALTER TABLE.",
    "ALTER TABLE users RENAME COLUMN nick TO nickname;",
    pg("sql-altertable.html")
  );
  k!(
    "ADD",
    "Add a column or constraint inside ALTER TABLE.",
    "ALTER TABLE users ADD COLUMN role TEXT;",
    pg("sql-altertable.html")
  );
  k!(
    "COLUMN",
    "The COLUMN keyword. Used inside ALTER TABLE ADD/DROP/RENAME COLUMN.",
    "ALTER TABLE users DROP COLUMN old_field;",
    pg("sql-altertable.html")
  );
  k!(
    "TRUNCATE",
    "Remove every row from a table fast (no row-by-row scan).",
    "TRUNCATE TABLE sessions;",
    pg("sql-truncate.html")
  );
  k!("IF", "Modifier in IF NOT EXISTS / IF EXISTS.", "CREATE TABLE IF NOT EXISTS ...", pg("sql-createtable.html"));
  k!(
    "IF NOT EXISTS",
    "Idempotent modifier on CREATE statements.",
    "CREATE TABLE IF NOT EXISTS users (...);",
    pg("sql-createtable.html")
  );
  k!(
    "IF EXISTS",
    "Idempotent modifier on DROP / ALTER statements.",
    "DROP TABLE IF EXISTS users;",
    pg("sql-droptable.html")
  );

  // Constraint atoms
  k!(
    "CONSTRAINT",
    "Name a table / column constraint. Required for FOREIGN KEY clauses.",
    "CONSTRAINT pk_users PRIMARY KEY (id)",
    pg("ddl-constraints.html")
  );
  k!("KEY", "Constraint qualifier. PRIMARY KEY / FOREIGN KEY.", "PRIMARY KEY (id)", pg("ddl-constraints.html"));
  k!(
    "PRIMARY",
    "Identifies the row. Implies UNIQUE + NOT NULL.",
    "id UUID PRIMARY KEY",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-PRIMARY-KEYS")
  );
  k!(
    "FOREIGN",
    "FOREIGN KEY references another table's primary key.",
    "FOREIGN KEY (user_id) REFERENCES users (id)",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );
  k!(
    "RESTRICT",
    "ON DELETE/UPDATE action: forbid the operation if referencing rows exist.",
    "REFERENCES users(id) ON DELETE RESTRICT",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );
  k!(
    "SET NULL",
    "ON DELETE action: NULL the referencing column.",
    "REFERENCES users(id) ON DELETE SET NULL",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );
  k!(
    "SET DEFAULT",
    "ON DELETE action: reset the referencing column to its DEFAULT.",
    "REFERENCES users(id) ON DELETE SET DEFAULT",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );
  k!(
    "NO ACTION",
    "ON DELETE/UPDATE action: defer constraint check; default behaviour.",
    "REFERENCES users(id) ON DELETE NO ACTION",
    pg("ddl-constraints.html#DDL-CONSTRAINTS-FK")
  );

  // Transactions and set operations
  k!(
    "SAVEPOINT",
    "Marker inside a transaction. ROLLBACK TO SAVEPOINT rewinds to it.",
    "SAVEPOINT before_update;",
    pg("sql-savepoint.html")
  );
  k!(
    "TRANSACTION",
    "Synonym block for BEGIN / COMMIT / ROLLBACK.",
    "BEGIN TRANSACTION; ... COMMIT;",
    pg("sql-begin.html")
  );
  k!(
    "INTERSECT",
    "Set operator: rows present in both result sets, deduplicated.",
    "SELECT user_id FROM orders INTERSECT SELECT id FROM users;",
    pg("queries-union.html")
  );
  k!(
    "EXCEPT",
    "Set operator: rows in the first result set that aren't in the second.",
    "SELECT id FROM users EXCEPT SELECT user_id FROM orders;",
    pg("queries-union.html")
  );
  k!(
    "RECURSIVE",
    "Make a WITH clause recursive. Used for tree / graph traversal.",
    "WITH RECURSIVE tree AS (...)",
    pg("queries-with.html#QUERIES-WITH-RECURSIVE")
  );
  k!("ANALYZE", "Refresh planner statistics. Distinct from EXPLAIN ANALYZE.", "ANALYZE users;", pg("sql-analyze.html"));
  k!("VACUUM", "Reclaim storage. VACUUM FULL rewrites the table.", "VACUUM ANALYZE users;", pg("sql-vacuum.html"));

  // Sort / null ordering atoms (already have NULLS FIRST/LAST as multi-word)
  k!(
    "NULLS",
    "Sort modifier. NULLS FIRST / NULLS LAST controls where NULLs land.",
    "ORDER BY deleted_at NULLS LAST",
    pg("sql-select.html#SQL-ORDERBY")
  );
  k!("FIRST", "Modifier for NULLS FIRST in ORDER BY.", "ORDER BY x NULLS FIRST", pg("sql-select.html#SQL-ORDERBY"));
  k!("LAST", "Modifier for NULLS LAST in ORDER BY.", "ORDER BY x NULLS LAST", pg("sql-select.html#SQL-ORDERBY"));

  // Wildcard
  k!(
    "*",
    "Star: select all columns of the row source. Avoid in production code; prefer explicit columns.",
    "SELECT * FROM users LIMIT 10;",
    pg("sql-select.html#SQL-SELECT-LIST")
  );

  // Bare NULL (distinct from IS NULL / NOT NULL multi-word entries).
  k!(
    "NULL",
    "SQL NULL: missing / unknown value. Use IS NULL / IS NOT NULL to test; `=` against NULL always yields NULL.",
    "WHERE deleted_at IS NULL",
    pg("functions-comparison.html")
  );

  // -----------------------------------------------------------------------
  // Bare forms of statement starters. Cursor on the first word should
  // still hover even when the multi-word form is covered.
  // -----------------------------------------------------------------------
  k!(
    "INSERT",
    "Insert new rows. Combine with RETURNING to get generated columns back.",
    "INSERT INTO users (name) VALUES ('alice');",
    pg("sql-insert.html")
  );
  k!(
    "DELETE",
    "Remove rows. RETURNING shows what was deleted.",
    "DELETE FROM users WHERE id = $1;",
    pg("sql-delete.html")
  );
  k!(
    "ORDER",
    "Sort modifier. ORDER BY <col> [ASC|DESC].",
    "ORDER BY created_at DESC",
    pg("sql-select.html#SQL-ORDERBY")
  );
  k!("GROUP", "Group rows. GROUP BY <cols>.", "GROUP BY user_id", pg("sql-select.html#SQL-GROUPBY"));

  // -----------------------------------------------------------------------
  // PL/pgSQL function / trigger definition keywords.
  // -----------------------------------------------------------------------
  k!(
    "FUNCTION",
    "Stored function. CREATE FUNCTION ... LANGUAGE plpgsql / sql.",
    "CREATE OR REPLACE FUNCTION fn() RETURNS void AS $$ ... $$ LANGUAGE plpgsql;",
    pg("sql-createfunction.html")
  );
  k!(
    "PROCEDURE",
    "Stored procedure (no return value). Call with CALL.",
    "CREATE PROCEDURE p() LANGUAGE plpgsql AS $$ ... $$;",
    pg("sql-createprocedure.html")
  );
  k!(
    "TRIGGER",
    "Stored callback that fires on INSERT / UPDATE / DELETE / TRUNCATE.",
    "CREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION fn();",
    pg("sql-createtrigger.html")
  );
  k!(
    "RETURNS",
    "Return-type clause in CREATE FUNCTION.",
    "CREATE FUNCTION fn() RETURNS uuid",
    pg("sql-createfunction.html")
  );
  k!("RETURN", "Return a value from a PL/pgSQL function.", "RETURN result;", pg("plpgsql-control-structures.html"));
  k!(
    "LANGUAGE",
    "Procedural language for CREATE FUNCTION. Common: plpgsql, sql.",
    "$$ ... $$ LANGUAGE plpgsql",
    pg("sql-createfunction.html")
  );
  k!(
    "PLPGSQL",
    "Default Postgres procedural language. Block-structured with DECLARE / BEGIN / END.",
    "LANGUAGE plpgsql",
    pg("plpgsql.html")
  );
  k!(
    "STABLE",
    "Volatility marker: same arguments return the same result within a single statement.",
    "CREATE FUNCTION fn() RETURNS uuid STABLE AS $$ ... $$;",
    pg("xfunc-volatility.html")
  );
  k!(
    "VOLATILE",
    "Volatility marker (default): result can change at any time.",
    "CREATE FUNCTION rnd() RETURNS double VOLATILE AS $$ ... $$;",
    pg("xfunc-volatility.html")
  );
  k!(
    "IMMUTABLE",
    "Volatility marker: pure, no DB or side-effect access.",
    "CREATE FUNCTION square(int) RETURNS int IMMUTABLE AS $$ ... $$;",
    pg("xfunc-volatility.html")
  );
  k!(
    "STORED",
    "STORED generated columns: value persisted in the table on each write.",
    "amount NUMERIC GENERATED ALWAYS AS (qty * price) STORED",
    pg("ddl-generated-columns.html")
  );
  k!(
    "GENERATED",
    "Generated column. ALWAYS / BY DEFAULT.",
    "id BIGINT GENERATED ALWAYS AS IDENTITY",
    pg("ddl-generated-columns.html")
  );
  k!(
    "ALWAYS",
    "Generated-column qualifier: always evaluate the expression.",
    "GENERATED ALWAYS AS IDENTITY",
    pg("ddl-generated-columns.html")
  );
  k!(
    "IDENTITY",
    "Auto-assign primary key from a sequence. Preferred over SERIAL.",
    "id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY",
    pg("ddl-identity-columns.html")
  );
  k!(
    "DECLARE",
    "Variable declarations at the top of a PL/pgSQL block.",
    "DECLARE n INT := 0; BEGIN ... END;",
    pg("plpgsql-declarations.html")
  );
  k!(
    "LOOP",
    "Unconditional loop. EXIT or RETURN to leave.",
    "LOOP n := n + 1; EXIT WHEN n > 10; END LOOP;",
    pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS")
  );
  k!(
    "FOR",
    "PL/pgSQL loop. FOR i IN 1..10 / FOR r IN SELECT ...",
    "FOR r IN SELECT * FROM users LOOP RAISE NOTICE '%', r.id; END LOOP;",
    pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS-FOR")
  );
  k!(
    "WHILE",
    "Loop while a condition is true.",
    "WHILE n < 10 LOOP n := n + 1; END LOOP;",
    pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS")
  );
  k!(
    "ELSIF",
    "PL/pgSQL else-if branch.",
    "IF a THEN ... ELSIF b THEN ... END IF;",
    pg("plpgsql-control-structures.html#PLPGSQL-CONDITIONALS")
  );
  k!(
    "RAISE",
    "Throw a PL/pgSQL notice / exception.",
    "RAISE NOTICE 'value is %', v;",
    pg("plpgsql-errors-and-messages.html")
  );
  k!(
    "NOTICE",
    "RAISE level: log message; query continues.",
    "RAISE NOTICE 'value is %', v;",
    pg("plpgsql-errors-and-messages.html")
  );
  k!("WARNING", "RAISE level above NOTICE.", "RAISE WARNING 'careful';", pg("plpgsql-errors-and-messages.html"));
  k!(
    "EXCEPTION",
    "RAISE EXCEPTION aborts the transaction. Also block-level handler.",
    "RAISE EXCEPTION 'bad input: %', x;",
    pg("plpgsql-errors-and-messages.html")
  );
  k!(
    "NEW",
    "Trigger row variable: the row being inserted / updated.",
    "BEGIN NEW.updated_at := now(); RETURN NEW; END;",
    pg("plpgsql-trigger.html")
  );
  k!(
    "OLD",
    "Trigger row variable: the row before UPDATE / DELETE.",
    "IF OLD.role <> NEW.role THEN RAISE NOTICE 'role changed'; END IF;",
    pg("plpgsql-trigger.html")
  );
  k!(
    "BEFORE",
    "Trigger timing: fire before the row change.",
    "CREATE TRIGGER t BEFORE INSERT ON users ...",
    pg("sql-createtrigger.html")
  );
  k!(
    "AFTER",
    "Trigger timing: fire after the row change.",
    "CREATE TRIGGER t AFTER UPDATE ON users ...",
    pg("sql-createtrigger.html")
  );
  k!(
    "INSTEAD",
    "Trigger timing for views: replace the operation entirely.",
    "CREATE TRIGGER t INSTEAD OF INSERT ON v ...",
    pg("sql-createtrigger.html")
  );
  k!(
    "EACH",
    "Trigger row-vs-statement modifier. FOR EACH ROW / FOR EACH STATEMENT.",
    "FOR EACH ROW EXECUTE FUNCTION fn();",
    pg("sql-createtrigger.html")
  );
  k!("ROW", "Trigger granularity: per row.", "FOR EACH ROW", pg("sql-createtrigger.html"));
  k!("STATEMENT", "Trigger granularity: per statement (default).", "FOR EACH STATEMENT", pg("sql-createtrigger.html"));
  k!("EXECUTE", "Run a function or dynamic SQL.", "EXECUTE FUNCTION audit_change();", pg("sql-createtrigger.html"));

  // -----------------------------------------------------------------------
  // Privileges, admin, set commands
  // -----------------------------------------------------------------------
  k!("GRANT", "Grant privileges to a role.", "GRANT SELECT ON users TO readonly;", pg("sql-grant.html"));
  k!("REVOKE", "Revoke privileges from a role.", "REVOKE SELECT ON users FROM readonly;", pg("sql-revoke.html"));
  k!("ROLE", "Database role (a user or group).", "CREATE ROLE readonly LOGIN PASSWORD '...';", pg("user-manag.html"));
  k!("USER", "Synonym for ROLE with LOGIN.", "CREATE USER alice PASSWORD '...';", pg("sql-createuser.html"));
  k!(
    "OWNER",
    "Object owner. ALTER ... OWNER TO sets it.",
    "ALTER TABLE users OWNER TO alice;",
    pg("sql-alterowner.html")
  );
  k!("PASSWORD", "ROLE password attribute.", "ALTER ROLE alice PASSWORD '...';", pg("sql-alterrole.html"));
  k!("SHOW", "Display a run-time parameter.", "SHOW search_path;", pg("sql-show.html"));
  k!(
    "COMMENT",
    "Attach a comment to a database object.",
    "COMMENT ON TABLE users IS 'app users';",
    pg("sql-comment.html")
  );
  k!("COPY", "Bulk-load / bulk-export a table.", "COPY users FROM '/tmp/users.csv' CSV HEADER;", pg("sql-copy.html"));
  k!(
    "MERGE",
    "Conditional upsert. Postgres 15+.",
    "MERGE INTO target t USING source s ON t.id = s.id WHEN MATCHED THEN UPDATE SET ...;",
    pg("sql-merge.html")
  );
  k!(
    "REFRESH",
    "Refresh a materialised view.",
    "REFRESH MATERIALIZED VIEW user_counts;",
    pg("sql-refreshmaterializedview.html")
  );
  k!("REINDEX", "Rebuild an index.", "REINDEX TABLE users;", pg("sql-reindex.html"));
  k!(
    "TO",
    "Target keyword in GRANT / REVOKE / ALTER ... OWNER TO / RENAME ... TO / SET ... TO.",
    "ALTER TABLE users OWNER TO alice;",
    pg("sql-grant.html")
  );
  k!("CALL", "Invoke a stored PROCEDURE.", "CALL refresh_counts();", pg("sql-call.html"));
  k!(
    "REPLACE",
    "Used in CREATE OR REPLACE to redefine functions / views without dropping them.",
    "CREATE OR REPLACE FUNCTION fn() ...",
    pg("sql-createfunction.html")
  );
  k!(
    "OF",
    "Modifier in INSTEAD OF / TYPE OF / FOR EACH ROW OF.",
    "CREATE TRIGGER t INSTEAD OF INSERT ON v ...",
    pg("sql-createtrigger.html")
  );

  // Casts (INT lives in the types table -- it is a data type, not a keyword).
  k!(
    "CAST",
    "Explicit type conversion. SQL-standard form of the `::` operator.",
    "CAST(id AS text)",
    pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS")
  );

  // Conflict / matched
  k!(
    "CONFLICT",
    "Used in ON CONFLICT clauses for UPSERT.",
    "INSERT INTO t ... ON CONFLICT (id) DO UPDATE ...",
    pg("sql-insert.html#SQL-ON-CONFLICT")
  );
  k!(
    "MATCHED",
    "Used in MERGE WHEN MATCHED / NOT MATCHED branches.",
    "WHEN MATCHED THEN UPDATE SET ...",
    pg("sql-merge.html")
  );
  k!("NOTHING", "ON CONFLICT DO NOTHING action.", "ON CONFLICT DO NOTHING", pg("sql-insert.html#SQL-ON-CONFLICT"));

  // CASE branch atoms
  k!(
    "WHEN",
    "CASE branch test. Pairs with THEN.",
    "CASE WHEN x > 0 THEN 'pos' WHEN x < 0 THEN 'neg' ELSE 'zero' END",
    pg("functions-conditional.html#FUNCTIONS-CASE")
  );
  k!(
    "THEN",
    "CASE branch result. Follows WHEN.",
    "CASE WHEN x > 0 THEN 'positive' END",
    pg("functions-conditional.html#FUNCTIONS-CASE")
  );
  k!(
    "ELSE",
    "CASE fallback branch.",
    "CASE WHEN x > 0 THEN 1 ELSE 0 END",
    pg("functions-conditional.html#FUNCTIONS-CASE")
  );
  k!(
    "END",
    "CASE terminator. Also closes a PL/pgSQL block (END LOOP / END IF).",
    "CASE WHEN x > 0 THEN 1 ELSE 0 END",
    pg("functions-conditional.html#FUNCTIONS-CASE")
  );

  // Sort order atoms
  k!("ASC", "Ascending order. The default for ORDER BY.", "ORDER BY created_at ASC", pg("sql-select.html#SQL-ORDERBY"));
  k!("DESC", "Descending order.", "ORDER BY created_at DESC", pg("sql-select.html#SQL-ORDERBY"));

  // Booleans
  k!("TRUE", "Boolean true.", "WHERE is_active = TRUE", pg("datatype-boolean.html"));
  k!("FALSE", "Boolean false.", "WHERE deleted = FALSE", pg("datatype-boolean.html"));

  // Array type and common literals
  k!(
    "ARRAY",
    "Array type / constructor. ARRAY[1,2,3] builds an int[] literal.",
    "ids INT[] DEFAULT ARRAY[1,2,3]",
    pg("arrays.html")
  );

  // -----------------------------------------------------------------------
  // Predicate / advanced grouping / windowing.
  // -----------------------------------------------------------------------
  k!(
    "COLLATE",
    "Apply a collation to a string column or expression.",
    "name TEXT COLLATE \"en_US.utf8\"",
    pg("collation.html")
  );
  k!(
    "FILTER",
    "Aggregate filter: count(...) FILTER (WHERE predicate).",
    "SELECT count(*) FILTER (WHERE active) FROM users;",
    pg("sql-expressions.html#SYNTAX-AGGREGATES")
  );
  k!(
    "WITHIN",
    "Used by ordered-set aggregates: agg() WITHIN GROUP (ORDER BY ...).",
    "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY age) FROM users;",
    pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE")
  );
  k!(
    "GROUPING SETS",
    "Compute multiple groupings in one query.",
    "GROUP BY GROUPING SETS ((a, b), (a), ())",
    pg("queries-table-expressions.html#QUERIES-GROUPING-SETS")
  );
  k!(
    "ROLLUP",
    "Hierarchical groupings ending with the empty grouping.",
    "GROUP BY ROLLUP (year, month)",
    pg("queries-table-expressions.html#QUERIES-GROUPING-SETS")
  );
  k!(
    "CUBE",
    "All combinations of grouping columns.",
    "GROUP BY CUBE (year, region)",
    pg("queries-table-expressions.html#QUERIES-GROUPING-SETS")
  );
  k!(
    "ESCAPE",
    "Custom escape character in LIKE patterns.",
    "WHERE name LIKE '50%%' ESCAPE '%'",
    pg("functions-matching.html#FUNCTIONS-LIKE")
  );
  k!(
    "SIMILAR",
    "SIMILAR TO: SQL-standard regex-like pattern matching.",
    "WHERE name SIMILAR TO '%(foo|bar)%'",
    pg("functions-matching.html#FUNCTIONS-SIMILARTO-REGEXP")
  );
  k!(
    "AT TIME ZONE",
    "Re-interpret a timestamp in a different zone.",
    "SELECT created_at AT TIME ZONE 'UTC' FROM users;",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-ZONECONVERT")
  );
  k!(
    "INTERVAL",
    "Interval literal. INTERVAL '1 day'.",
    "now() - INTERVAL '7 days'",
    pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT")
  );
  k!(
    "OVERLAPS",
    "Tuple-of-pairs predicate: (a,b) OVERLAPS (c,d).",
    "(start1, end1) OVERLAPS (start2, end2)",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-OVERLAPS")
  );
  k!(
    "CONCURRENTLY",
    "Create / drop indexes without holding a write lock.",
    "CREATE INDEX CONCURRENTLY idx_users_email ON users (email);",
    pg("sql-createindex.html")
  );
  k!(
    "ONLY",
    "Restrict UPDATE / DELETE / SELECT to a parent table without inheriting children.",
    "DELETE FROM ONLY parent WHERE id = $1;",
    pg("ddl-inherit.html")
  );
  k!(
    "WITH ORDINALITY",
    "Add a row-number column when unnesting an array in FROM.",
    "SELECT * FROM unnest(arr) WITH ORDINALITY",
    pg("queries-table-expressions.html#QUERIES-TABLEFUNCTIONS")
  );
  k!(
    "FETCH",
    "SQL-standard alternative to LIMIT. FETCH FIRST n ROWS ONLY.",
    "SELECT * FROM users ORDER BY id FETCH FIRST 50 ROWS ONLY;",
    pg("sql-select.html#SQL-LIMIT")
  );
  k!(
    "ROWS",
    "FETCH FIRST n ROWS ONLY / ROWS BETWEEN window framing.",
    "FETCH FIRST 50 ROWS ONLY",
    pg("sql-select.html#SQL-LIMIT")
  );
  k!(
    "RANGE",
    "Window frame mode: RANGE between value-relative bounds.",
    "OVER (ORDER BY ts RANGE BETWEEN INTERVAL '1 day' PRECEDING AND CURRENT ROW)",
    pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS")
  );
  k!(
    "CURRENT",
    "Window frame anchor.",
    "OVER (ORDER BY ts ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)",
    pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS")
  );
  k!(
    "UNBOUNDED",
    "Window frame bound: UNBOUNDED PRECEDING / UNBOUNDED FOLLOWING.",
    "OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING)",
    pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS")
  );
  k!(
    "PRECEDING",
    "Window frame bound. UNBOUNDED PRECEDING / N PRECEDING.",
    "OVER (ROWS BETWEEN 3 PRECEDING AND CURRENT ROW)",
    pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS")
  );
  k!(
    "FOLLOWING",
    "Window frame bound. UNBOUNDED FOLLOWING / N FOLLOWING.",
    "OVER (ROWS BETWEEN CURRENT ROW AND 3 FOLLOWING)",
    pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS")
  );
  k!(
    "BETWEEN AND",
    "BETWEEN range predicate uses AND to separate the bounds.",
    "WHERE age BETWEEN 18 AND 65",
    pg("functions-comparison.html")
  );

  // --- DDL: materialized / temporary / unlogged --------------------------
  k!(
    "MATERIALIZED",
    "Use MATERIALIZED VIEW for a stored-query that survives across sessions; refresh with REFRESH.",
    "CREATE MATERIALIZED VIEW daily_sales AS SELECT day, sum(amount) FROM orders GROUP BY day;",
    pg("sql-creatematerializedview.html")
  );
  k!(
    "MATERIALIZED VIEW",
    "A view whose result is physically stored. Run REFRESH MATERIALIZED VIEW to recompute.",
    "CREATE MATERIALIZED VIEW daily_sales AS ...",
    pg("rules-materializedviews.html")
  );
  k!(
    "CREATE MATERIALIZED VIEW",
    "Define a stored, refreshable view.",
    "CREATE MATERIALIZED VIEW daily_sales AS SELECT ...",
    pg("sql-creatematerializedview.html")
  );
  k!(
    "REFRESH MATERIALIZED VIEW",
    "Recompute a materialized view. CONCURRENTLY allows reads during refresh.",
    "REFRESH MATERIALIZED VIEW CONCURRENTLY daily_sales;",
    pg("sql-refreshmaterializedview.html")
  );
  k!("TEMP", "Shorthand for TEMPORARY.", "CREATE TEMP TABLE staging (...);", pg("sql-createtable.html"));
  k!(
    "TEMPORARY",
    "Drops at session end; resides in pg_temp schema.",
    "CREATE TEMPORARY TABLE staging (...);",
    pg("sql-createtable.html")
  );
  k!(
    "UNLOGGED",
    "Skips WAL writes -- faster but data lost on crash. Good for cache tables.",
    "CREATE UNLOGGED TABLE cache_blob (...);",
    pg("sql-createtable.html")
  );
  k!(
    "TABLESPACE",
    "Place the relation on a non-default tablespace.",
    "CREATE TABLE big (...) TABLESPACE fast_ssd;",
    pg("manage-ag-tablespaces.html")
  );
  k!(
    "INHERITS",
    "Single-parent table inheritance. Largely superseded by declarative partitioning.",
    "CREATE TABLE child () INHERITS (parent);",
    pg("ddl-inherit.html")
  );

  // --- Partitioning ------------------------------------------------------
  k!(
    "PARTITION BY RANGE",
    "Range-partitioned table on one or more columns.",
    "CREATE TABLE m (...) PARTITION BY RANGE (created_at);",
    pg("ddl-partitioning.html")
  );
  k!(
    "PARTITION BY LIST",
    "List partitioning -- one partition per discrete value set.",
    "CREATE TABLE m (...) PARTITION BY LIST (country);",
    pg("ddl-partitioning.html")
  );
  k!(
    "PARTITION BY HASH",
    "Hash partitioning -- modulo on a column.",
    "CREATE TABLE m (...) PARTITION BY HASH (id);",
    pg("ddl-partitioning.html")
  );
  k!(
    "PARTITION OF",
    "Declare a child partition of an existing table.",
    "CREATE TABLE m_2026 PARTITION OF m FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');",
    pg("ddl-partitioning.html")
  );
  k!(
    "FOR VALUES",
    "Bound spec for a partition.",
    "PARTITION OF m FOR VALUES FROM ('a') TO ('m')",
    pg("ddl-partitioning.html")
  );
  k!(
    "DEFAULT",
    "Used in PARTITION OF DEFAULT or column DEFAULT clause.",
    "PARTITION OF m DEFAULT",
    pg("ddl-partitioning.html")
  );

  // --- Row locking (FOR UPDATE family) ----------------------------------
  k!(
    "FOR UPDATE",
    "Lock selected rows against concurrent UPDATE/DELETE.",
    "SELECT * FROM accounts WHERE id = $1 FOR UPDATE;",
    pg("sql-select.html#SQL-FOR-UPDATE-SHARE")
  );
  k!(
    "FOR NO KEY UPDATE",
    "Weaker than FOR UPDATE -- still blocks DELETE but not concurrent FK lookups.",
    "SELECT * FROM accounts FOR NO KEY UPDATE;",
    pg("sql-select.html#SQL-FOR-UPDATE-SHARE")
  );
  k!(
    "FOR SHARE",
    "Share lock -- multiple readers, blocks writers.",
    "SELECT * FROM accounts FOR SHARE;",
    pg("sql-select.html#SQL-FOR-UPDATE-SHARE")
  );
  k!(
    "FOR KEY SHARE",
    "Weakest row lock -- only prevents key/PK change.",
    "SELECT * FROM accounts FOR KEY SHARE;",
    pg("sql-select.html#SQL-FOR-UPDATE-SHARE")
  );
  k!(
    "NOWAIT",
    "Abort immediately if the row lock is held elsewhere.",
    "SELECT * FROM job WHERE id = $1 FOR UPDATE NOWAIT;",
    pg("sql-select.html#SQL-FOR-UPDATE-SHARE")
  );
  k!(
    "SKIP LOCKED",
    "Skip rows that are already locked rather than waiting -- ideal for queue tables.",
    "SELECT * FROM job WHERE state='ready' FOR UPDATE SKIP LOCKED LIMIT 10;",
    pg("sql-select.html#SQL-FOR-UPDATE-SHARE")
  );

  // --- ORDER BY / NULLS placement ----------------------------------------
  k!("NULLS FIRST", "Place NULLs at the start of an ORDER BY.", "ORDER BY rank NULLS FIRST", pg("queries-order.html"));
  k!(
    "NULLS LAST",
    "Place NULLs at the end of an ORDER BY (default for ASC).",
    "ORDER BY rank NULLS LAST",
    pg("queries-order.html")
  );

  // --- Constraints: timing / IDENTITY ------------------------------------
  k!(
    "DEFERRABLE",
    "Constraint can be checked at end of transaction; pair with INITIALLY DEFERRED/IMMEDIATE.",
    "ALTER TABLE x ADD CONSTRAINT c CHECK (...) DEFERRABLE INITIALLY DEFERRED;",
    pg("sql-set-constraints.html")
  );
  k!(
    "INITIALLY DEFERRED",
    "Default mode of a DEFERRABLE constraint -- check at commit.",
    "DEFERRABLE INITIALLY DEFERRED",
    pg("sql-set-constraints.html")
  );
  k!(
    "INITIALLY IMMEDIATE",
    "Constraint checked per statement (default for non-deferrable).",
    "DEFERRABLE INITIALLY IMMEDIATE",
    pg("sql-set-constraints.html")
  );
  k!(
    "GENERATED ALWAYS AS",
    "Stored or virtual generated column; the value comes from the expression on write.",
    "GENERATED ALWAYS AS (price * qty) STORED",
    pg("ddl-generated-columns.html")
  );
  k!(
    "GENERATED BY DEFAULT AS",
    "Sequence-backed identity column that the user MAY override at INSERT.",
    "id BIGINT GENERATED BY DEFAULT AS IDENTITY",
    pg("sql-createtable.html")
  );

  // --- Async / utility ---------------------------------------------------
  k!("LISTEN", "Subscribe to NOTIFY events on a channel.", "LISTEN job_inserted;", pg("sql-listen.html"));
  k!(
    "NOTIFY",
    "Send an asynchronous notification on a channel.",
    "NOTIFY job_inserted, 'payload';",
    pg("sql-notify.html")
  );
  k!("UNLISTEN", "Stop receiving NOTIFY events.", "UNLISTEN job_inserted;", pg("sql-unlisten.html"));
  k!("CHECKPOINT", "Force a WAL checkpoint -- diagnostic / maintenance use.", "CHECKPOINT;", pg("sql-checkpoint.html"));
  k!(
    "LOCK TABLE",
    "Acquire an explicit table-level lock.",
    "LOCK TABLE accounts IN SHARE ROW EXCLUSIVE MODE;",
    pg("sql-lock.html")
  );
  k!(
    "PREPARE",
    "Server-side prepared statement -- $1, $2 placeholders, planned once.",
    "PREPARE upsert_user(text) AS INSERT INTO users(email) VALUES ($1);",
    pg("sql-prepare.html")
  );
  k!("DEALLOCATE", "Release a server-side prepared statement.", "DEALLOCATE upsert_user;", pg("sql-deallocate.html"));

  // --- PL/pgSQL control flow --------------------------------------------
  k!(
    "EXIT",
    "Leave the enclosing loop. EXIT WHEN <cond> is the idiomatic form.",
    "EXIT WHEN i > 10;",
    pg("plpgsql-control-structures.html")
  );
  k!(
    "CONTINUE",
    "Skip to next iteration of the enclosing loop.",
    "CONTINUE WHEN should_skip(row);",
    pg("plpgsql-control-structures.html")
  );
  k!(
    "PERFORM",
    "Execute a SELECT and discard its result (used inside PL/pgSQL).",
    "PERFORM trigger_side_effect();",
    pg("plpgsql-statements.html")
  );
  k!(
    "FOUND",
    "Boolean set after the most recent SQL command (UPDATE/INSERT/SELECT INTO etc.).",
    "IF NOT FOUND THEN RAISE EXCEPTION 'no row'; END IF;",
    pg("plpgsql-statements.html#PLPGSQL-STATEMENTS-DIAGNOSTICS")
  );
  k!(
    "STRICT",
    "SELECT INTO STRICT -- raise if 0 or >1 rows. Also a function volatility marker.",
    "SELECT id INTO STRICT v_id FROM users WHERE email = $1;",
    pg("plpgsql-statements.html#PLPGSQL-STATEMENTS-SQL-ONEROW")
  );
  k!(
    "INTO STRICT",
    "PL/pgSQL: assign exactly one row or raise.",
    "SELECT * INTO STRICT v_row FROM users WHERE id = $1;",
    pg("plpgsql-statements.html#PLPGSQL-STATEMENTS-SQL-ONEROW")
  );

  // --- Misc DDL & utility ------------------------------------------------
  k!(
    "ENABLE TRIGGER",
    "Re-enable a trigger that was disabled.",
    "ALTER TABLE users ENABLE TRIGGER audit_users;",
    pg("sql-altertable.html")
  );
  k!(
    "DISABLE TRIGGER",
    "Temporarily turn off a trigger.",
    "ALTER TABLE users DISABLE TRIGGER audit_users;",
    pg("sql-altertable.html")
  );
  k!(
    "SET DATA TYPE",
    "Change a column's type, optionally with USING <expr>.",
    "ALTER TABLE t ALTER COLUMN c SET DATA TYPE bigint USING c::bigint;",
    pg("sql-altertable.html")
  );
  k!(
    "DROP NOT NULL",
    "Make a NOT NULL column nullable again.",
    "ALTER TABLE t ALTER COLUMN c DROP NOT NULL;",
    pg("sql-altertable.html")
  );
  k!(
    "SET NOT NULL",
    "Forbid NULL in a column. Postgres scans the table to verify.",
    "ALTER TABLE t ALTER COLUMN c SET NOT NULL;",
    pg("sql-altertable.html")
  );

  m
}
