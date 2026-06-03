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
    "WINDOW",
    "Named window clause -- declare a reusable window spec for window functions.",
    "SELECT rank() OVER w FROM t WINDOW w AS (PARTITION BY id ORDER BY ts)",
    pg("sql-select.html#SQL-WINDOW")
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

  // ---- Missing-essentials batch (auto-added) --------------------
  k!("NEXT", "FETCH NEXT clause -- equivalent to LIMIT in SQL standard syntax.", "SELECT * FROM t FETCH NEXT 10 ROWS ONLY;", pg("sql-select.html"));
  k!("NATURAL", "NATURAL JOIN -- automatic equi-join on common column names. Brittle; prefer explicit ON/USING.", "SELECT * FROM a NATURAL JOIN b;", pg("queries-table-expressions.html"));
  k!("PARTITION", "Window-frame partitioning OR table partitioning (PARTITION BY RANGE/LIST/HASH).", "OVER (PARTITION BY dept)", pg("ddl-partitioning.html"));
  k!("GROUPS", "Window-frame mode counting peer groups (vs ROWS / RANGE).", "OVER (ORDER BY x GROUPS BETWEEN 1 PRECEDING AND CURRENT ROW)", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("ISOLATION", "Transaction isolation level.", "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ;", pg("transaction-iso.html"));
  k!("LEVEL", "Used with ISOLATION LEVEL.", "BEGIN ISOLATION LEVEL SERIALIZABLE;", pg("transaction-iso.html"));
  k!("IMMEDIATE", "SET CONSTRAINTS ... IMMEDIATE -- check immediately at statement end.", "SET CONSTRAINTS ALL IMMEDIATE;", pg("sql-set-constraints.html"));
  k!("DEFERRED", "SET CONSTRAINTS ... DEFERRED -- defer check until commit. Constraint must be DEFERRABLE.", "SET CONSTRAINTS fk_x DEFERRED;", pg("sql-set-constraints.html"));
  k!("TYPE", "CREATE TYPE / ALTER TYPE -- composite, enum, range, base types.", "CREATE TYPE status AS ENUM ('open','closed');", pg("sql-createtype.html"));
  k!("DOMAIN", "CREATE DOMAIN -- named NOT NULL/CHECK shorthand over a base type.", "CREATE DOMAIN email_t AS TEXT CHECK (VALUE ~ '@');", pg("sql-createdomain.html"));
  k!("DATABASE", "CREATE/ALTER/DROP DATABASE.", "CREATE DATABASE mydb OWNER alice;", pg("sql-createdatabase.html"));
  k!("PUBLIC", "Special pseudo-role granted access to everyone.", "GRANT SELECT ON t TO PUBLIC;", pg("sql-grant.html"));
  k!("PRIVILEGES", "Used with GRANT/REVOKE ALL PRIVILEGES.", "GRANT ALL PRIVILEGES ON t TO bob;", pg("sql-grant.html"));
  k!("SECURITY", "SECURITY DEFINER / INVOKER on functions; controls whose privileges run the body.", "CREATE FUNCTION f() ... SECURITY DEFINER;", pg("sql-createfunction.html"));
  k!("DEFINER", "Function runs with the privileges of the role that defined it.", "SECURITY DEFINER", pg("sql-createfunction.html"));
  k!("INVOKER", "Function runs with the privileges of the calling role (default).", "SECURITY INVOKER", pg("sql-createfunction.html"));
  k!("OWNED", "ALTER X OWNED BY ... -- transfer all objects owned by a role.", "ALTER SCHEMA s OWNER TO alice;", pg("sql-alterrole.html"));
  k!("EXCLUDE", "Exclusion constraint -- generalised UNIQUE using operators.", "EXCLUDE USING gist (room WITH =, span WITH &&)", pg("sql-createtable.html"));
  k!("GIST", "Generalised Search Tree -- index method for geometric / range / fts data.", "CREATE INDEX ... USING gist (col);", pg("gist.html"));
  k!("GIN", "Generalised Inverted Index -- arrays, jsonb, full-text.", "CREATE INDEX ... USING gin (col);", pg("gin.html"));
  k!("BRIN", "Block Range INdex -- small index for large, naturally sorted tables.", "CREATE INDEX ... USING brin (ts);", pg("brin.html"));
  k!("BTREE", "B-tree -- default index method.", "CREATE INDEX ... USING btree (col);", pg("btree.html"));
  k!("HASH", "Hash index -- equality only; WAL-logged since PG10.", "CREATE INDEX ... USING hash (col);", pg("hash-index.html"));
  k!("LOCK", "Acquire a table-level lock.", "LOCK TABLE t IN ACCESS EXCLUSIVE MODE;", pg("sql-lock.html"));
  k!("SHARE", "Lock mode allowing other readers (ROW SHARE / SHARE / SHARE UPDATE EXCLUSIVE / SHARE ROW EXCLUSIVE).", "LOCK t IN SHARE MODE;", pg("explicit-locking.html"));
  k!("ACCESS", "ACCESS SHARE / ACCESS EXCLUSIVE lock modes.", "LOCK t IN ACCESS EXCLUSIVE MODE;", pg("explicit-locking.html"));
  k!("EXCLUSIVE", "EXCLUSIVE / ACCESS EXCLUSIVE / SHARE ROW EXCLUSIVE lock mode.", "LOCK t IN EXCLUSIVE MODE;", pg("explicit-locking.html"));
  k!("MODE", "Lock-mode keyword in LOCK statements.", "LOCK t IN SHARE MODE;", pg("sql-lock.html"));
  k!("NO", "NO ACTION / NO INHERIT / NO CYCLE / WITH NO DATA.", "ON DELETE NO ACTION", pg("sql-createtable.html"));
  k!("SKIP", "SKIP LOCKED -- skip rows already locked by another transaction.", "SELECT ... FOR UPDATE SKIP LOCKED;", pg("sql-select.html"));
  k!("LOCKED", "Used with SKIP LOCKED.", "FOR UPDATE SKIP LOCKED", pg("sql-select.html"));
  k!("WAIT", "NOWAIT / WAIT -- alternative to SKIP LOCKED in SELECT FOR locking.", "SELECT ... FOR UPDATE NOWAIT;", pg("sql-select.html"));
  k!("ABORT", "Synonym for ROLLBACK.", "ABORT;", pg("sql-abort.html"));
  k!("CLUSTER", "CLUSTER TABLE -- physically reorder by an index.", "CLUSTER t USING idx;", pg("sql-cluster.html"));
  k!("CSV", "COPY ... FORMAT CSV.", "COPY t FROM '/p.csv' CSV HEADER;", pg("sql-copy.html"));
  k!("PROGRAM", "COPY ... FROM/TO PROGRAM 'cmd'.", "COPY t FROM PROGRAM 'gunzip -c f.gz' CSV;", pg("sql-copy.html"));
  k!("DELIMITER", "COPY DELIMITER ','.", "COPY t FROM '/p.csv' DELIMITER ',';", pg("sql-copy.html"));
  k!("HEADER", "COPY ... CSV HEADER -- skip first row.", "COPY t FROM 'p.csv' CSV HEADER;", pg("sql-copy.html"));
  k!("QUOTE", "COPY ... CSV QUOTE '\"'.", "COPY t TO 'p.csv' CSV QUOTE '\\'';", pg("sql-copy.html"));
  k!("FORCE", "COPY ... FORCE QUOTE / FORCE NOT NULL.", "COPY t TO 'p.csv' CSV FORCE QUOTE *;", pg("sql-copy.html"));
  k!("VERBOSE", "EXPLAIN VERBOSE / VACUUM VERBOSE.", "EXPLAIN VERBOSE SELECT ...;", pg("sql-explain.html"));
  k!("FORMAT", "EXPLAIN (FORMAT JSON|TEXT|YAML|XML).", "EXPLAIN (FORMAT JSON) SELECT ...;", pg("sql-explain.html"));
  k!("TIMING", "EXPLAIN (TIMING true|false) -- per-node timing.", "EXPLAIN (ANALYZE, TIMING true) SELECT ...;", pg("sql-explain.html"));
  k!("SETTINGS", "EXPLAIN (SETTINGS true) -- list non-default GUCs.", "EXPLAIN (SETTINGS true) SELECT ...;", pg("sql-explain.html"));
  k!("ENABLE", "ALTER TABLE ENABLE TRIGGER / RULE / ROW LEVEL SECURITY / REPLICA.", "ALTER TABLE t ENABLE ROW LEVEL SECURITY;", pg("sql-altertable.html"));
  k!("DISABLE", "ALTER TABLE DISABLE TRIGGER / RULE / RLS.", "ALTER TABLE t DISABLE TRIGGER ALL;", pg("sql-altertable.html"));
  k!("REPLICA", "ALTER TABLE REPLICA IDENTITY -- pick what gets logged for logical replication.", "ALTER TABLE t REPLICA IDENTITY FULL;", pg("sql-altertable.html"));
  k!("RULE", "CREATE RULE -- query rewriter (mostly used internally by views).", "CREATE RULE ... AS ON SELECT TO ... DO INSTEAD ...;", pg("sql-createrule.html"));

  // ---- PG keyword sweep (bulk-added) -------------------------
  k!("ABSOLUTE", "FETCH ABSOLUTE <n> FROM <cursor> -- jump to row <n> (1-based) without sequential scan.", "FETCH ABSOLUTE 100 FROM c;", pg("sql-fetch.html"));
  k!("ACTION", "FK referential action: NO ACTION (default), CASCADE, SET NULL, SET DEFAULT, RESTRICT.", "FOREIGN KEY (uid) REFERENCES users(id) ON DELETE NO ACTION", pg("sql-createtable.html#SQL-CREATETABLE-REFERENCES"));
  k!("ADMIN", "GRANT <role> TO <member> WITH ADMIN OPTION -- recipient can grant <role> to others.", "GRANT app_admin TO alice WITH ADMIN OPTION;", pg("sql-grant.html"));
  k!("AGGREGATE", "CREATE/ALTER/DROP AGGREGATE -- user-defined aggregate function with state transition + final fn.", "CREATE AGGREGATE sum_squares(int) (SFUNC = int4pl, STYPE = int);", pg("sql-createaggregate.html"));
  k!("ALSO", "CREATE RULE ... DO ALSO <action> -- rule runs ALONGSIDE the original DML (vs DO INSTEAD).", "CREATE RULE log_ins AS ON INSERT TO t DO ALSO INSERT INTO audit VALUES (NEW.id);", pg("sql-createrule.html"));
  k!("ANALYSE", "British spelling of ANALYZE; same behavior.", "EXPLAIN (ANALYSE) SELECT * FROM t;", pg("sql-analyze.html"));
  k!("ASENSITIVE", "DECLARE <c> ASENSITIVE CURSOR ... -- SQL-standard noise word (default in PG). Cursor's visibility of concurrent updates is implementation-defined.", "DECLARE c ASENSITIVE CURSOR FOR SELECT * FROM t;", pg("sql-declare.html"));
  k!("ASSERTION", "SQL-standard `CREATE ASSERTION <name> CHECK (...)` -- reserved in PG, not implemented.", "-- reserved", pg("appendix-keywords.html"));
  k!("ASSIGNMENT", "CREATE CAST ... AS ASSIGNMENT -- cast injected during `INSERT`/`UPDATE` column-value assignment, not in general expressions.", "CREATE CAST (text AS my_t) WITH FUNCTION my_t_in(text) AS ASSIGNMENT;", pg("sql-createcast.html"));
  k!("ASYMMETRIC", "BETWEEN [ASYMMETRIC] -- default; requires lower <= upper. Compare with SYMMETRIC.", "WHERE x BETWEEN ASYMMETRIC 1 AND 10", pg("functions-comparison.html"));
  k!("AT", "<timestamptz> AT TIME ZONE '<tz>' -- convert between zones; AT LOCAL (PG17+) for session zone.", "SELECT now() AT TIME ZONE 'UTC';", pg("functions-datetime.html#FUNCTIONS-DATETIME-ZONECONVERT"));
  k!("ATTACH", "ALTER TABLE <parent> ATTACH PARTITION <child> FOR VALUES ... -- promote regular table to a partition.", "ALTER TABLE events ATTACH PARTITION events_2026 FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');", pg("sql-altertable.html"));
  k!("ATTRIBUTE", "ALTER TYPE <composite> { ADD | DROP | ALTER } ATTRIBUTE <a> ... -- modify composite type's columns.", "ALTER TYPE point3d ADD ATTRIBUTE z double precision;", pg("sql-altertype.html"));
  k!("AUTHORIZATION", "CREATE SCHEMA <s> AUTHORIZATION <role> -- create schema owned by <role>.", "CREATE SCHEMA app AUTHORIZATION app_owner;", pg("sql-createschema.html"));
  k!("BACKWARD", "FETCH BACKWARD <n> FROM <cursor> -- step <n> rows toward start. Requires SCROLL cursor.", "FETCH BACKWARD 10 FROM c;", pg("sql-fetch.html"));
  k!("BIGINT", "Signed 8-byte integer, range -9223372036854775808..+9223372036854775807. Alias `int8`.", "CREATE TABLE counters (id BIGSERIAL PRIMARY KEY, hits BIGINT NOT NULL DEFAULT 0);", pg("datatype-numeric.html#DATATYPE-INT"));
  k!("BINARY", "COPY ... WITH (FORMAT binary) -- machine-readable wire format (no header line). Older syntax: `COPY ... BINARY`.", "COPY t TO '/tmp/d.bin' WITH (FORMAT binary);", pg("sql-copy.html"));
  k!("BIT", "Fixed-length bit string. `BIT(n)` -- exactly n bits. Variable form: `BIT VARYING(n)` / `VARBIT(n)`.", "flags BIT(8)", pg("datatype-bit.html"));
  k!("BOOLEAN", "Three-valued logic: TRUE / FALSE / NULL. 1 byte storage. Alias `bool`.", "CREATE TABLE flags (k text PRIMARY KEY, v BOOLEAN NOT NULL DEFAULT false);", pg("datatype-boolean.html"));
  k!("BOTH", "TRIM([BOTH] <chars> FROM <s>) -- strip <chars> from both ends. Default direction.", "SELECT TRIM(BOTH ' ' FROM '  hello  ');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("BREADTH", "WITH RECURSIVE ... SEARCH BREADTH FIRST BY <cols> SET <out> -- BFS traversal annotation for recursive CTEs (PG14+).", "WITH RECURSIVE t AS (...) SEARCH BREADTH FIRST BY id SET ord SELECT * FROM t ORDER BY ord;", pg("queries-with.html#QUERIES-WITH-SEARCH"));
  k!("CACHE", "CREATE SEQUENCE ... CACHE <n> -- preallocate `<n>` values per backend; reduces locking but wastes IDs on crash.", "CREATE SEQUENCE s CACHE 50;", pg("sql-createsequence.html"));
  k!("CALLED", "CREATE FUNCTION ... CALLED ON NULL INPUT -- default; fn body runs even when an arg is NULL. Opposite of `STRICT` / `RETURNS NULL ON NULL INPUT`.", "CREATE FUNCTION f(int) RETURNS int LANGUAGE sql CALLED ON NULL INPUT AS $$ SELECT 1 $$;", pg("sql-createfunction.html"));
  k!("CASCADED", "CREATE VIEW ... WITH CASCADED CHECK OPTION -- updated rows must satisfy this view's predicate AND all underlying views'.", "CREATE VIEW v WITH CASCADED CHECK OPTION AS SELECT * FROM t WHERE active;", pg("sql-createview.html"));
  k!("CATALOG", "SQL-standard alias for database (synonymous with `CURRENT_CATALOG`). Mostly reserved noise in PG.", "SELECT CURRENT_CATALOG;", pg("functions-info.html"));
  k!("CHAIN", "COMMIT AND CHAIN / ROLLBACK AND CHAIN -- end the current transaction and immediately start a new one with the same options.", "COMMIT AND CHAIN;", pg("sql-commit.html"));
  k!("CHAR", "Fixed-length blank-padded string. `CHAR(n)` = `CHARACTER(n)`. Prefer TEXT unless padding is required.", "code CHAR(3)", pg("datatype-character.html"));
  k!("CHARACTER", "Fixed-length blank-padded string. `CHARACTER(n)` = `CHAR(n)`. Prefer TEXT unless you really need padding.", "name CHARACTER(10)", pg("datatype-character.html"));
  k!("CHARACTERISTICS", "SET SESSION CHARACTERISTICS AS TRANSACTION ... -- set default transaction options for the session.", "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL SERIALIZABLE READ ONLY;", pg("sql-set-transaction.html"));
  k!("CLASS", "CREATE/DROP OPERATOR CLASS -- set of operators + support functions an index AM uses for a specific type.", "CREATE OPERATOR CLASS my_int_ops DEFAULT FOR TYPE int USING btree AS OPERATOR 1 < ...;", pg("sql-createopclass.html"));
  k!("CLOSE", "CLOSE <cursor> | CLOSE ALL -- release cursor and its resources.", "CLOSE c;", pg("sql-close.html"));
  k!("COALESCE", "Returns the first non-NULL argument; short-circuits.", "SELECT COALESCE(nickname, name, 'anon') FROM users;", pg("functions-conditional.html#FUNCTIONS-COALESCE-NVL-IFNULL"));
  k!("COLLATION", "Column/expression collation -- locale rules for ordering/comparison of strings.", "name TEXT COLLATE \"en_US.utf8\"", pg("sql-createcollation.html"));
  k!("COLUMNS", "JSON_TABLE(... COLUMNS (<col_specs>)) -- declare projected output columns of a JSON_TABLE expression.", "SELECT * FROM JSON_TABLE(j, '$[*]' COLUMNS (id INT PATH '$.id', name TEXT PATH '$.n'));", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("COMMENTS", "CREATE TABLE ... LIKE parent INCLUDING COMMENTS -- carry COMMENT ON COLUMN annotations over from parent.", "CREATE TABLE child (LIKE parent INCLUDING COMMENTS);", pg("sql-createtable.html"));
  k!("COMMITTED", "SET TRANSACTION ISOLATION LEVEL READ COMMITTED -- default isolation; sees rows committed before each statement starts.", "BEGIN ISOLATION LEVEL READ COMMITTED;", pg("transaction-iso.html#XACT-READ-COMMITTED"));
  k!("COMPRESSION", "ALTER COLUMN ... SET COMPRESSION { pglz | lz4 | default } -- per-column TOAST compression (PG14+).", "ALTER TABLE t ALTER COLUMN body SET COMPRESSION lz4;", pg("sql-altertable.html"));
  k!("CONFIGURATION", "CREATE/ALTER/DROP TEXT SEARCH CONFIGURATION -- token-to-dictionary map for full-text search.", "CREATE TEXT SEARCH CONFIGURATION my (COPY = simple);", pg("sql-createtsconfig.html"));
  k!("CONNECTION", "CREATE/ALTER/DROP PUBLICATION ... / SUBSCRIPTION ... CONNECTION '...' -- connection string of the publisher cluster.", "CREATE SUBSCRIPTION s CONNECTION 'host=pub user=repl' PUBLICATION p;", pg("sql-createsubscription.html"));
  k!("CONSTRAINTS", "SET CONSTRAINTS { ALL | <names> } { DEFERRED | IMMEDIATE } -- defer FK/UNIQUE checks until COMMIT, or force re-check now.", "SET CONSTRAINTS ALL DEFERRED;", pg("sql-set-constraints.html"));
  k!("CONTENT", "xmlparse(CONTENT '<x/>') / xmlserialize(CONTENT ...) -- XML content (fragment), no DOCUMENT prologue required.", "SELECT xmlparse(CONTENT '<a/>');", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("CONVERSION", "CREATE/DROP CONVERSION -- character-set conversion between two encodings.", "CREATE CONVERSION my_conv FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8;", pg("sql-createconversion.html"));
  k!("COST", "CREATE FUNCTION ... COST <c> -- planner hint: per-row execution cost (default 100; 1 for built-ins, 10 for SQL fns).", "CREATE FUNCTION f() RETURNS int LANGUAGE sql COST 10 AS $$ SELECT 1 $$;", pg("sql-createfunction.html"));
  k!("CURRENT_CATALOG", "Returns the current database name (SQL standard synonym of `current_database()`).", "SELECT CURRENT_CATALOG;", pg("functions-info.html"));
  k!("CURRENT_DATE", "Returns today's date in the session time zone.", "SELECT CURRENT_DATE;", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  k!("CURRENT_ROLE", "Returns the active role name (synonym of `CURRENT_USER`).", "SELECT CURRENT_ROLE;", pg("functions-info.html"));
  k!("CURRENT_SCHEMA", "Returns the first non-implicit schema in `search_path`.", "SELECT CURRENT_SCHEMA;", pg("functions-info.html"));
  k!("CURRENT_TIME", "Returns the wall-clock time-with-time-zone for the current transaction.", "SELECT CURRENT_TIME;", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  k!("CURRENT_TIMESTAMP", "Returns transaction start timestamptz (same value within one transaction).", "SELECT CURRENT_TIMESTAMP;", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  k!("CURRENT_USER", "Returns the active SQL user name (effective role for permission checks).", "SELECT CURRENT_USER;", pg("functions-info.html"));
  k!("CURSOR", "DECLARE <name> CURSOR FOR <query> -- server-side iterator over query results.", "DECLARE c CURSOR FOR SELECT * FROM big;", pg("sql-declare.html"));
  k!("CYCLE", "CREATE SEQUENCE ... CYCLE -- wrap to MINVALUE/MAXVALUE instead of raising error when exhausted.", "CREATE SEQUENCE s CYCLE;", pg("sql-createsequence.html"));
  k!("DATA", "Two uses: `CREATE TABLE AS ... WITH [NO] DATA` (copy rows or not), and `CREATE FOREIGN DATA WRAPPER`.", "CREATE TABLE snap AS SELECT * FROM t WITH NO DATA;", pg("sql-createtableas.html"));
  k!("DAY", "EXTRACT(DAY FROM <ts|interval>) -- day-of-month for timestamps; total days for intervals.", "SELECT EXTRACT(DAY FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("DEC", "SQL-standard short form of DECIMAL. Same precision/scale rules.", "amount DEC(12,2)", pg("datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL"));
  k!("DECIMAL", "Exact arbitrary-precision number. Alias `NUMERIC`. Use for money/measurements; never `float`.", "price DECIMAL(12,2) NOT NULL CHECK (price >= 0)", pg("datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL"));
  k!("DEFAULTS", "CREATE TABLE ... LIKE parent INCLUDING DEFAULTS -- copy column default expressions from the parent table.", "CREATE TABLE child (LIKE parent INCLUDING DEFAULTS);", pg("sql-createtable.html"));
  k!("DELIMITERS", "Legacy keyword for delimiter list in `CREATE OPERATOR` AST -- in modern PG superseded by `COPY DELIMITER '<c>'`.", "COPY t FROM '/tmp/d.csv' WITH (DELIMITER ',');", pg("sql-copy.html"));
  k!("DEPENDS", "ALTER ... DEPENDS ON EXTENSION <name> -- mark object as dependent on extension; DROP EXTENSION cascades.", "ALTER FUNCTION my_fn() DEPENDS ON EXTENSION my_ext;", pg("sql-alterfunction.html"));
  k!("DEPTH", "WITH RECURSIVE ... SEARCH DEPTH FIRST BY <cols> SET <out> -- DFS traversal annotation for recursive CTEs (PG14+).", "WITH RECURSIVE t AS (...) SEARCH DEPTH FIRST BY id SET ord SELECT * FROM t ORDER BY ord;", pg("queries-with.html#QUERIES-WITH-SEARCH"));
  k!("DETACH", "ALTER TABLE <parent> DETACH PARTITION <child> [CONCURRENTLY | FINALIZE] -- remove partition from a partitioned table; CONCURRENTLY avoids AccessExclusive lock (PG14+).", "ALTER TABLE events DETACH PARTITION events_2019 CONCURRENTLY;", pg("sql-altertable.html"));
  k!("DICTIONARY", "CREATE/ALTER/DROP TEXT SEARCH DICTIONARY -- normalizer (stem/synonym/stopword) used by FTS configs.", "CREATE TEXT SEARCH DICTIONARY english_stem (TEMPLATE = snowball, LANGUAGE = 'english');", pg("sql-createtsdictionary.html"));
  k!("DISCARD", "DISCARD { ALL | PLANS | SEQUENCES | TEMP | TEMPORARY } -- reset session state. `DISCARD ALL` is what connection poolers run between hand-offs.", "DISCARD ALL;", pg("sql-discard.html"));
  k!("DOCUMENT", "xmlparse(DOCUMENT '<x/>') / IS DOCUMENT predicate -- requires a complete XML document (single root, prologue allowed).", "SELECT x IS DOCUMENT FROM xml_t;", pg("functions-xml.html"));
  k!("DOUBLE", "Part of `DOUBLE PRECISION` (8-byte IEEE-754). Standalone DOUBLE is not a PG type.", "ratio DOUBLE PRECISION", pg("datatype-numeric.html#DATATYPE-FLOAT"));
  k!("ENCODING", "COPY ... ENCODING '<charset>' / CREATE DATABASE ... ENCODING <charset> -- declare character set.", "COPY t FROM '/tmp/d.csv' WITH (FORMAT csv, ENCODING 'UTF8');", pg("sql-copy.html"));
  k!("ENCRYPTED", "CREATE/ALTER ROLE ... [ENCRYPTED] PASSWORD '<pw>' -- store password hashed (now always true; raw text retained for syntax compat).", "ALTER ROLE alice ENCRYPTED PASSWORD 'secret';", pg("sql-createrole.html"));
  k!("ENUM", "CREATE TYPE <t> AS ENUM ('a','b',...) -- fixed list of string values stored compactly. Add new labels with `ALTER TYPE ... ADD VALUE`.", "CREATE TYPE mood AS ENUM ('sad','ok','happy');", pg("datatype-enum.html"));
  k!("EVENT", "CREATE/ALTER/DROP EVENT TRIGGER -- fires on DDL events (ddl_command_start, ddl_command_end, table_rewrite, sql_drop).", "CREATE EVENT TRIGGER no_drops ON sql_drop EXECUTE FUNCTION block_drops();", pg("sql-createeventtrigger.html"));
  k!("EXCLUDING", "CREATE TABLE ... LIKE parent EXCLUDING { ALL | COMMENTS | CONSTRAINTS | DEFAULTS | IDENTITY | INDEXES | STATISTICS | STORAGE | GENERATED | COMPRESSION } -- inverse of INCLUDING.", "CREATE TABLE child (LIKE parent INCLUDING ALL EXCLUDING INDEXES);", pg("sql-createtable.html"));
  k!("EXPRESSION", "ALTER TABLE ... ALTER COLUMN <c> { SET | DROP } EXPRESSION -- swap or remove the generated-column expression (PG16+).", "ALTER TABLE t ALTER COLUMN full_name SET EXPRESSION AS (first || ' ' || last);", pg("sql-altertable.html"));
  k!("EXTENSION", "CREATE / ALTER / DROP EXTENSION <name> -- packaged set of objects (tables, fns, ops) installed/upgraded as a unit.", "CREATE EXTENSION IF NOT EXISTS pg_trgm;", pg("sql-createextension.html"));
  k!("EXTERNAL", "ALTER TABLE ... ALTER COLUMN <c> SET STORAGE EXTERNAL -- TOAST out-of-line but uncompressed.", "ALTER TABLE t ALTER COLUMN body SET STORAGE EXTERNAL;", pg("sql-altertable.html"));
  k!("EXTRACT", "EXTRACT(<field> FROM <timestamp|interval>) -- pull a sub-component (year, month, dow, epoch, ...).", "SELECT EXTRACT(YEAR FROM ts), EXTRACT(EPOCH FROM age(now(), birth)) FROM users;", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("FAMILY", "CREATE / ALTER / DROP OPERATOR FAMILY -- groups of operator classes sharing input types for index AMs.", "CREATE OPERATOR FAMILY int_ops USING btree;", pg("sql-createopfamily.html"));
  k!("FINALIZE", "ALTER TABLE ... DETACH PARTITION <child> FINALIZE -- complete a previously-interrupted DETACH CONCURRENTLY.", "ALTER TABLE events DETACH PARTITION events_2019 FINALIZE;", pg("sql-altertable.html"));
  k!("FLOAT", "Inexact IEEE-754 number. `FLOAT(n)` maps to REAL (n<=24) or DOUBLE PRECISION (n>=25).", "weight FLOAT(24)", pg("datatype-numeric.html#DATATYPE-FLOAT"));
  k!("FORWARD", "FETCH FORWARD <n> FROM <cursor> -- fetch next <n> rows (default direction).", "FETCH FORWARD 100 FROM c;", pg("sql-fetch.html"));
  k!("FREEZE", "COPY ... WITH (FREEZE) -- mark rows as frozen so they skip VACUUM FREEZE later. Only valid when loading into a just-created table in the same xact.", "COPY t FROM '/tmp/d.csv' WITH (FORMAT csv, FREEZE);", pg("sql-copy.html"));
  k!("FUNCTIONS", "GRANT/REVOKE ... ON ALL FUNCTIONS IN SCHEMA / ALTER DEFAULT PRIVILEGES ... ON FUNCTIONS -- bulk privilege ops on functions.", "GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("GLOBAL", "Accepted for SQL-standard compat in `CREATE [GLOBAL] TEMPORARY TABLE`. PG ignores GLOBAL -- all temp tables are session-local.", "CREATE GLOBAL TEMPORARY TABLE t (id int);", pg("sql-createtable.html"));
  k!("GRANTED", "REVOKE ... GRANTED BY <role> -- drop only the grant made by <role>. Required to clean privileges installed by an extension.", "REVOKE SELECT ON t FROM alice GRANTED BY ext_owner;", pg("sql-revoke.html"));
  k!("GREATEST", "Returns the largest of the listed values (NULLs ignored unless all are NULL).", "SELECT GREATEST(a, b, c) FROM t;", pg("functions-conditional.html#FUNCTIONS-GREATEST-LEAST"));
  k!("GROUPING", "GROUPING(<col>[,...]) -- bitmask telling which columns of a GROUPING SETS / ROLLUP / CUBE row are NULL because the group rolled them up.", "SELECT region, GROUPING(region) AS gr FROM s GROUP BY ROLLUP (region);", pg("functions-aggregate.html#FUNCTIONS-GROUPING-TABLE"));
  k!("HANDLER", "CREATE LANGUAGE / CREATE FOREIGN DATA WRAPPER ... HANDLER <fn> -- C function implementing the PL or FDW.", "CREATE FOREIGN DATA WRAPPER fdw HANDLER my_fdw_handler;", pg("sql-createforeigndatawrapper.html"));
  k!("HOLD", "DECLARE ... WITH HOLD CURSOR -- cursor survives COMMIT (vs WITHOUT HOLD, default).", "DECLARE c CURSOR WITH HOLD FOR SELECT id FROM t;", pg("sql-declare.html"));
  k!("HOUR", "EXTRACT(HOUR FROM <ts|interval>) -- hour-of-day 0..23; interval hour part.", "SELECT EXTRACT(HOUR FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("IMPLICIT", "CREATE CAST ... AS IMPLICIT -- planner may inject the cast silently. Use sparingly -- breeds ambiguous overload resolution.", "CREATE CAST (text AS my_t) WITH FUNCTION my_t_in(text) AS IMPLICIT;", pg("sql-createcast.html"));
  k!("IMPORT", "IMPORT FOREIGN SCHEMA <s> FROM SERVER <srv> INTO <local_s> -- bulk-create foreign tables for every remote table.", "IMPORT FOREIGN SCHEMA public FROM SERVER remote INTO ext_pub;", pg("sql-importforeignschema.html"));
  k!("INCLUDE", "CREATE INDEX ... INCLUDE (<cols>) -- non-key columns stored in leaf pages; enables index-only scans without affecting uniqueness.", "CREATE INDEX ix_users_email ON users (email) INCLUDE (id, name);", pg("sql-createindex.html"));
  k!("INCREMENT", "CREATE SEQUENCE ... INCREMENT [BY] <n> -- step between successive `nextval()` results.", "CREATE SEQUENCE s INCREMENT BY 10;", pg("sql-createsequence.html"));
  k!("INDEXES", "CREATE TABLE ... LIKE parent INCLUDING INDEXES -- copy parent's PK/UNIQUE/EXCLUDE constraints + corresponding indexes.", "CREATE TABLE child (LIKE parent INCLUDING INDEXES);", pg("sql-createtable.html"));
  k!("INHERIT", "CREATE/ALTER ROLE ... [NO]INHERIT -- whether the role automatically uses the privileges of granted roles.", "ALTER ROLE alice INHERIT;", pg("sql-createrole.html"));
  k!("INITIALLY", "Constraint timing: DEFERRABLE INITIALLY { DEFERRED | IMMEDIATE } -- per-transaction default check mode for deferrable constraints.", "FOREIGN KEY (uid) REFERENCES users(id) DEFERRABLE INITIALLY DEFERRED", pg("sql-createtable.html"));
  k!("INLINE", "CREATE LANGUAGE ... INLINE <fn> -- C function that executes anonymous DO blocks. Internal.", "CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler INLINE plpgsql_inline_handler;", pg("sql-createlanguage.html"));
  k!("INOUT", "Function/procedure parameter mode: combines IN + OUT -- caller passes a value, callee returns one.", "CREATE PROCEDURE p(INOUT counter int) LANGUAGE plpgsql AS $$ BEGIN counter := counter + 1; END $$;", pg("sql-createfunction.html"));
  k!("INPUT", "CREATE TYPE ... INPUT = <fn> -- C function parsing text input form. Pair with `OUTPUT`.", "CREATE TYPE complex (INPUT = complex_in, OUTPUT = complex_out, INTERNALLENGTH = 16);", pg("sql-createtype.html"));
  k!("INSENSITIVE", "DECLARE <c> INSENSITIVE CURSOR ... -- SQL-standard noise word in PG (every cursor sees a snapshot). Kept for compat.", "DECLARE c INSENSITIVE CURSOR FOR SELECT * FROM t;", pg("sql-declare.html"));
  k!("INT", "Signed 4-byte integer, range -2147483648..+2147483647. Alias `INTEGER` / `int4`.", "qty INT NOT NULL DEFAULT 0", pg("datatype-numeric.html#DATATYPE-INT"));
  k!("INTEGER", "Signed 4-byte integer, range -2147483648..+2147483647. Alias `INT` / `int4`.", "age INTEGER CHECK (age >= 0)", pg("datatype-numeric.html#DATATYPE-INT"));
  k!("LABEL", "SECURITY LABEL FOR <provider> ON ... IS '<label>' / ALTER TYPE ... ADD VALUE -- attach security label, or add enum label.", "ALTER TYPE mood ADD VALUE 'meh' AFTER 'ok';", pg("sql-security-label.html"));
  k!("LARGE", "CREATE/DROP CAST ... ON LARGE OBJECT, lo_* fns -- LARGE OBJECT family (bytea-style server-side blobs).", "SELECT lo_create(0);", pg("largeobjects.html"));
  k!("LEADING", "TRIM(LEADING <chars> FROM <s>) -- strip <chars> from the left.", "SELECT TRIM(LEADING '0' FROM '00042');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("LEAKPROOF", "CREATE FUNCTION ... LEAKPROOF -- promise that function reveals no info about args beyond return value. Required for use in RLS-protected views.", "CREATE FUNCTION pure_eq(a int, b int) RETURNS boolean LANGUAGE sql LEAKPROOF AS $$ SELECT a = b $$;", pg("sql-createfunction.html"));
  k!("LEAST", "Returns the smallest of the listed values (NULLs ignored unless all are NULL).", "SELECT LEAST(a, b, c) FROM t;", pg("functions-conditional.html#FUNCTIONS-GREATEST-LEAST"));
  k!("LOAD", "LOAD '<shared_library>' -- explicitly load a shared library into the backend (superuser-only for non-default paths).", "LOAD 'auto_explain';", pg("sql-load.html"));
  k!("LOCAL", "SET LOCAL <param> = <value> -- only affects the current transaction (vs SET, which lasts the session).", "BEGIN; SET LOCAL statement_timeout = '5s'; ...; COMMIT;", pg("sql-set.html"));
  k!("LOCALTIME", "Returns the current time-without-time-zone in the session zone.", "SELECT LOCALTIME;", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  k!("LOCALTIMESTAMP", "Returns the current timestamp-without-time-zone (transaction start).", "SELECT LOCALTIMESTAMP;", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  k!("LOCATION", "CREATE TABLESPACE <ts> LOCATION '<path>' / CREATE DATABASE ... TABLESPACE -- filesystem directory where data lives.", "CREATE TABLESPACE fast LOCATION '/mnt/nvme/pg';", pg("sql-createtablespace.html"));
  k!("LOGGED", "ALTER TABLE ... SET { LOGGED | UNLOGGED } -- switch between WAL-logged (crash-safe) and unlogged (faster, lost on crash).", "ALTER TABLE staging SET LOGGED;", pg("sql-altertable.html"));
  k!("MAPPING", "CREATE/ALTER/DROP USER MAPPING FOR <role> SERVER <srv> -- per-user credentials for a foreign server.", "CREATE USER MAPPING FOR alice SERVER remote OPTIONS (user 'alice', password 's');", pg("sql-createusermapping.html"));
  k!("MATCH", "FOREIGN KEY ... MATCH { FULL | PARTIAL | SIMPLE } -- how composite FK handles NULLs. SIMPLE (default) lets any NULL pass.", "FOREIGN KEY (a,b) REFERENCES p(a,b) MATCH FULL", pg("sql-createtable.html#SQL-CREATETABLE-PARMS-REFERENCES"));
  k!("MAXVALUE", "CREATE SEQUENCE ... MAXVALUE <n> | NO MAXVALUE -- ceiling for ascending sequences.", "CREATE SEQUENCE s MAXVALUE 100000;", pg("sql-createsequence.html"));
  k!("METHOD", "CREATE INDEX ... USING <method> / ALTER TABLE ... SET ACCESS METHOD <am> -- pick index/table access method (btree, hash, gin, gist, brin, heap, ...).", "ALTER TABLE big SET ACCESS METHOD heap;", pg("sql-altertable.html"));
  k!("MINUTE", "EXTRACT(MINUTE FROM <ts|interval>) -- minute 0..59; interval minute part.", "SELECT EXTRACT(MINUTE FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("MINVALUE", "CREATE SEQUENCE ... MINVALUE <n> | NO MINVALUE -- floor for descending sequences.", "CREATE SEQUENCE s INCREMENT -1 MINVALUE 0 START WITH 100;", pg("sql-createsequence.html"));
  k!("MONTH", "EXTRACT(MONTH FROM <ts|interval>) -- 1..12 for timestamps; remaining months in interval after years.", "SELECT EXTRACT(MONTH FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("MOVE", "MOVE <direction> <n> FROM <cursor> -- reposition cursor without returning rows.", "MOVE FORWARD 100 FROM c;", pg("sql-move.html"));
  k!("NAME", "Internal PG type for identifiers (NAMEDATALEN-1 bytes, default 63). Used by system catalogs.", "SELECT relname::name FROM pg_class LIMIT 5;", pg("datatype-character.html"));
  k!("NAMES", "SET NAMES '<charset>' -- client encoding alias. PG-compat form of `SET client_encoding`.", "SET NAMES 'UTF8';", pg("multibyte.html"));
  k!("NATIONAL", "`NATIONAL CHARACTER [VARYING](n)` -- SQL-standard alias of CHAR(n)/VARCHAR(n). PG treats national chars same as CHAR.", "name NATIONAL CHARACTER VARYING(64)", pg("datatype-character.html"));
  k!("NCHAR", "Alias of CHAR. PG-compat for SQL-standard `NATIONAL CHARACTER`.", "code NCHAR(3)", pg("datatype-character.html"));
  k!("NFC", "Unicode Normalization Form C (composed). Argument to `normalize()`/IS NORMALIZED.", "SELECT normalize('café', NFC);", pg("functions-string.html#FUNCTIONS-STRING-NORMALIZATION"));
  k!("NFD", "Unicode Normalization Form D (decomposed).", "SELECT normalize('café', NFD);", pg("functions-string.html#FUNCTIONS-STRING-NORMALIZATION"));
  k!("NFKC", "Unicode Normalization Form KC (compatibility composed).", "SELECT normalize('fi', NFKC);", pg("functions-string.html#FUNCTIONS-STRING-NORMALIZATION"));
  k!("NFKD", "Unicode Normalization Form KD (compatibility decomposed).", "SELECT normalize('fi', NFKD);", pg("functions-string.html#FUNCTIONS-STRING-NORMALIZATION"));
  k!("NONE", "Placeholder type for `CREATE OPERATOR` when a side has no argument (prefix/postfix op).", "CREATE OPERATOR -@ (RIGHTARG = numeric, FUNCTION = neg);", pg("sql-createoperator.html"));
  k!("NORMALIZE", "normalize(<text> [, NFC|NFD|NFKC|NFKD]) -- Unicode normalization (PG13+). Default NFC.", "SELECT normalize('café', NFC);", pg("functions-string.html#FUNCTIONS-STRING-NORMALIZATION"));
  k!("NORMALIZED", "<text> IS [NOT] [<form>] NORMALIZED -- predicate testing Unicode normalization (PG13+).", "SELECT 'café' IS NORMALIZED NFC;", pg("functions-string.html#FUNCTIONS-STRING-NORMALIZATION"));
  k!("NOTNULL", "PG-specific shortcut: `<expr> NOTNULL` -- equivalent to `<expr> IS NOT NULL`. Compare with ISNULL.", "WHERE deleted_at NOTNULL", pg("functions-comparison.html"));
  k!("NULLIF", "Returns NULL when arg1 = arg2, otherwise arg1 -- inverse of COALESCE.", "SELECT NULLIF(score, 0) FROM rows;", pg("functions-conditional.html#FUNCTIONS-NULLIF"));
  k!("NUMERIC", "Exact arbitrary-precision number; safe for money. `NUMERIC(p,s)` -- p total digits, s after decimal point.", "balance NUMERIC(15,4) NOT NULL DEFAULT 0", pg("datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL"));
  k!("OBJECT", "SECURITY LABEL FOR <provider> ON <object_type> <name> IS '<label>' -- attach a security label to a database object.", "SECURITY LABEL FOR selinux ON TABLE t IS 'system_u:object_r:sepgsql_table_t:s0';", pg("sql-security-label.html"));
  k!("OFF", "GUC boolean false. `SET <param> = off`. Equivalent to `false` / `0` / `no`.", "SET enable_seqscan = off;", pg("config-setting.html"));
  k!("OIDS", "Legacy CREATE TABLE ... WITH OIDS -- removed in PG12. Only kept as a parse-stage relic.", "-- removed: every row had a system OID column", pg("sql-createtable.html"));
  k!("OPERATOR", "CREATE/ALTER/DROP OPERATOR -- user-defined infix/prefix operator backed by a function.", "CREATE OPERATOR === (LEFTARG = int, RIGHTARG = int, FUNCTION = my_eq);", pg("sql-createoperator.html"));
  k!("OPTION", "GRANT ... WITH GRANT/ADMIN OPTION / CREATE VIEW ... WITH CHECK OPTION -- grant-forwarding or update-check predicate.", "GRANT SELECT ON t TO svc WITH GRANT OPTION;", pg("sql-grant.html"));
  k!("OPTIONS", "ALTER FOREIGN TABLE / SERVER / USER MAPPING ... OPTIONS (ADD|SET|DROP <key> '<val>', ...) -- FDW-specific key/value bag.", "ALTER FOREIGN TABLE r OPTIONS (SET schema_name 'public', SET table_name 'remote_t');", pg("sql-alterforeigntable.html"));
  k!("ORDINALITY", "FROM <set_returning_fn>(...) WITH ORDINALITY -- append a `ordinality` bigint column numbering output rows 1..N.", "SELECT * FROM unnest(ARRAY['a','b','c']) WITH ORDINALITY AS u(v, idx);", pg("queries-table-expressions.html#QUERIES-TABLEFUNCTIONS"));
  k!("OTHERS", "WINDOW frame exclusion: `EXCLUDE NO OTHERS` (default) -- keep every row in the frame.", "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE NO OTHERS", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("OUT", "Function/procedure parameter mode: OUT -- returned to caller, not provided by call site.", "CREATE FUNCTION counts(OUT live int, OUT dead int) AS $$ ... $$ LANGUAGE plpgsql;", pg("sql-createfunction.html"));
  k!("OVERLAY", "OVERLAY(<s> PLACING <r> FROM <pos> [FOR <len>]) -- substring replacement starting at 1-based <pos>.", "SELECT OVERLAY('Postgres' PLACING 'SQL' FROM 5);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("OVERRIDING", "INSERT INTO <t> OVERRIDING { SYSTEM | USER } VALUE -- bypass `GENERATED ALWAYS` (system) or use a system value over a USER-generated default.", "INSERT INTO t (id, name) OVERRIDING SYSTEM VALUE VALUES (42, 'a');", pg("sql-insert.html"));
  k!("PARALLEL", "CREATE FUNCTION ... PARALLEL { UNSAFE | RESTRICTED | SAFE } -- declares safety in parallel workers. Default UNSAFE.", "CREATE FUNCTION add(int,int) RETURNS int LANGUAGE sql PARALLEL SAFE AS $$ SELECT $1 + $2 $$;", pg("sql-createfunction.html"));
  k!("PARAMETER", "Reserved word for function parameter declarations. Appears in CREATE FUNCTION ... PARAMETER STYLE -- mostly SQL-standard noise in PG.", "-- reserved by spec", pg("appendix-keywords.html"));
  k!("PARSER", "CREATE/ALTER/DROP TEXT SEARCH PARSER -- tokenizer used by FTS configs.", "CREATE TEXT SEARCH PARSER my_parser (START = ..., GETTOKEN = ..., END = ..., HEADLINE = ..., LEXTYPES = ...);", pg("sql-createtsparser.html"));
  k!("PARTIAL", "FOREIGN KEY ... MATCH PARTIAL -- composite FK; some columns NULL, others present. Reserved in PG -- not implemented.", "FOREIGN KEY (a,b) REFERENCES p(a,b) MATCH PARTIAL", pg("sql-createtable.html"));
  k!("PASSING", "XMLTABLE / JSON_TABLE ... PASSING <expr> [AS <name>] -- bind values usable inside the XPath / JSONPath expression.", "SELECT * FROM XMLTABLE('/r' PASSING x COLUMNS id INT PATH '@id') AS t;", pg("functions-xml.html#FUNCTIONS-XML-PROCESSING-XMLTABLE"));
  k!("PLACING", "Part of `OVERLAY(<s> PLACING <r> FROM <pos> ...)` substring-replacement syntax.", "SELECT OVERLAY('Postgres' PLACING 'SQL' FROM 5);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("PLANS", "DISCARD PLANS -- evict every cached prepared-statement plan in the current session.", "DISCARD PLANS;", pg("sql-discard.html"));
  k!("POLICY", "CREATE/ALTER/DROP POLICY <name> ON <table> -- Row-Level Security predicate scoped to a role and DML class.", "CREATE POLICY tenant_isolation ON orders FOR ALL TO app USING (tenant_id = current_setting('app.tenant')::int);", pg("sql-createpolicy.html"));
  k!("POSITION", "POSITION(<needle> IN <haystack>) -- 1-based index, 0 when not found. SQL-standard form of `strpos()`.", "SELECT POSITION('@' IN email) FROM users;", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  k!("PRECISION", "Part of `DOUBLE PRECISION` (8-byte IEEE-754) and `TIMESTAMP(p) WITH TIME ZONE` (fractional-second digits).", "ratio DOUBLE PRECISION", pg("datatype-numeric.html#DATATYPE-FLOAT"));
  k!("PREPARED", "COMMIT/ROLLBACK PREPARED '<gxid>' -- second phase of a two-phase commit (after `PREPARE TRANSACTION '<gxid>'`).", "COMMIT PREPARED 'xact_42';", pg("sql-commit-prepared.html"));
  k!("PRESERVE", "CREATE TEMPORARY TABLE ... ON COMMIT PRESERVE ROWS -- default; keep rows across COMMIT (vs DELETE ROWS / DROP).", "CREATE TEMP TABLE scratch (id int) ON COMMIT PRESERVE ROWS;", pg("sql-createtable.html"));
  k!("PRIOR", "FETCH PRIOR FROM <cursor> -- step one row back. Requires SCROLL cursor.", "FETCH PRIOR FROM c;", pg("sql-fetch.html"));
  k!("PROCEDURAL", "Legacy `CREATE [PROCEDURAL] LANGUAGE` -- equivalent to plain `CREATE LANGUAGE`. Kept for compat.", "CREATE PROCEDURAL LANGUAGE plpgsql;", pg("sql-createlanguage.html"));
  k!("PROCEDURES", "GRANT/REVOKE ... ON ALL PROCEDURES IN SCHEMA / ALTER DEFAULT PRIVILEGES ... ON PROCEDURES -- bulk-privilege ops on procedures.", "GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("PUBLICATION", "CREATE/ALTER/DROP PUBLICATION -- logical-replication source: set of tables whose DML is shipped to subscribers.", "CREATE PUBLICATION pub_orders FOR TABLE orders, order_lines;", pg("sql-createpublication.html"));
  k!("READ", "SET TRANSACTION READ { ONLY | WRITE } -- forbid/allow writes for the current xact.", "BEGIN READ ONLY;", pg("sql-set-transaction.html"));
  k!("REAL", "Inexact 4-byte IEEE-754 float. Alias `float4`. ~6 decimal digits of precision.", "ratio REAL NOT NULL DEFAULT 0", pg("datatype-numeric.html#DATATYPE-FLOAT"));
  k!("REASSIGN", "REASSIGN OWNED BY <old_role> TO <new_role> -- transfer ownership of every object in current DB.", "REASSIGN OWNED BY leaving_user TO new_owner;", pg("sql-reassign-owned.html"));
  k!("RECHECK", "Legacy CREATE OPERATOR CLASS ... RECHECK -- told planner the index gave approximate match. Removed in PG8.4; AM controls it now.", "-- removed in modern PG", pg("sql-createopclass.html"));
  k!("REF", "Reserved word for SQL-standard reference types. Not implemented in PG; reserved for parse-stage compat.", "-- reserved", pg("appendix-keywords.html"));
  k!("REFERENCING", "CREATE TRIGGER ... REFERENCING { OLD | NEW } TABLE AS <name> -- expose transition tables to a statement-level trigger.", "CREATE TRIGGER trg AFTER UPDATE ON t REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION audit();", pg("sql-createtrigger.html"));
  k!("RELATIVE", "FETCH RELATIVE <n> FROM <cursor> -- skip <n> rows (negative = backwards).", "FETCH RELATIVE -5 FROM c;", pg("sql-fetch.html"));
  k!("RELEASE", "RELEASE SAVEPOINT <name> -- discard a savepoint and merge its work into the parent. Cannot be undone after RELEASE.", "RELEASE SAVEPOINT sp1;", pg("sql-release-savepoint.html"));
  k!("REPEATABLE", "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ -- snapshot taken at xact start; no non-repeatable reads. PG implements via Snapshot Isolation.", "BEGIN ISOLATION LEVEL REPEATABLE READ;", pg("transaction-iso.html#XACT-REPEATABLE-READ"));
  k!("RESET", "RESET <param> | RESET ALL -- restore a GUC to its default (postgresql.conf / startup) value.", "RESET statement_timeout;", pg("sql-reset.html"));
  k!("RESTART", "ALTER SEQUENCE ... RESTART [WITH <n>] -- reset sequence counter; `TRUNCATE ... RESTART IDENTITY` also valid.", "ALTER SEQUENCE s RESTART WITH 1;", pg("sql-altersequence.html"));
  k!("ROUTINE", "ALTER ROUTINE / DROP ROUTINE -- umbrella DDL covering functions AND procedures (PG11+).", "ALTER ROUTINE f(int) RENAME TO g;", pg("sql-alterroutine.html"));
  k!("ROUTINES", "GRANT/REVOKE ... ON ALL ROUTINES IN SCHEMA / ALTER DEFAULT PRIVILEGES ... ON ROUTINES -- bulk-privilege ops on fns + procs.", "GRANT EXECUTE ON ALL ROUTINES IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("SCHEMAS", "ALTER DEFAULT PRIVILEGES ... IN SCHEMAS <s>[, ...] -- scope default ACL changes to specific schemas.", "ALTER DEFAULT PRIVILEGES IN SCHEMAS app, billing GRANT SELECT ON TABLES TO readonly;", pg("sql-alterdefaultprivileges.html"));
  k!("SCROLL", "DECLARE <c> [SCROLL|NO SCROLL] CURSOR ... -- SCROLL cursor supports FETCH BACKWARD/ABSOLUTE/RELATIVE. NO SCROLL forbids them.", "DECLARE c SCROLL CURSOR FOR SELECT id FROM t;", pg("sql-declare.html"));
  k!("SEARCH", "WITH RECURSIVE ... SEARCH { DEPTH | BREADTH } FIRST BY <cols> SET <out> -- annotate recursive CTE output rows with traversal order (SQL:1999 / PG14+).", "WITH RECURSIVE t AS (...) SEARCH DEPTH FIRST BY id SET ord SELECT * FROM t ORDER BY ord;", pg("queries-with.html#QUERIES-WITH-SEARCH"));
  k!("SECOND", "EXTRACT(SECOND FROM <ts|interval>) -- seconds with fractional part (0..59.999999).", "SELECT EXTRACT(SECOND FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("SEQUENCES", "GRANT/REVOKE ... ON ALL SEQUENCES IN SCHEMA / ALTER DEFAULT PRIVILEGES ... ON SEQUENCES / DISCARD SEQUENCES -- bulk-privilege/state ops on sequences.", "DISCARD SEQUENCES;", pg("sql-grant.html"));
  k!("SERIALIZABLE", "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE -- strongest isolation; PG uses Serializable Snapshot Isolation (SSI) which can throw `40001 serialization_failure` -- retry from the client.", "BEGIN ISOLATION LEVEL SERIALIZABLE;", pg("transaction-iso.html#XACT-SERIALIZABLE"));
  k!("SERVER", "CREATE/ALTER/DROP SERVER -- foreign server, an FDW endpoint targetable by foreign tables / user mappings.", "CREATE SERVER remote FOREIGN DATA WRAPPER postgres_fdw OPTIONS (host 'db', port '5432', dbname 'app');", pg("sql-createserver.html"));
  k!("SESSION", "SET SESSION <param> = <val> -- session-scoped setting (default; vs `SET LOCAL` xact-scope). Also `SET SESSION AUTHORIZATION <role>` and `SET SESSION CHARACTERISTICS`.", "SET SESSION timezone TO 'UTC';", pg("sql-set.html"));
  k!("SESSION_USER", "Returns the session user name (login role, unaffected by SET ROLE).", "SELECT SESSION_USER;", pg("functions-info.html"));
  k!("SYSTEM_USER", "Returns the auth-method-prefixed external identity used to authenticate the session (PG16+, e.g. `scram-sha-256:alice`).", "SELECT SYSTEM_USER;", pg("functions-info.html"));
  k!("SETOF", "Function return type: SETOF <type> -- returns a row set, callable in FROM as a table.", "CREATE FUNCTION ids() RETURNS SETOF int LANGUAGE sql AS $$ SELECT generate_series(1,3) $$;", pg("sql-createfunction.html"));
  k!("SETS", "GROUPING SETS ((a), (b), (a,b)) -- list of grouping subsets evaluated in one pass; complements ROLLUP/CUBE.", "SELECT a, b, sum(x) FROM t GROUP BY GROUPING SETS ((a), (b), ());", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  k!("SIMPLE", "FOREIGN KEY ... MATCH SIMPLE -- default; FK passes if any column in the multi-column key is NULL.", "FOREIGN KEY (a,b) REFERENCES p(a,b) MATCH SIMPLE", pg("sql-createtable.html#SQL-CREATETABLE-PARMS-REFERENCES"));
  k!("SMALLINT", "Signed 2-byte integer, range -32768..+32767. Alias `int2`.", "year SMALLINT CHECK (year BETWEEN 1900 AND 2100)", pg("datatype-numeric.html#DATATYPE-INT"));
  k!("SNAPSHOT", "SET TRANSACTION SNAPSHOT '<id>' -- import an existing snapshot (from pg_export_snapshot()); used for consistent parallel dumps.", "BEGIN; SET TRANSACTION SNAPSHOT '00000004-00000004-1';", pg("sql-set-transaction.html"));
  k!("SQL", "CREATE FUNCTION ... LANGUAGE sql -- pure-SQL body. PG14+ supports atomic standard form `BEGIN ATOMIC ... END;`.", "CREATE FUNCTION add(int,int) RETURNS int LANGUAGE sql AS $$ SELECT $1+$2 $$;", pg("xfunc-sql.html"));
  k!("STANDALONE", "xmlroot(..., STANDALONE { YES | NO | NO VALUE }) -- toggle the `standalone` attribute on the XML prolog.", "SELECT xmlroot(x, VERSION '1.0', STANDALONE YES);", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("START", "Two uses: `START TRANSACTION` (synonym of BEGIN), and `CREATE SEQUENCE ... START [WITH] <n>` (initial value).", "START TRANSACTION ISOLATION LEVEL SERIALIZABLE;", pg("sql-start-transaction.html"));
  k!("STATISTICS", "ALTER TABLE ... ALTER COLUMN <c> SET STATISTICS <n> / CREATE STATISTICS -- per-column ANALYZE target, or multivariate stats object.", "ALTER TABLE t ALTER COLUMN tags SET STATISTICS 500;", pg("sql-altertable.html"));
  k!("STDIN", "COPY <t> FROM STDIN -- stream rows from client connection. Terminate with `\\.` in psql.", "COPY t FROM STDIN WITH (FORMAT csv);", pg("sql-copy.html"));
  k!("STDOUT", "COPY <t> TO STDOUT -- stream rows back to client connection (no server-side file needed).", "COPY (SELECT * FROM t) TO STDOUT WITH (FORMAT csv);", pg("sql-copy.html"));
  k!("STORAGE", "ALTER TABLE ... ALTER COLUMN <c> SET STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN } -- TOAST strategy for variable-length values.", "ALTER TABLE t ALTER COLUMN body SET STORAGE EXTENDED;", pg("sql-altertable.html"));
  k!("STRIP", "xmlserialize(... STRIP WHITESPACE) -- collapse insignificant whitespace in the serialized output.", "SELECT xmlserialize(DOCUMENT x AS text STRIP WHITESPACE);", pg("functions-xml.html"));
  k!("SUBSCRIPTION", "CREATE/ALTER/DROP SUBSCRIPTION -- logical-replication sink that reads from a publisher's PUBLICATION.", "CREATE SUBSCRIPTION sub_orders CONNECTION 'host=pub user=repl dbname=app' PUBLICATION pub_orders;", pg("sql-createsubscription.html"));
  k!("SUBSTRING", "SUBSTRING(<s> FROM <pos> [FOR <len>]) | SUBSTRING(<s> FROM <pattern>) -- SQL-standard substring or POSIX-regex extract.", "SELECT SUBSTRING('Postgres' FROM 5 FOR 3), SUBSTRING('abc123', '[0-9]+');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  k!("SUPPORT", "CREATE FUNCTION ... SUPPORT <fn> -- planner support function: gives row estimates / index info to the planner (PG12+, C only).", "CREATE FUNCTION my_fn(int) RETURNS int LANGUAGE c SUPPORT my_fn_support AS 'libfn', 'my_fn';", pg("sql-createfunction.html"));
  k!("SYMMETRIC", "BETWEEN SYMMETRIC <a> AND <b> -- swap operands if a > b, so the test always picks min..max.", "WHERE x BETWEEN SYMMETRIC 10 AND 1", pg("functions-comparison.html"));
  k!("SYSID", "Legacy CREATE/ALTER USER ... SYSID <n> -- set role OID. Removed long ago; PG ignores it. Kept for parse compat.", "-- removed", pg("sql-createrole.html"));
  k!("SYSTEM", "ALTER SYSTEM SET <param> = <val> -- writes to postgresql.auto.conf; takes effect after `SELECT pg_reload_conf()` (or restart for some params).", "ALTER SYSTEM SET shared_buffers = '4GB';", pg("sql-altersystem.html"));
  k!("TABLES", "GRANT/REVOKE ... ON ALL TABLES IN SCHEMA / ALTER DEFAULT PRIVILEGES ... ON TABLES -- bulk privilege ops on tables (also covers views).", "GRANT SELECT ON ALL TABLES IN SCHEMA app TO readonly;", pg("sql-grant.html"));
  k!("TABLESAMPLE", "FROM <t> TABLESAMPLE <method> (<pct>) -- statistical sampling. Built-ins: BERNOULLI (per-row coin flip), SYSTEM (block-level, cheaper).", "SELECT * FROM big TABLESAMPLE BERNOULLI (1);", pg("sql-select.html#SQL-FROM"));
  k!("TEMPLATE", "CREATE DATABASE <new> TEMPLATE <existing> -- clone an existing DB. Also `CREATE TEXT SEARCH TEMPLATE` for FTS.", "CREATE DATABASE staging TEMPLATE template_app;", pg("sql-createdatabase.html"));
  k!("TEXT", "Variable-length string of unlimited length. Same performance as `VARCHAR` -- prefer TEXT.", "body TEXT NOT NULL", pg("datatype-character.html"));
  k!("TIES", "FETCH FIRST <n> ROWS WITH TIES -- include peer rows tied with row <n> per the ORDER BY (SQL:2008 / PG13+).", "SELECT * FROM scores ORDER BY pts DESC FETCH FIRST 3 ROWS WITH TIES;", pg("sql-select.html#SQL-LIMIT"));
  k!("TIME", "Time-of-day data type. `TIME [(p)]` (no zone) or `TIME [(p)] WITH TIME ZONE` (timetz). Almost always prefer `timestamptz` over `timetz`.", "start_at TIME(3)", pg("datatype-datetime.html"));
  k!("TIMESTAMP", "Datetime data type. `TIMESTAMP [(p)]` (timestamp) or `TIMESTAMP [(p)] WITH TIME ZONE` (timestamptz). Prefer timestamptz; PG stores both as UTC.", "created_at TIMESTAMP(3) WITH TIME ZONE NOT NULL DEFAULT now()", pg("datatype-datetime.html"));
  k!("TRAILING", "TRIM(TRAILING <chars> FROM <s>) -- strip <chars> from the right.", "SELECT TRIM(TRAILING '/' FROM '/a/b/');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("TRANSFORM", "CREATE TRANSFORM FOR <type> LANGUAGE <pl> (FROM SQL WITH FUNCTION ..., TO SQL WITH FUNCTION ...) -- PL-side codec for a PG type (e.g. hstore <-> Perl hash).", "CREATE TRANSFORM FOR hstore LANGUAGE plperl (FROM SQL WITH FUNCTION hstore_to_plperl(internal), TO SQL WITH FUNCTION plperl_to_hstore(internal));", pg("sql-createtransform.html"));
  k!("TREAT", "TREAT(<expr> AS <type>) -- SQL-standard subtype assertion. Reserved in PG; no current effect (treated as cast).", "SELECT TREAT(x AS text);", pg("appendix-keywords.html"));
  k!("TRIM", "TRIM([LEADING|TRAILING|BOTH] [<chars>] FROM <s>) -- strip <chars> (default ' ') from one or both ends.", "SELECT TRIM(BOTH '\"' FROM '\"hello\"');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  k!("TRUSTED", "CREATE [TRUSTED] LANGUAGE / EXTENSION -- TRUSTED PL can be created by ordinary users; trusted extensions installable by table owners (PG13+).", "CREATE TRUSTED LANGUAGE plperl;", pg("sql-createlanguage.html"));
  k!("TYPES", "ALTER DEFAULT PRIVILEGES ... ON TYPES -- ALTER DEFAULT PRIVILEGES target covering CREATE/USAGE on types.", "ALTER DEFAULT PRIVILEGES IN SCHEMA app GRANT USAGE ON TYPES TO svc;", pg("sql-alterdefaultprivileges.html"));
  k!("UESCAPE", "U&'<text>' UESCAPE '<c>' -- Unicode string literal with custom backslash. Same for identifiers: U&\"...\" UESCAPE '<c>'.", "SELECT U&'d\\0061t\\0061' UESCAPE '\\\\';", pg("sql-syntax-lexical.html#SQL-SYNTAX-STRINGS-UESCAPE"));
  k!("UNCOMMITTED", "SET TRANSACTION ISOLATION LEVEL READ UNCOMMITTED -- accepted for compat; PG silently upgrades to READ COMMITTED (no dirty reads).", "BEGIN ISOLATION LEVEL READ UNCOMMITTED;", pg("transaction-iso.html"));
  k!("UNENCRYPTED", "Legacy `CREATE/ALTER ROLE ... UNENCRYPTED PASSWORD '<pw>'` -- PG10+ rejects this; passwords are always hashed.", "-- removed; use ENCRYPTED (or omit -- it's the default)", pg("sql-createrole.html"));
  k!("UNKNOWN", "Three-valued-logic third value (NULL in boolean context). `<bool> IS UNKNOWN` true when expression is NULL.", "WHERE active IS UNKNOWN", pg("functions-comparison.html"));
  k!("UNTIL", "CREATE/ALTER ROLE ... VALID UNTIL '<ts>' -- role password expiry; after that, login is refused.", "ALTER ROLE alice VALID UNTIL '2027-01-01';", pg("sql-createrole.html"));
  k!("USAGE", "GRANT USAGE ON { SCHEMA | SEQUENCE | DOMAIN | FDW | SERVER | TYPE | LANGUAGE } -- needed in addition to per-object privileges.", "GRANT USAGE ON SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("VALID", "CREATE/ALTER ROLE ... VALID UNTIL '<ts>' / Constraints become VALID after `ALTER TABLE ... VALIDATE CONSTRAINT`.", "ALTER ROLE alice VALID UNTIL 'infinity';", pg("sql-createrole.html"));
  k!("VALIDATE", "ALTER TABLE ... VALIDATE CONSTRAINT <name> -- re-check existing rows against a constraint added with NOT VALID; converts it to VALID.", "ALTER TABLE t VALIDATE CONSTRAINT fk_t_user;", pg("sql-altertable.html"));
  k!("VALIDATOR", "CREATE LANGUAGE ... VALIDATOR <fn> / CREATE FOREIGN DATA WRAPPER ... VALIDATOR <fn> -- C function that validates options at create time.", "CREATE FOREIGN DATA WRAPPER w VALIDATOR my_fdw_validator;", pg("sql-createforeigndatawrapper.html"));
  k!("VALUE", "ALTER TYPE <enum> ADD VALUE [IF NOT EXISTS] '<lab>' [BEFORE|AFTER '<other>'] -- append/insert enum label.", "ALTER TYPE mood ADD VALUE IF NOT EXISTS 'meh' AFTER 'ok';", pg("sql-altertype.html"));
  k!("VARCHAR", "Variable-length string with optional limit. `VARCHAR(n)` = `CHARACTER VARYING(n)`. Functionally identical to TEXT -- prefer TEXT + CHECK if you need a length cap.", "name VARCHAR(64) NOT NULL", pg("datatype-character.html"));
  k!("VARIADIC", "Function: last parameter `VARIADIC <type>[]` accepts a variable number of args; call site can use `VARIADIC <arr>` to pass an array as that vararg.", "CREATE FUNCTION fmt(VARIADIC vals text[]) RETURNS text LANGUAGE sql AS $$ SELECT array_to_string(vals, ',') $$;", pg("xfunc-sql.html#XFUNC-SQL-VARIADIC-FUNCTIONS"));
  k!("VARYING", "Modifier for variable-length strings/bits. `CHARACTER VARYING(n)` = `VARCHAR(n)`; `BIT VARYING(n)` = `VARBIT(n)`.", "name CHARACTER VARYING(64)", pg("datatype-character.html"));
  k!("VERSION", "xmlroot(<x>, VERSION '<v>') -- set/replace XML prolog version. Also `CREATE EXTENSION ... VERSION '<v>'` and `ALTER EXTENSION ... UPDATE TO '<v>'`.", "SELECT xmlroot(x, VERSION '1.0', STANDALONE NO);", pg("functions-xml.html"));
  k!("VIEWS", "ALTER DEFAULT PRIVILEGES ... ON VIEWS -- bulk-default privileges for views (treated separately from tables since PG14).", "ALTER DEFAULT PRIVILEGES IN SCHEMA app GRANT SELECT ON TABLES TO readonly;", pg("sql-alterdefaultprivileges.html"));
  k!("WHITESPACE", "xmlserialize(... [PRESERVE | STRIP] WHITESPACE) -- whether the serializer keeps insignificant whitespace (PG16+).", "SELECT xmlserialize(DOCUMENT x AS text PRESERVE WHITESPACE);", pg("functions-xml.html"));
  k!("WITHOUT", "Part of `TIMESTAMP/TIME WITHOUT TIME ZONE` and `WITHOUT OVERLAPS` (period FKs, PG17+).", "ts TIMESTAMP WITHOUT TIME ZONE", pg("datatype-datetime.html"));
  k!("WORK", "Noise word in `COMMIT [WORK]` / `ROLLBACK [WORK]` / `BEGIN [WORK]` -- SQL-standard, no effect.", "COMMIT WORK;", pg("sql-commit.html"));
  k!("WRAPPER", "CREATE/ALTER/DROP FOREIGN DATA WRAPPER -- FDW handler that knows how to talk to remote data sources.", "CREATE FOREIGN DATA WRAPPER postgres_fdw HANDLER postgres_fdw_handler VALIDATOR postgres_fdw_validator;", pg("sql-createforeigndatawrapper.html"));
  k!("WRITE", "SET TRANSACTION READ WRITE -- default; allow writes for the current xact. Inverse of READ ONLY.", "BEGIN READ WRITE;", pg("sql-set-transaction.html"));
  k!("XML", "PG XML data type. Stores well-formed XML documents or content fragments; query with `XMLEXISTS`, `XMLTABLE`, `xpath()`.", "doc XML NOT NULL CHECK (xpath_exists('/root', doc))", pg("datatype-xml.html"));
  k!("XMLATTRIBUTES", "Part of `XMLELEMENT(NAME tag, XMLATTRIBUTES(<expr> AS <name>, ...), <content>)` -- build element attribute list.", "SELECT XMLELEMENT(NAME a, XMLATTRIBUTES(id, ts AS created), body);", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLCONCAT", "XMLCONCAT(<x1>, <x2>, ...) -- concatenate XML fragments into a single forest.", "SELECT XMLCONCAT('<a/>'::xml, '<b/>'::xml);", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLELEMENT", "XMLELEMENT(NAME <tag>[, XMLATTRIBUTES(...)][, <content>...]) -- construct an XML element.", "SELECT XMLELEMENT(NAME user, XMLATTRIBUTES(id), name) FROM users;", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLEXISTS", "XMLEXISTS(<xpath> PASSING <x>) -- boolean: does the XPath match anything?", "SELECT * FROM docs WHERE XMLEXISTS('//author' PASSING doc);", pg("functions-xml.html#FUNCTIONS-XML-PREDICATES"));
  k!("XMLFOREST", "XMLFOREST(<expr> AS <name>, ...) -- build a forest (sibling elements) one per expression.", "SELECT XMLFOREST(id, name AS user_name) FROM users;", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLNAMESPACES", "XMLTABLE(XMLNAMESPACES('uri' AS <prefix>, ...), <xpath> PASSING <x> COLUMNS ...) -- bind XPath namespaces inside XMLTABLE.", "SELECT * FROM XMLTABLE(XMLNAMESPACES('http://x' AS x), '/x:root/x:item' PASSING d COLUMNS id INT PATH '@id');", pg("functions-xml.html#FUNCTIONS-XML-PROCESSING-XMLTABLE"));
  k!("XMLPARSE", "XMLPARSE({DOCUMENT|CONTENT} <text>) -- parse text into xml. DOCUMENT requires a complete document; CONTENT accepts fragments.", "SELECT XMLPARSE(CONTENT '<a/>');", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLPI", "XMLPI(NAME <target>[, <content>]) -- XML processing instruction.", "SELECT XMLPI(NAME php, 'echo 1;');", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLROOT", "XMLROOT(<x>, VERSION '<v>' | NO VALUE[, STANDALONE {YES|NO|NO VALUE}]) -- set/replace the XML prolog.", "SELECT XMLROOT(x, VERSION '1.0', STANDALONE NO);", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLSERIALIZE", "XMLSERIALIZE({DOCUMENT|CONTENT} <x> AS <type> [INDENT] [{PRESERVE|STRIP} WHITESPACE]) -- serialize xml to text/bytea.", "SELECT XMLSERIALIZE(DOCUMENT x AS text INDENT);", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLTABLE", "XMLTABLE([XMLNAMESPACES(...),] <row_xpath> PASSING <x> COLUMNS <col_specs>) -- shred XML into a relational result set.", "SELECT * FROM XMLTABLE('/r/item' PASSING d COLUMNS id INT PATH '@id', name TEXT PATH 'name');", pg("functions-xml.html#FUNCTIONS-XML-PROCESSING-XMLTABLE"));
  k!("YEAR", "EXTRACT(YEAR FROM <ts|interval>) -- AD year for timestamps; year portion of interval.", "SELECT EXTRACT(YEAR FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("YES", "xmlroot(..., STANDALONE YES) -- mark XML document as standalone.", "SELECT xmlroot(x, VERSION '1.0', STANDALONE YES);", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("ZONE", "Part of `[WITH|WITHOUT] TIME ZONE` (datetime types) and `AT TIME ZONE` (conversion expression).", "ts TIMESTAMP WITH TIME ZONE; SELECT ts AT TIME ZONE 'UTC' FROM e;", pg("datatype-datetime.html"));

  // ---- PG 16+ / SQL-2023 additions (round 147) ----
  k!("JSON_TABLE", "JSON_TABLE(<doc>, <jsonpath> COLUMNS (...)) -- relational projection of JSON (PG17+).", "SELECT * FROM JSON_TABLE(j, '$.items[*]' COLUMNS (id int PATH '$.id'));", pg("functions-json.html"));
  k!("JSON_VALUE", "JSON_VALUE(<doc>, <jsonpath> RETURNING <type>) -- scalar extraction.", "SELECT JSON_VALUE(payload, '$.user.id' RETURNING int);", pg("functions-json.html"));
  k!("JSON_QUERY", "JSON_QUERY(<doc>, <jsonpath>) -- returns jsonb fragment.", "SELECT JSON_QUERY(payload, '$.items');", pg("functions-json.html"));
  k!("JSON_EXISTS", "JSON_EXISTS(<doc>, <jsonpath>) -- boolean test.", "WHERE JSON_EXISTS(payload, '$.flag')", pg("functions-json.html"));
  k!("JSON_OBJECT", "JSON_OBJECT(<key>: <value>[, ...]) -- SQL-standard constructor.", "SELECT JSON_OBJECT('id': id, 'name': name) FROM t;", pg("functions-json.html"));
  k!("JSON_OBJECTAGG", "JSON_OBJECTAGG(<k>: <v>) -- aggregate JSON object constructor.", "SELECT JSON_OBJECTAGG(id: name) FROM users;", pg("functions-json.html"));
  k!("JSON_ARRAY", "JSON_ARRAY(<v>[, ...]) -- SQL-standard JSON array constructor.", "SELECT JSON_ARRAY(1, 'a', null);", pg("functions-json.html"));
  k!("JSON_ARRAYAGG", "JSON_ARRAYAGG(<v>) -- aggregate JSON array constructor.", "SELECT JSON_ARRAYAGG(id) FROM users;", pg("functions-json.html"));
  k!("JSON_SCALAR", "JSON_SCALAR(<value>) -- wrap a value as JSON scalar.", "SELECT JSON_SCALAR(42);", pg("functions-json.html"));
  k!("JSON_SERIALIZE", "JSON_SERIALIZE(<jsonb>) -- canonical text serialisation.", "SELECT JSON_SERIALIZE(payload);", pg("functions-json.html"));
  k!("IS JSON", "IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] -- SQL/JSON predicate (PG16+).", "WHERE col IS JSON ARRAY", pg("functions-json.html"));
  k!("IS JSON VALUE", "IS [NOT] JSON VALUE -- the input is *any* JSON value (default if no shape token).", "WHERE payload::jsonb IS JSON VALUE", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  k!("IS JSON SCALAR", "IS [NOT] JSON SCALAR -- the input is a JSON number / string / boolean / null.", "WHERE payload IS JSON SCALAR", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  k!("IS JSON ARRAY", "IS [NOT] JSON ARRAY -- the input is a JSON array.", "WHERE payload IS JSON ARRAY", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  k!("IS JSON OBJECT", "IS [NOT] JSON OBJECT -- the input is a JSON object.", "WHERE payload IS JSON OBJECT", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  k!("IS NOT JSON", "IS NOT JSON [{VALUE|SCALAR|ARRAY|OBJECT}] -- negated SQL/JSON predicate.", "WHERE payload IS NOT JSON OBJECT", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  k!("WITH UNIQUE KEYS", "IS [NOT] JSON OBJECT WITH UNIQUE KEYS -- additionally require no duplicate keys at any level (PG16+).", "WHERE payload IS JSON OBJECT WITH UNIQUE KEYS", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  k!("WITHOUT UNIQUE KEYS", "IS [NOT] JSON OBJECT WITHOUT UNIQUE KEYS -- explicit opt-out (default).", "WHERE payload IS JSON OBJECT WITHOUT UNIQUE KEYS", pg("functions-json.html#FUNCTIONS-SQLJSON-IS-JSON"));
  // ---- JSON_TABLE clauses (PG17+ SQL/JSON) ----
  k!("NESTED PATH", "JSON_TABLE COLUMNS (... NESTED PATH '<jsonpath>' [AS <name>] COLUMNS (...)) -- shred a nested array/object into additional rows.", "COLUMNS (id INT PATH '$.id', NESTED PATH '$.items[*]' COLUMNS (qty INT PATH '$.qty'))", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("NESTED", "JSON_TABLE NESTED PATH '<jsonpath>' -- prefix of NESTED PATH.", "NESTED PATH '$.items[*]'", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("PATH", "JSON_TABLE / JSON_QUERY / JSON_VALUE PATH '<jsonpath>' -- jsonpath spec for a column or top-level expression.", "id INT PATH '$.id'", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("PLAN DEFAULT", "JSON_TABLE ... PLAN DEFAULT (INNER | OUTER, CROSS | UNION) -- choose the default sibling join / parent-child join semantics.", "PLAN DEFAULT (OUTER, UNION)", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("PLAN", "JSON_TABLE PLAN <name> { CROSS | UNION } <name> -- explicit nested-path join plan.", "PLAN (root OUTER (items CROSS tags))", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("ON EMPTY", "JSON_VALUE / JSON_QUERY / JSON_TABLE ... { ERROR | NULL | EMPTY ARRAY | EMPTY OBJECT | DEFAULT <expr> } ON EMPTY -- behavior when the jsonpath matches no value.", "DEFAULT '[]'::jsonb ON EMPTY", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("ON ERROR", "JSON_VALUE / JSON_QUERY / JSON_TABLE ... { ERROR | NULL | EMPTY ARRAY | EMPTY OBJECT | DEFAULT <expr> } ON ERROR -- behavior when jsonpath evaluation raises.", "NULL ON ERROR", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("EMPTY ARRAY", "ON EMPTY/ON ERROR clause: substitute `[]` (or `{}` for OBJECT) when the jsonpath returns nothing / errors.", "EMPTY ARRAY ON EMPTY", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("EMPTY OBJECT", "ON EMPTY/ON ERROR clause: substitute `{}` when the jsonpath returns nothing / errors.", "EMPTY OBJECT ON EMPTY", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("WRAPPER", "JSON_QUERY ... WITH { CONDITIONAL | UNCONDITIONAL } WRAPPER / WITHOUT WRAPPER -- choose how to wrap a non-array result.", "JSON_QUERY(j, '$.items' WITH UNCONDITIONAL WRAPPER)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("WITH WRAPPER", "JSON_QUERY ... WITH WRAPPER -- alias for WITH UNCONDITIONAL WRAPPER.", "JSON_QUERY(j, '$.items' WITH WRAPPER)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("WITHOUT WRAPPER", "JSON_QUERY ... WITHOUT WRAPPER (default).", "JSON_QUERY(j, '$.item' WITHOUT WRAPPER)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  // ---- CREATE FUNCTION/PROCEDURE BEGIN ATOMIC body (SQL standard) ----
  k!("BEGIN ATOMIC", "PG14+ CREATE FUNCTION ... LANGUAGE sql BEGIN ATOMIC ... END -- SQL-standard inline body; references are resolved at create time, so renames cascade.", "CREATE FUNCTION add(a int, b int) RETURNS int LANGUAGE sql BEGIN ATOMIC RETURN a + b; END;", pg("sql-createfunction.html"));
  k!("ATOMIC", "BEGIN ATOMIC ... END marker -- parsed and resolved at CREATE time (unlike the legacy `$$ ... $$` body).", "BEGIN ATOMIC RETURN 1; END", pg("sql-createfunction.html"));
  // ---- SQL standard SUBSTRING / SIMILAR TO ESCAPE chain ----
  k!("SUBSTRING SIMILAR", "SUBSTRING(<src> SIMILAR '<pattern>' ESCAPE '<char>') -- SQL-standard regex extraction.", "SELECT SUBSTRING('foo123bar' SIMILAR '%#\"[0-9]+#\"%' ESCAPE '#');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("SIMILAR TO ESCAPE", "WHERE <expr> SIMILAR TO '<pattern>' ESCAPE '<char>' -- pick a custom escape character.", "WHERE name SIMILAR TO 'a#%b' ESCAPE '#'", pg("functions-matching.html#FUNCTIONS-SIMILARTO-REGEXP"));
  k!("LIKE ESCAPE", "<expr> LIKE '<pattern>' ESCAPE '<char>' -- escape `%` / `_` with a custom character (default `\\\\`).", "WHERE name LIKE 'a/%' ESCAPE '/'", pg("functions-matching.html#FUNCTIONS-LIKE"));
  k!("ILIKE ESCAPE", "<expr> ILIKE '<pattern>' ESCAPE '<char>' -- case-insensitive LIKE with custom escape.", "WHERE name ILIKE 'A/_' ESCAPE '/'", pg("functions-matching.html#FUNCTIONS-LIKE"));
  k!("ESCAPE", "Escape-char clause: LIKE / ILIKE / SIMILAR TO ... ESCAPE '<char>'. Default is the single backslash.", "name LIKE 'a\\%b' ESCAPE '\\\\'", pg("functions-matching.html#FUNCTIONS-LIKE"));
  // ---- POSITION / OVERLAY remaining bits ----
  k!("OVERLAY PLACING", "OVERLAY(<s> PLACING <r> FROM <pos> [FOR <len>]) -- SQL-standard substring replacement.", "SELECT OVERLAY('Postgres' PLACING 'SQL' FROM 5);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("OVERLAY FROM", "Trailing part of OVERLAY(... FROM <pos>) -- start position is 1-based.", "OVERLAY('abc' PLACING 'X' FROM 2)", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  k!("OVERLAY FOR", "Optional length: OVERLAY(... FROM <pos> FOR <len>).", "OVERLAY('abcdef' PLACING 'XY' FROM 2 FOR 3)", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  // ---- TRIM standard syntax variants ----
  k!("TRIM FROM", "TRIM([{LEADING|TRAILING|BOTH}] [<chars>] FROM <text>) -- explicit chars argument.", "TRIM(LEADING '0' FROM '0042')", pg("functions-string.html"));
  // ---- SQL standard INTERVAL field qualifiers ----
  k!("YEAR TO MONTH", "INTERVAL '<n>' YEAR TO MONTH -- SQL-standard year-month interval qualifier; constrains the precision.", "SELECT INTERVAL '1-6' YEAR TO MONTH;", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("DAY TO HOUR", "INTERVAL '<n>' DAY TO HOUR -- restrict to day+hour precision.", "INTERVAL '2 3' DAY TO HOUR", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("DAY TO MINUTE", "INTERVAL '<n>' DAY TO MINUTE -- day/hour/minute precision.", "INTERVAL '2 3:15' DAY TO MINUTE", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("DAY TO SECOND", "INTERVAL '<n>' DAY TO SECOND -- full sub-day precision.", "INTERVAL '2 3:15:30' DAY TO SECOND", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("HOUR TO MINUTE", "INTERVAL '<n>' HOUR TO MINUTE -- hour+minute precision.", "INTERVAL '3:15' HOUR TO MINUTE", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("HOUR TO SECOND", "INTERVAL '<n>' HOUR TO SECOND -- sub-hour precision.", "INTERVAL '3:15:30' HOUR TO SECOND", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("MINUTE TO SECOND", "INTERVAL '<n>' MINUTE TO SECOND -- minute+second precision.", "INTERVAL '15:30' MINUTE TO SECOND", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("INTERVAL YEAR", "INTERVAL '<n>' YEAR -- year-only precision; truncates sub-year fields.", "INTERVAL '5' YEAR", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("INTERVAL MONTH", "INTERVAL '<n>' MONTH -- month-only precision.", "INTERVAL '18' MONTH", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("INTERVAL DAY", "INTERVAL '<n>' DAY -- day-only precision.", "INTERVAL '45' DAY", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("INTERVAL HOUR", "INTERVAL '<n>' HOUR -- hour-only precision.", "INTERVAL '36' HOUR", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("INTERVAL MINUTE", "INTERVAL '<n>' MINUTE -- minute-only precision.", "INTERVAL '120' MINUTE", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  k!("INTERVAL SECOND", "INTERVAL '<n>' SECOND [(p)] -- second-only precision; (p) is fractional digits.", "INTERVAL '90.5' SECOND(1)", pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT"));
  // ---- SQL standard CHARACTER family ----
  k!("CHARACTER LARGE OBJECT", "CHARACTER LARGE OBJECT -- SQL-standard alias for TEXT (PG stores as text).", "body CHARACTER LARGE OBJECT", pg("datatype-character.html"));
  k!("BINARY LARGE OBJECT", "BINARY LARGE OBJECT -- SQL-standard alias for BYTEA (PG stores as bytea).", "blob BINARY LARGE OBJECT", pg("datatype-binary.html"));
  k!("CLOB", "CLOB -- shorthand for CHARACTER LARGE OBJECT; PG maps to TEXT.", "body CLOB", pg("datatype-character.html"));
  k!("BLOB", "BLOB -- shorthand for BINARY LARGE OBJECT; PG maps to BYTEA.", "blob BLOB", pg("datatype-binary.html"));
  k!("NCHAR VARYING", "NCHAR VARYING(<n>) -- SQL-standard alias of VARCHAR(n) (national character set).", "name NCHAR VARYING(64)", pg("datatype-character.html"));
  k!("NATIONAL CHAR", "NATIONAL CHAR(<n>) -- SQL-standard alias of CHAR(n).", "code NATIONAL CHAR(3)", pg("datatype-character.html"));
  k!("NATIONAL CHARACTER", "NATIONAL CHARACTER [VARYING](<n>) -- SQL-standard char/varchar variant.", "name NATIONAL CHARACTER VARYING(64)", pg("datatype-character.html"));
  k!("NATIONAL CHARACTER VARYING", "NATIONAL CHARACTER VARYING(<n>) -- SQL-standard alias of VARCHAR(n).", "name NATIONAL CHARACTER VARYING(64)", pg("datatype-character.html"));
  k!("CHARACTER SET", "CHARACTER SET <name> -- SQL-standard; PG accepts the syntax but ignores it (encoding is per-database).", "name VARCHAR(64) CHARACTER SET utf8", pg("datatype-character.html"));
  // ---- SQL standard CAST helpers ----
  k!("AS BIGINT", "CAST(... AS BIGINT) -- 8-byte signed integer.", "SELECT CAST(x AS BIGINT);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS SMALLINT", "CAST(... AS SMALLINT) -- 2-byte signed integer.", "SELECT CAST(x AS SMALLINT);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS INTEGER", "CAST(... AS INTEGER) -- 4-byte signed integer.", "SELECT CAST(x AS INTEGER);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS NUMERIC", "CAST(... AS NUMERIC(p, s)) -- exact decimal with precision/scale.", "SELECT CAST(price AS NUMERIC(10,2));", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS REAL", "CAST(... AS REAL) -- 4-byte float.", "SELECT CAST(x AS REAL);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS DOUBLE PRECISION", "CAST(... AS DOUBLE PRECISION) -- 8-byte float.", "SELECT CAST(x AS DOUBLE PRECISION);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS DATE", "CAST(... AS DATE) -- calendar date, no time/zone.", "SELECT CAST(ts AS DATE);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS TIME", "CAST(... AS TIME) -- time of day, no zone.", "SELECT CAST(ts AS TIME);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS TIMESTAMPTZ", "CAST(... AS TIMESTAMPTZ) -- with time zone (PG stores as UTC).", "SELECT CAST(ts AS TIMESTAMPTZ);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS BOOLEAN", "CAST(... AS BOOLEAN) -- accepts 't','f','yes','no',1,0.", "SELECT CAST(flag AS BOOLEAN);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  // ---- ORDER BY collation helpers (SQL standard) ----
  k!("COLLATE", "ORDER BY <col> COLLATE \"<collation>\" -- pick a non-default collation for ordering / comparisons.", "ORDER BY name COLLATE \"de-DE-x-icu\"", pg("collation.html"));
  k!("WITH ORDINALITY", "FROM <set-returning-fn>(...) WITH ORDINALITY -- append an `ordinality` bigint column starting at 1.", "SELECT * FROM unnest(ARRAY['a','b']) WITH ORDINALITY AS t(v, ord);", pg("queries-table-expressions.html"));
  // ---- Aggregate / window-function modifier syntax ----
  k!("FILTER", "<agg>(...) FILTER (WHERE <pred>) -- conditionally include rows in the aggregate.", "SELECT count(*) FILTER (WHERE flag) FROM t;", pg("sql-expressions.html#SYNTAX-AGGREGATES"));
  k!("FILTER (WHERE", "Same as FILTER WHERE -- aggregate-only WHERE predicate.", "count(*) FILTER (WHERE active)", pg("sql-expressions.html#SYNTAX-AGGREGATES"));
  k!("IGNORE NULLS", "Window function modifier (SQL standard): skip NULLs in LAG/LEAD/FIRST_VALUE/LAST_VALUE/NTH_VALUE. PG accepts the syntax in PG16+ via the standard-conformant parser.", "lag(v) IGNORE NULLS OVER (ORDER BY t)", pg("functions-window.html"));
  k!("RESPECT NULLS", "Window function modifier (SQL standard, default): keep NULLs in LAG/LEAD/FIRST_VALUE/etc.", "lag(v) RESPECT NULLS OVER (ORDER BY t)", pg("functions-window.html"));
  k!("FROM FIRST", "NTH_VALUE(<expr>, <n>) FROM FIRST -- pick the nth row from the start of the window frame (default).", "nth_value(v, 1) FROM FIRST OVER (ORDER BY t)", pg("functions-window.html"));
  k!("FROM LAST", "NTH_VALUE(<expr>, <n>) FROM LAST -- pick the nth row counting backwards from the end of the frame.", "nth_value(v, 1) FROM LAST OVER (ORDER BY t)", pg("functions-window.html"));
  k!("AGG ORDER BY", "<agg>(<expr> ORDER BY <key> [ASC|DESC]) -- intra-aggregate ordering (e.g. for string_agg / array_agg / json_agg).", "SELECT string_agg(name, ',' ORDER BY id) FROM t;", pg("sql-expressions.html#SYNTAX-AGGREGATES"));
  // ---- XML predicate helpers ----
  k!("XMLEXISTS PASSING", "XMLEXISTS('<xpath>' PASSING [BY {REF|VALUE}] <xml> [AS <name>]) -- bind XML/values reachable inside the XPath.", "XMLEXISTS('/r/i' PASSING BY VALUE doc AS doc)", pg("functions-xml.html#FUNCTIONS-XML-PREDICATES"));
  k!("PASSING BY VALUE", "XMLEXISTS / XMLTABLE ... PASSING BY VALUE <expr> -- pass argument by value (the only mode PG supports; BY REF is parsed and ignored).", "XMLEXISTS('/r' PASSING BY VALUE x)", pg("functions-xml.html#FUNCTIONS-XML-PREDICATES"));
  k!("PASSING BY REF", "XMLEXISTS / XMLTABLE ... PASSING BY REF <expr> -- SQL-standard mode; PG accepts but treats as BY VALUE.", "XMLEXISTS('/r' PASSING BY REF x)", pg("functions-xml.html#FUNCTIONS-XML-PREDICATES"));
  k!("XMLPARSE DOCUMENT", "XMLPARSE(DOCUMENT <text>) -- parse a complete XML document; requires a single root element.", "XMLPARSE(DOCUMENT '<root/>')", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  k!("XMLPARSE CONTENT", "XMLPARSE(CONTENT <text>) -- parse an XML fragment (no root requirement).", "XMLPARSE(CONTENT '<a/><b/>')", pg("functions-xml.html#FUNCTIONS-PRODUCING-XML"));
  // ---- GROUPING SETS / ROLLUP / CUBE column-group forms ----
  k!("GROUPING SETS", "GROUP BY GROUPING SETS ((<cols>), (<cols>), ()) -- enumerate the exact grouping combinations; () is grand total.", "SELECT a, b, count(*) FROM t GROUP BY GROUPING SETS ((a, b), (a), ());", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  k!("ROLLUP", "GROUP BY ROLLUP (a, b, c) -- expands to a hierarchy of grouping sets: (a,b,c), (a,b), (a), ().", "GROUP BY ROLLUP (region, sub_region, store)", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  k!("CUBE", "GROUP BY CUBE (a, b) -- every subset of the columns: (a,b), (a), (b), ().", "GROUP BY CUBE (region, channel)", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  k!("ROLLUP COLUMN GROUP", "GROUP BY ROLLUP ((a, b), c) -- ROLLUP with composite column groups; the (a,b) pair is treated as one level.", "GROUP BY ROLLUP ((country, city), product)", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  k!("CUBE COLUMN GROUP", "GROUP BY CUBE ((a, b), c) -- CUBE with composite column groups; (a,b) is treated as a single grouping dimension.", "GROUP BY CUBE ((year, quarter), region)", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  // ---- TABLE function syntax ----
  k!("TABLE (FUNCTION", "FROM TABLE(<set_returning_function>(...)) -- SQL-standard spelling of the implicit `FROM <srf>(...)` form. PG accepts it too.", "SELECT * FROM TABLE(generate_series(1, 10))", pg("queries-table-expressions.html"));
  // ---- Recursive CTE CYCLE USING column ----
  k!("CYCLE USING", "WITH RECURSIVE ... CYCLE <cols> SET <flag> USING <path> -- PG14+ cycle detection; the USING column is the path array used for the test.", "CYCLE id SET is_cycle USING path", pg("queries-with.html#QUERIES-WITH-CYCLE"));
  k!("CYCLE SET", "WITH RECURSIVE ... CYCLE <cols> SET <flag_col> [TO <true_val>] [DEFAULT <false_val>] -- flag column appended to the recursive result.", "CYCLE id SET is_cycle TO 't' DEFAULT 'f' USING path", pg("queries-with.html#QUERIES-WITH-CYCLE"));
  // ---- Full-text search weight classes ----
  k!("WEIGHT A", "setweight(tsvector, 'A') -- highest tsearch weight (usually used for titles).", "SELECT setweight(to_tsvector(title), 'A');", pg("textsearch-controls.html#TEXTSEARCH-MANIPULATE-TSVECTOR"));
  k!("WEIGHT B", "setweight(tsvector, 'B') -- second-highest tsearch weight (often subtitles / leading paragraphs).", "SELECT setweight(to_tsvector(body), 'B');", pg("textsearch-controls.html#TEXTSEARCH-MANIPULATE-TSVECTOR"));
  k!("WEIGHT C", "setweight(tsvector, 'C') -- third tsearch weight (often section text).", "setweight(to_tsvector(section), 'C')", pg("textsearch-controls.html#TEXTSEARCH-MANIPULATE-TSVECTOR"));
  k!("WEIGHT D", "setweight(tsvector, 'D') -- lowest tsearch weight (default for plain body text).", "setweight(to_tsvector(body), 'D')", pg("textsearch-controls.html#TEXTSEARCH-MANIPULATE-TSVECTOR"));
  // ---- Misc SQL:2003 ----
  k!("MULTISET", "SQL:2003 MULTISET / MULTISET UNION / MULTISET INTERSECT -- collection of unordered values with duplicates. PG does NOT implement MULTISET (use arrays).", "-- not supported by PG; use ARRAY[...] || ARRAY[...] for union", pg("datatype.html"));
  // ---- JSON_TABLE column-spec details (SQL/JSON, PG17+) ----
  k!("EXISTS PATH", "JSON_TABLE COLUMNS (<name> [type] EXISTS PATH '<jsonpath>') -- boolean column: true when jsonpath matches.", "COLUMNS (has_tags boolean EXISTS PATH '$.tags')", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  k!("DEFAULT ON EMPTY", "ON EMPTY clause variant: ... DEFAULT <expr> ON EMPTY -- substitute <expr> when jsonpath returns no value.", "DEFAULT 0 ON EMPTY", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("DEFAULT ON ERROR", "ON ERROR clause variant: ... DEFAULT <expr> ON ERROR -- substitute <expr> when jsonpath evaluation errors.", "DEFAULT '{}'::jsonb ON ERROR", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("NULL ON ERROR", "JSON_VALUE / JSON_QUERY ... NULL ON ERROR -- return SQL NULL on path-evaluation errors (default for JSON_VALUE).", "JSON_VALUE(j, '$.id' NULL ON ERROR)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("ERROR ON ERROR", "JSON_VALUE / JSON_QUERY ... ERROR ON ERROR -- raise an exception on path-evaluation errors.", "JSON_VALUE(j, '$.id' ERROR ON ERROR)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("NULL ON EMPTY", "JSON_VALUE / JSON_QUERY ... NULL ON EMPTY -- return SQL NULL when jsonpath matches no value (default).", "JSON_VALUE(j, '$.id' NULL ON EMPTY)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("ERROR ON EMPTY", "JSON_VALUE / JSON_QUERY ... ERROR ON EMPTY -- raise an exception when jsonpath matches nothing.", "JSON_VALUE(j, '$.id' ERROR ON EMPTY)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  // ---- JSON constructor / aggregate NULL handling (SQL:2023) ----
  k!("ABSENT ON NULL", "JSON_OBJECT / JSON_OBJECTAGG / JSON_ARRAY / JSON_ARRAYAGG ... ABSENT ON NULL -- omit entries / elements whose value is NULL.", "JSON_OBJECT('id' VALUE id, 'mid' VALUE middle_name ABSENT ON NULL)", pg("functions-json.html#FUNCTIONS-SQLJSON-OBJECT"));
  k!("NULL ON NULL", "JSON_OBJECT / JSON_OBJECTAGG ... NULL ON NULL -- keep `null` entries (default for JSON_ARRAY).", "JSON_OBJECT('id' VALUE id, 'tag' VALUE tag NULL ON NULL)", pg("functions-json.html#FUNCTIONS-SQLJSON-OBJECT"));
  k!("KEY VALUE", "JSON_OBJECT(KEY <key_expr> VALUE <val_expr>) -- SQL-standard pair syntax; PG also accepts the shorthand `<k> : <v>`.", "JSON_OBJECT(KEY 'id' VALUE id)", pg("functions-json.html#FUNCTIONS-SQLJSON-OBJECT"));
  k!("RETURNING JSON", "JSON_OBJECT / JSON_ARRAY / JSON_QUERY ... RETURNING JSON -- choose json output type.", "JSON_OBJECT('id' VALUE id RETURNING json)", pg("functions-json.html"));
  k!("RETURNING JSONB", "JSON_OBJECT / JSON_ARRAY / JSON_QUERY ... RETURNING JSONB -- choose jsonb output type (more efficient for storage / indexing).", "JSON_OBJECT('id' VALUE id RETURNING jsonb)", pg("functions-json.html"));
  k!("RETURNING TEXT", "JSON_QUERY / JSON_VALUE ... RETURNING text -- return the result coerced to text.", "JSON_VALUE(j, '$.name' RETURNING text)", pg("functions-json.html"));
  k!("FORMAT JSON", "JSON_OBJECT(... FORMAT JSON) / JSON_QUERY(... FORMAT JSON) -- treat the value as already-encoded JSON (don't re-quote strings).", "JSON_OBJECT('payload' VALUE col FORMAT JSON)", pg("functions-json.html"));
  k!("FORMAT JSONB", "Variant of FORMAT JSON -- treat the value as binary jsonb.", "JSON_OBJECT('payload' VALUE col FORMAT JSONB)", pg("functions-json.html"));
  // ---- JSON path FORMAT/OMIT QUOTES (PG17 SQL/JSON) ----
  k!("WITH QUOTES", "JSON_QUERY(... WITH QUOTES) -- keep surrounding `\"...\"` when the result is a scalar string (default).", "JSON_QUERY(j, '$.name' WITH QUOTES)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  k!("OMIT QUOTES", "JSON_QUERY(... OMIT QUOTES) -- strip the surrounding `\"...\"` when the result is a scalar string.", "JSON_QUERY(j, '$.name' OMIT QUOTES)", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  // ---- DOMAIN nuances ----
  k!("DOMAIN CHECK", "CREATE DOMAIN <name> AS <type> [NOT NULL] [DEFAULT ...] [CONSTRAINT <c>] CHECK (<expr>) -- domain-wide invariant evaluated on every assignment.", "CREATE DOMAIN positive_int AS int CHECK (VALUE > 0);", pg("sql-createdomain.html"));
  k!("DOMAIN VALUE", "Inside DOMAIN ... CHECK (<expr>) the literal `VALUE` refers to the value being checked.", "CHECK (VALUE > 0 AND VALUE < 100)", pg("sql-createdomain.html"));
  k!("CONSTRAINT CHECK", "Named CHECK constraint: CONSTRAINT <name> CHECK (<expr>) -- gives the predicate a stable name for ALTER ... VALIDATE.", "CONSTRAINT positive CHECK (qty > 0)", pg("sql-createtable.html"));
  k!("ADD CONSTRAINT CHECK", "ALTER TABLE/DOMAIN ADD CONSTRAINT <name> CHECK (<expr>) [NOT VALID] -- add a CHECK without rewriting (use NOT VALID then VALIDATE later).", "ALTER TABLE t ADD CONSTRAINT positive CHECK (qty > 0) NOT VALID;", pg("sql-altertable.html"));
  // ---- COLLATION provider ----
  k!("LOCALE_PROVIDER", "CREATE DATABASE / CREATE COLLATION ... LOCALE_PROVIDER { libc | icu | builtin } -- selects which locale backend resolves comparisons.", "CREATE COLLATION fr (LOCALE_PROVIDER = icu, LOCALE = 'fr-FR');", pg("collation.html#COLLATION-MANAGING-CREATE"));
  k!("PROVIDER LIBC", "CREATE COLLATION ... (PROVIDER = libc) -- POSIX libc locales; behaviour depends on the OS install.", "CREATE COLLATION c (PROVIDER = libc, LOCALE = 'fr_FR.UTF-8');", pg("collation.html"));
  k!("PROVIDER ICU", "CREATE COLLATION ... (PROVIDER = icu) -- stable, version-tracked ICU collations (recommended for portability).", "CREATE COLLATION fr (PROVIDER = icu, LOCALE = 'fr-FR-x-icu');", pg("collation.html"));
  k!("PROVIDER BUILTIN", "CREATE COLLATION ... (PROVIDER = builtin) -- PG17+ built-in C.UTF-8 / C locale provider; no external libraries needed.", "CREATE COLLATION c_utf8 (PROVIDER = builtin, LOCALE = 'C.UTF-8');", pg("collation.html"));
  k!("DETERMINISTIC", "CREATE COLLATION ... (DETERMINISTIC = false) -- non-deterministic collation; allows case/accent-insensitive equality.", "CREATE COLLATION ci (PROVIDER = icu, LOCALE = 'und-u-ks-level2', DETERMINISTIC = false);", pg("collation.html"));
  k!("COLLATION VERSION", "CREATE COLLATION ... (VERSION = '<v>') -- pin a specific collation version so ALTER COLLATION REFRESH VERSION can detect drift.", "CREATE COLLATION fr (PROVIDER = icu, LOCALE = 'fr-FR', VERSION = '153.14');", pg("collation.html"));
  // ---- TEXT SEARCH specifics ----
  k!("TEXT SEARCH PARSER", "CREATE TEXT SEARCH PARSER <name> (...) -- defines how raw text is split into tokens.", "CREATE TEXT SEARCH PARSER my_parser (START = prsd_start, GETTOKEN = prsd_nexttoken, END = prsd_end, LEXTYPES = prsd_lextype);", pg("sql-createtsparser.html"));
  k!("TEXT SEARCH DICTIONARY", "CREATE TEXT SEARCH DICTIONARY <name> (TEMPLATE = <tmpl>[, ...]) -- maps tokens to lexemes / discards stopwords.", "CREATE TEXT SEARCH DICTIONARY english_stem (TEMPLATE = snowball, LANGUAGE = english);", pg("sql-createtsdictionary.html"));
  k!("TEXT SEARCH TEMPLATE", "CREATE TEXT SEARCH TEMPLATE <name> (LEXIZE = <fn>[, INIT = <fn>]) -- low-level building block reused by dictionaries.", "CREATE TEXT SEARCH TEMPLATE my_tmpl (INIT = my_init, LEXIZE = my_lexize);", pg("sql-createtstemplate.html"));
  k!("TEXT SEARCH CONFIGURATION", "CREATE TEXT SEARCH CONFIGURATION <name> (PARSER = <parser> | COPY = <other>) -- bundles a parser plus token-type -> dictionary mappings.", "CREATE TEXT SEARCH CONFIGURATION en (COPY = english);", pg("sql-createtsconfig.html"));
  k!("MAPPING FOR", "ALTER TEXT SEARCH CONFIGURATION <c> ADD MAPPING FOR <token_types> WITH <dicts> -- bind token classes to dictionaries.", "ALTER TEXT SEARCH CONFIGURATION en ADD MAPPING FOR asciiword, word WITH english_stem;", pg("sql-altertsconfig.html"));
  k!("MAPPING REPLACE", "ALTER TEXT SEARCH CONFIGURATION <c> ALTER MAPPING REPLACE <old_dict> WITH <new_dict> -- swap a dictionary without re-listing token types.", "ALTER TEXT SEARCH CONFIGURATION en ALTER MAPPING REPLACE simple WITH english_stem;", pg("sql-altertsconfig.html"));
  // ---- RULE INSTEAD WITH ----
  k!("RULE INSTEAD", "CREATE RULE <name> AS ON <event> TO <rel> DO INSTEAD <action> -- replace the original DML with the rule's action.", "CREATE RULE no_delete AS ON DELETE TO t DO INSTEAD NOTHING;", pg("sql-createrule.html"));
  k!("DO INSTEAD NOTHING", "CREATE RULE ... DO INSTEAD NOTHING -- suppress the original DML entirely.", "CREATE RULE block_ins AS ON INSERT TO t DO INSTEAD NOTHING;", pg("sql-createrule.html"));
  k!("DO INSTEAD", "CREATE RULE ... DO INSTEAD <action> -- replace the original DML.", "DO INSTEAD INSERT INTO log VALUES (...)", pg("sql-createrule.html"));
  // ---- GENERATED column extras ----
  k!("GENERATED ALWAYS AS STORED", "PG12+ stored generated column: computed expression materialised on disk (only STORED was supported pre-PG18; VIRTUAL added in PG18).", "amount_with_tax NUMERIC GENERATED ALWAYS AS (amount * 1.2) STORED", pg("ddl-generated-columns.html"));
  k!("GENERATED ALWAYS AS", "Generic prefix used by both identity (`AS IDENTITY`) and computed (`AS (<expr>) STORED|VIRTUAL`) generated columns.", "GENERATED ALWAYS AS (amount * 1.2) STORED", pg("ddl-generated-columns.html"));
  k!("GENERATED BY DEFAULT AS", "Identity-only prefix: GENERATED BY DEFAULT AS IDENTITY -- user can override; only valid with IDENTITY (not computed expressions).", "id int GENERATED BY DEFAULT AS IDENTITY", pg("sql-createtable.html"));
  k!("AS IDENTITY", "GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY [ (<sequence options>) ] -- modern identity column (preferred over SERIAL).", "id int GENERATED ALWAYS AS IDENTITY (START WITH 1000)", pg("sql-createtable.html"));
  k!("IDENTITY (", "AS IDENTITY ( <sequence_options> ) -- supply optional sequence parameters (INCREMENT/MINVALUE/MAXVALUE/CACHE/CYCLE).", "AS IDENTITY (INCREMENT 5 START 100 CACHE 20)", pg("sql-createtable.html"));
  // ---- INHERITS / OF / TYPED tables ----
  k!("INHERITS", "CREATE TABLE child () INHERITS (<parent>[, ...]) -- multiple-inheritance table; rows of children visible from parent unless ONLY is used.", "CREATE TABLE cars () INHERITS (vehicles);", pg("ddl-inherit.html"));
  k!("CREATE TABLE OF", "CREATE TABLE <name> OF <composite_type> -- typed table whose columns mirror the composite type.", "CREATE TABLE my_t OF address_t;", pg("sql-createtable.html"));
  k!("OF TYPE", "Generic spelling of `OF <composite_type>` -- table inherits its column shape from a composite type.", "CREATE TABLE t OF address_t (PRIMARY KEY (id));", pg("sql-createtable.html"));
  k!("NOT OF", "ALTER TABLE <name> NOT OF -- detach a typed table from its composite type (PG13+).", "ALTER TABLE my_t NOT OF;", pg("sql-altertable.html"));
  // ---- FK NULLS NOT DISTINCT ----
  k!("NULLS NOT DISTINCT", "UNIQUE / PRIMARY KEY / EXCLUDE constraint modifier (PG15+): treat NULLs as equal, so duplicate NULLs are rejected.", "UNIQUE NULLS NOT DISTINCT (email)", pg("sql-createtable.html"));
  k!("NULLS DISTINCT", "Default for UNIQUE constraints -- NULLs are considered distinct (multiple NULLs allowed).", "UNIQUE NULLS DISTINCT (col)", pg("sql-createtable.html"));
  // ---- Index OPCLASS specifics ----
  k!("OPERATOR CLASS", "CREATE INDEX ix ON t USING gin (<col> <opclass>) -- choose the operator class explicitly; needed for non-default ops (e.g. jsonb_path_ops, gin_trgm_ops).", "CREATE INDEX ix_doc ON docs USING gin (data jsonb_path_ops);", pg("sql-createopclass.html"));
  k!("jsonb_path_ops", "GIN opclass for jsonb that indexes only path -> value pairs. Faster + smaller than jsonb_ops but loses the `?` `?|` `?&` ops.", "USING gin (data jsonb_path_ops)", pg("datatype-json.html#JSON-INDEXING"));
  k!("jsonb_ops", "Default GIN opclass for jsonb -- supports `@>`, `?`, `?|`, `?&` and key/value path operators.", "USING gin (data jsonb_ops)", pg("datatype-json.html#JSON-INDEXING"));
  k!("gin_trgm_ops", "pg_trgm GIN opclass -- enables `%`, `<%`, `<<%` similarity ops + LIKE/ILIKE acceleration.", "CREATE INDEX ON users USING gin (name gin_trgm_ops);", pg("pgtrgm.html#PGTRGM-INDEX"));
  k!("gist_trgm_ops", "pg_trgm GiST opclass -- KNN-friendly variant (supports `<->`).", "CREATE INDEX ON users USING gist (name gist_trgm_ops);", pg("pgtrgm.html#PGTRGM-INDEX"));
  k!("text_pattern_ops", "BTREE opclass that compares strictly byte-by-byte, ignoring the column's collation -- needed for LIKE 'foo%' to use a btree index under a non-C collation.", "CREATE INDEX ON t (name text_pattern_ops);", pg("indexes-opclass.html"));
  k!("varchar_pattern_ops", "Like text_pattern_ops but for varchar.", "CREATE INDEX ON t (code varchar_pattern_ops);", pg("indexes-opclass.html"));
  k!("bpchar_pattern_ops", "Like text_pattern_ops but for char(n) / bpchar.", "CREATE INDEX ON t (code bpchar_pattern_ops);", pg("indexes-opclass.html"));
  // ---- INCLUDE column list nuance ----
  k!("INDEX INCLUDE", "INDEX ON t (<keys>) INCLUDE (<extras>) -- extras are not part of uniqueness but are stored in the leaf, enabling more index-only scans.", "CREATE UNIQUE INDEX ON t (id) INCLUDE (name, updated_at);", pg("sql-createindex.html"));
  // ---- Storage reloptions (ALTER TABLE / CREATE TABLE ... WITH) ----
  k!("fillfactor", "Storage reloption: leave free space in each page for HOT updates (heap) or splits (btree). Default 100 (heap) / 90 (btree).", "ALTER TABLE t SET (fillfactor = 80);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_enabled", "Storage reloption: per-table override for autovacuum.", "ALTER TABLE big SET (autovacuum_enabled = false);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_vacuum_threshold", "Per-table autovacuum threshold (rows). Lower for hotter tables.", "ALTER TABLE t SET (autovacuum_vacuum_threshold = 1000);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_vacuum_scale_factor", "Per-table autovacuum scale factor.", "ALTER TABLE t SET (autovacuum_vacuum_scale_factor = 0.05);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_analyze_threshold", "Per-table autoanalyze threshold (rows).", "ALTER TABLE t SET (autovacuum_analyze_threshold = 500);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_analyze_scale_factor", "Per-table autoanalyze scale factor.", "ALTER TABLE t SET (autovacuum_analyze_scale_factor = 0.02);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_vacuum_cost_delay", "Per-table autovacuum cost delay (ms).", "ALTER TABLE t SET (autovacuum_vacuum_cost_delay = 5);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("autovacuum_vacuum_insert_threshold", "Per-table threshold for insert-only autovacuum (PG13+).", "ALTER TABLE log SET (autovacuum_vacuum_insert_threshold = 10000);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("parallel_workers", "Per-table cap on parallel workers used for sequential scans.", "ALTER TABLE big SET (parallel_workers = 4);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("user_catalog_table", "Mark a regular table as a 'catalog' for logical decoding -- INSERT/UPDATE/DELETE still emit WAL even on the standby. Extensions use this.", "ALTER TABLE meta SET (user_catalog_table = true);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("deduplicate_items", "BTree-only reloption (PG13+): enable leaf-page dedup.", "ALTER INDEX ix SET (deduplicate_items = on);", pg("sql-createindex.html#SQL-CREATEINDEX-STORAGE-PARAMETERS"));
  k!("toast.autovacuum_enabled", "Per-table TOAST autovacuum toggle.", "ALTER TABLE t SET (toast.autovacuum_enabled = true);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("toast.autovacuum_vacuum_threshold", "Per-table TOAST autovacuum threshold (rows).", "ALTER TABLE t SET (toast.autovacuum_vacuum_threshold = 50000);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  k!("toast_tuple_target", "Per-table reloption: target inline TOAST tuple size before TOAST-out kicks in (default 2032 bytes).", "ALTER TABLE wide SET (toast_tuple_target = 4080);", pg("sql-createtable.html#SQL-CREATETABLE-STORAGE-PARAMETERS"));
  // ---- PG18 NOT ENFORCED constraints ----
  k!("NOT ENFORCED", "PG18+ constraint modifier: declare the constraint but skip runtime checks. Useful for hand-validated data or replication scenarios.", "ALTER TABLE t ADD CHECK (qty > 0) NOT ENFORCED;", pg("sql-altertable.html"));
  k!("ENFORCED", "Default for constraints: rows must satisfy the predicate. Counterpart of NOT ENFORCED.", "ALTER TABLE t ALTER CONSTRAINT positive ENFORCED;", pg("sql-altertable.html"));
  // ---- Replication / VM admin fns ----
  k!("pg_truncate_visibility_map", "pg_truncate_visibility_map(regclass) -> void -- discard the VM file for a relation; forces a re-build at next VACUUM. PG13+.", "SELECT pg_truncate_visibility_map('users'::regclass);", pg("functions-admin.html#FUNCTIONS-ADMIN-INDEX-TABLE"));
  // ---- SECURITY LABEL providers ----
  k!("SECURITY LABEL FOR", "SECURITY LABEL FOR <provider> ON <obj> IS '<label>' -- attach a label managed by a specific label provider (selinux/anon/...).", "SECURITY LABEL FOR selinux ON TABLE secret IS 'system_u:object_r:sepgsql_table_t:s0';", pg("sql-security-label.html"));
  k!("LABEL FOR", "Trailing portion of SECURITY LABEL: ... FOR <provider> -- pick which loaded label provider stores the label.", "SECURITY LABEL FOR selinux ON ...", pg("sql-security-label.html"));
  // ---- EVENT TRIGGER WHEN TAG filter (PG17 LOGIN event) ----
  k!("EVENT TRIGGER WHEN", "CREATE EVENT TRIGGER <name> ON <event> WHEN TAG IN ('<tag>', ...) EXECUTE FUNCTION <fn>() -- restrict to specific command tags.", "CREATE EVENT TRIGGER t ON ddl_command_start WHEN TAG IN ('CREATE TABLE') EXECUTE FUNCTION fn();", pg("sql-createeventtrigger.html"));
  k!("WHEN TAG IN", "Event-trigger filter: WHEN TAG IN ('CREATE TABLE', 'CREATE INDEX', ...) -- match by command-tag string.", "WHEN TAG IN ('CREATE TABLE', 'CREATE INDEX')", pg("sql-createeventtrigger.html"));
  k!("EVENT TRIGGER LOGIN", "PG17+ event-trigger event: fires once when a user logs in. Trigger fn cannot reject the login but can audit / configure session.", "CREATE EVENT TRIGGER t ON login EXECUTE FUNCTION audit_login();", pg("event-trigger-overview.html"));
  // ---- COMMENT ON missing object classes ----
  k!("COMMENT ON LARGE OBJECT", "COMMENT ON LARGE OBJECT <oid> IS '<text>' -- attach a comment to a large object by oid.", "COMMENT ON LARGE OBJECT 12345 IS 'invoice scan';", pg("sql-comment.html"));
  k!("COMMENT ON SUBSCRIPTION", "COMMENT ON SUBSCRIPTION <name> IS '<text>'.", "COMMENT ON SUBSCRIPTION sub IS 'primary -> reporting';", pg("sql-comment.html"));
  k!("COMMENT ON PUBLICATION", "COMMENT ON PUBLICATION <name> IS '<text>'.", "COMMENT ON PUBLICATION pub IS 'tables for read replica';", pg("sql-comment.html"));
  k!("COMMENT ON EVENT TRIGGER", "COMMENT ON EVENT TRIGGER <name> IS '<text>'.", "COMMENT ON EVENT TRIGGER audit_ddl IS 'logs every DDL';", pg("sql-comment.html"));
  k!("COMMENT ON ACCESS METHOD", "COMMENT ON ACCESS METHOD <name> IS '<text>'.", "COMMENT ON ACCESS METHOD bloom IS 'lossy filter index';", pg("sql-comment.html"));
  k!("COMMENT ON FOREIGN TABLE", "COMMENT ON FOREIGN TABLE <name> IS '<text>'.", "COMMENT ON FOREIGN TABLE remote_users IS 'pulled from upstream';", pg("sql-comment.html"));
  k!("COMMENT ON FOREIGN DATA WRAPPER", "COMMENT ON FOREIGN DATA WRAPPER <name> IS '<text>'.", "COMMENT ON FOREIGN DATA WRAPPER postgres_fdw IS 'cross-cluster federation';", pg("sql-comment.html"));
  k!("COMMENT ON SERVER", "COMMENT ON SERVER <name> IS '<text>'.", "COMMENT ON SERVER reporting_pg IS 'BI replica';", pg("sql-comment.html"));
  k!("COMMENT ON USER MAPPING", "COMMENT ON USER MAPPING FOR <role> SERVER <name> IS '<text>'.", "COMMENT ON USER MAPPING FOR app_user SERVER reporting_pg IS 'read-only';", pg("sql-comment.html"));
  k!("COMMENT ON COLLATION", "COMMENT ON COLLATION <name> IS '<text>'.", "COMMENT ON COLLATION fr IS 'French locale';", pg("sql-comment.html"));
  k!("COMMENT ON CONVERSION", "COMMENT ON CONVERSION <name> IS '<text>'.", "COMMENT ON CONVERSION utf8_to_latin1 IS '';", pg("sql-comment.html"));
  k!("COMMENT ON STATISTICS", "COMMENT ON STATISTICS <name> IS '<text>'.", "COMMENT ON STATISTICS stat_zip IS 'multi-col selectivity';", pg("sql-comment.html"));
  // ---- LISTEN / NOTIFY surface ----
  k!("LISTEN channel", "LISTEN <channel> -- subscribe to NOTIFY messages for <channel>; valid identifiers only, no quoting.", "LISTEN order_events;", pg("sql-listen.html"));
  k!("NOTIFY channel", "NOTIFY <channel> [, '<payload>'] -- send an asynchronous message; payload is text (1 KB cap by default).", "NOTIFY order_events, 'shipped:42';", pg("sql-notify.html"));
  k!("UNLISTEN ALL", "UNLISTEN * -- cancel all this session's LISTEN subscriptions.", "UNLISTEN *;", pg("sql-unlisten.html"));
  // ---- Misc CREATE ROLE chain kws ----
  k!("VALID UNTIL", "CREATE/ALTER ROLE ... VALID UNTIL '<timestamptz>' -- password expiration.", "ALTER ROLE bob VALID UNTIL '2026-12-31';", pg("sql-createrole.html"));
  k!("CONNECTION LIMIT", "CREATE/ALTER ROLE ... CONNECTION LIMIT <n> -- cap concurrent sessions per role (-1 = unlimited, default).", "ALTER ROLE bob CONNECTION LIMIT 5;", pg("sql-createrole.html"));
  k!("ENCRYPTED PASSWORD", "CREATE/ALTER ROLE ... ENCRYPTED PASSWORD '<plaintext>' -- SCRAM-hash on the way in (default behavior).", "CREATE ROLE bob LOGIN ENCRYPTED PASSWORD 'secret';", pg("sql-createrole.html"));
  k!("UNENCRYPTED", "CREATE/ALTER ROLE ... UNENCRYPTED PASSWORD '...' -- deprecated; PG13+ rejects this form.", "-- avoid: ALTER ROLE bob UNENCRYPTED PASSWORD '...'", pg("sql-createrole.html"));
  k!("SUPERUSER", "CREATE/ALTER ROLE ... SUPERUSER -- bypass every permission check. Reserve for admin role only.", "ALTER ROLE admin SUPERUSER;", pg("sql-createrole.html"));
  k!("NOSUPERUSER", "Default for new roles -- explicit denial of SUPERUSER.", "CREATE ROLE app LOGIN NOSUPERUSER;", pg("sql-createrole.html"));
  k!("CREATEDB", "CREATE/ALTER ROLE ... CREATEDB -- allow database creation.", "CREATE ROLE app CREATEDB;", pg("sql-createrole.html"));
  k!("NOCREATEDB", "Default for new roles -- explicit denial of CREATEDB.", "CREATE ROLE app NOCREATEDB;", pg("sql-createrole.html"));
  k!("CREATEROLE", "CREATE/ALTER ROLE ... CREATEROLE -- allow creating + managing other roles.", "CREATE ROLE admin CREATEROLE;", pg("sql-createrole.html"));
  k!("NOCREATEROLE", "Default for new roles -- explicit denial of CREATEROLE.", "CREATE ROLE app NOCREATEROLE;", pg("sql-createrole.html"));
  k!("REPLICATION", "CREATE/ALTER ROLE ... REPLICATION -- allow connections in replication mode (logical/physical).", "CREATE ROLE rep_user LOGIN REPLICATION PASSWORD '...';", pg("sql-createrole.html"));
  k!("NOREPLICATION", "Default for new roles -- explicit denial of REPLICATION.", "CREATE ROLE app NOREPLICATION;", pg("sql-createrole.html"));
  k!("BYPASSRLS", "CREATE/ALTER ROLE ... BYPASSRLS -- skip every row-level security policy.", "ALTER ROLE migrator BYPASSRLS;", pg("sql-createrole.html"));
  k!("NOBYPASSRLS", "Default for new roles -- explicit denial of BYPASSRLS.", "CREATE ROLE app NOBYPASSRLS;", pg("sql-createrole.html"));
  k!("INHERIT ROLE", "CREATE/ALTER ROLE ... INHERIT -- automatically gain privileges of granted roles (default).", "ALTER ROLE bob INHERIT;", pg("sql-createrole.html"));
  k!("NOINHERIT", "CREATE/ALTER ROLE ... NOINHERIT -- require explicit SET ROLE to use granted role's privileges.", "ALTER ROLE app NOINHERIT;", pg("sql-createrole.html"));
  k!("IN ROLE", "CREATE ROLE ... IN ROLE <existing>[, ...] -- add the new role as a member of these.", "CREATE ROLE alice LOGIN IN ROLE app_users;", pg("sql-createrole.html"));
  k!("IN GROUP", "Legacy synonym of IN ROLE.", "CREATE ROLE alice LOGIN IN GROUP app_users;", pg("sql-createrole.html"));
  k!("ROLE ADMIN", "CREATE ROLE ... ROLE <other> ADMIN <admin_role> -- grant ADMIN OPTION on memberships.", "CREATE ROLE alice ROLE app_users ADMIN app_admin;", pg("sql-createrole.html"));
  k!("SYSID", "Legacy SYSID <oid> -- accepted but ignored. PG assigns its own oid.", "CREATE ROLE alice SYSID 12345;", pg("sql-createrole.html"));
  // ---- Cursor extras ----
  k!("HOLD WITHOUT", "DECLARE <c> CURSOR HOLD WITHOUT HOLD -- not legal; HOLD or WITHOUT HOLD pick one.", "DECLARE c CURSOR WITHOUT HOLD FOR SELECT 1;", pg("sql-declare.html"));
  k!("TIMETZ", "Postgres-internal name for TIME WITH TIME ZONE.", "open_at TIMETZ", pg("datatype-datetime.html"));
  k!("TIMESTAMPTZ", "Postgres-internal name for TIMESTAMP WITH TIME ZONE (8-byte, recommended over TIMESTAMP for absolute moments).", "ts TIMESTAMPTZ NOT NULL", pg("datatype-datetime.html"));
  // ---- ALTER DEFAULT PRIVILEGES variations ----
  k!("ALTER DEFAULT PRIVILEGES", "ALTER DEFAULT PRIVILEGES [FOR ROLE <r>] [IN SCHEMA <s>] {GRANT|REVOKE} <priv> ON <class> TO <role>; -- pre-set privileges for future objects.", "ALTER DEFAULT PRIVILEGES FOR ROLE app IN SCHEMA public GRANT SELECT ON TABLES TO ro;", pg("sql-alterdefaultprivileges.html"));
  k!("DEFAULT PRIVILEGES FOR ROLE", "ALTER DEFAULT PRIVILEGES FOR ROLE <r> ... -- only apply when <r> creates objects.", "ALTER DEFAULT PRIVILEGES FOR ROLE app IN SCHEMA public GRANT SELECT ON TABLES TO ro;", pg("sql-alterdefaultprivileges.html"));
  k!("DEFAULT PRIVILEGES IN SCHEMA", "ALTER DEFAULT PRIVILEGES IN SCHEMA <s> ... -- scope to schema instead of every DB schema.", "ALTER DEFAULT PRIVILEGES IN SCHEMA app GRANT SELECT ON TABLES TO ro;", pg("sql-alterdefaultprivileges.html"));
  k!("ON TABLES", "ALTER DEFAULT PRIVILEGES ... GRANT/REVOKE ... ON TABLES -- future tables + views + matviews.", "ON TABLES TO ro_role", pg("sql-alterdefaultprivileges.html"));
  k!("ON SEQUENCES", "ALTER DEFAULT PRIVILEGES ... ON SEQUENCES -- future sequences.", "ON SEQUENCES TO svc_role", pg("sql-alterdefaultprivileges.html"));
  k!("ON FUNCTIONS", "ALTER DEFAULT PRIVILEGES ... ON FUNCTIONS -- future fns (covers procs too).", "ON FUNCTIONS TO svc_role", pg("sql-alterdefaultprivileges.html"));
  k!("ON ROUTINES", "ALTER DEFAULT PRIVILEGES ... ON ROUTINES -- PG11+ umbrella for fns + procs.", "ON ROUTINES TO svc_role", pg("sql-alterdefaultprivileges.html"));
  k!("ON TYPES", "ALTER DEFAULT PRIVILEGES ... ON TYPES -- future user-defined types.", "ON TYPES TO devs", pg("sql-alterdefaultprivileges.html"));
  k!("ON SCHEMAS", "ALTER DEFAULT PRIVILEGES ... ON SCHEMAS -- future schemas (PG10+).", "ON SCHEMAS TO devs", pg("sql-alterdefaultprivileges.html"));
  // ---- COMMENT ON remaining classes ----
  k!("COMMENT ON TABLESPACE", "COMMENT ON TABLESPACE <name> IS '<text>'.", "COMMENT ON TABLESPACE fastdisk IS 'NVMe pool';", pg("sql-comment.html"));
  k!("COMMENT ON ROUTINE", "COMMENT ON ROUTINE <name>(args) IS '<text>' -- works for both fns AND procs (PG11+).", "COMMENT ON ROUTINE up() IS 'startup';", pg("sql-comment.html"));
  k!("COMMENT ON OPERATOR", "COMMENT ON OPERATOR <op>(left_type, right_type) IS '<text>'.", "COMMENT ON OPERATOR ===(int, int) IS 'strict equality';", pg("sql-comment.html"));
  k!("COMMENT ON OPERATOR CLASS", "COMMENT ON OPERATOR CLASS <name> USING <am> IS '<text>'.", "COMMENT ON OPERATOR CLASS oc USING btree IS '';", pg("sql-comment.html"));
  k!("COMMENT ON OPERATOR FAMILY", "COMMENT ON OPERATOR FAMILY <name> USING <am> IS '<text>'.", "COMMENT ON OPERATOR FAMILY of USING btree IS '';", pg("sql-comment.html"));
  k!("COMMENT ON AGGREGATE", "COMMENT ON AGGREGATE <name>(args) IS '<text>'.", "COMMENT ON AGGREGATE my_sum(int) IS 'sum impl';", pg("sql-comment.html"));
  k!("COMMENT ON RULE", "COMMENT ON RULE <name> ON <table> IS '<text>'.", "COMMENT ON RULE r ON t IS 'redirect insert';", pg("sql-comment.html"));
  k!("COMMENT ON DOMAIN", "COMMENT ON DOMAIN <name> IS '<text>'.", "COMMENT ON DOMAIN positive_int IS 'check > 0';", pg("sql-comment.html"));
  k!("COMMENT ON TYPE", "COMMENT ON TYPE <name> IS '<text>'.", "COMMENT ON TYPE mood IS 'enum';", pg("sql-comment.html"));
  k!("COMMENT ON SCHEMA", "COMMENT ON SCHEMA <name> IS '<text>'.", "COMMENT ON SCHEMA public IS 'main';", pg("sql-comment.html"));
  k!("COMMENT ON CAST", "COMMENT ON CAST (<src_type> AS <tgt_type>) IS '<text>'.", "COMMENT ON CAST (text AS int) IS 'implicit';", pg("sql-comment.html"));
  k!("COMMENT ON TEXT SEARCH CONFIGURATION", "COMMENT ON TEXT SEARCH CONFIGURATION <name> IS '<text>'.", "COMMENT ON TEXT SEARCH CONFIGURATION en IS 'english';", pg("sql-comment.html"));
  k!("COMMENT ON TEXT SEARCH DICTIONARY", "COMMENT ON TEXT SEARCH DICTIONARY <name> IS '<text>'.", "COMMENT ON TEXT SEARCH DICTIONARY english_stem IS 'snowball';", pg("sql-comment.html"));
  k!("COMMENT ON TEXT SEARCH PARSER", "COMMENT ON TEXT SEARCH PARSER <name> IS '<text>'.", "COMMENT ON TEXT SEARCH PARSER my_parser IS '';", pg("sql-comment.html"));
  k!("COMMENT ON TEXT SEARCH TEMPLATE", "COMMENT ON TEXT SEARCH TEMPLATE <name> IS '<text>'.", "COMMENT ON TEXT SEARCH TEMPLATE my_tmpl IS '';", pg("sql-comment.html"));
  k!("COMMENT ON POLICY", "COMMENT ON POLICY <name> ON <table> IS '<text>'.", "COMMENT ON POLICY own_rows ON users IS 'tenant isolation';", pg("sql-comment.html"));
  k!("COMMENT ON TRANSFORM", "COMMENT ON TRANSFORM FOR <type> LANGUAGE <lang> IS '<text>'.", "COMMENT ON TRANSFORM FOR hstore LANGUAGE plperl IS 'hstore<->perl hash';", pg("sql-comment.html"));
  // ---- SET ROLE / SET SESSION AUTHORIZATION subtleties ----
  k!("SET LOCAL ROLE", "SET LOCAL ROLE <role> -- transaction-scoped role switch (reverts at COMMIT/ROLLBACK).", "BEGIN; SET LOCAL ROLE readonly; SELECT 1; COMMIT;", pg("sql-set-role.html"));
  k!("SET SESSION ROLE", "SET SESSION ROLE <role> -- session-scoped role switch (default for SET ROLE without LOCAL/SESSION).", "SET SESSION ROLE app_user;", pg("sql-set-role.html"));
  k!("SET ROLE NONE", "SET ROLE NONE -- shortcut for RESET ROLE; revert to the session's outer role.", "SET ROLE NONE;", pg("sql-set-role.html"));
  k!("RESET SESSION AUTHORIZATION", "RESET SESSION AUTHORIZATION -- revert masquerade back to the connection's authenticated role.", "RESET SESSION AUTHORIZATION;", pg("sql-reset.html"));
  k!("SET SESSION AUTHORIZATION DEFAULT", "SET SESSION AUTHORIZATION DEFAULT -- equivalent to RESET SESSION AUTHORIZATION.", "SET SESSION AUTHORIZATION DEFAULT;", pg("sql-set-session-authorization.html"));
  // ---- FOR UPDATE / SHARE chain refinements ----
  k!("SKIP LOCKED", "SELECT ... FOR UPDATE/SHARE SKIP LOCKED -- skip rows held by another tx instead of blocking; ideal for worker queues.", "SELECT id FROM jobs WHERE done = false FOR UPDATE SKIP LOCKED LIMIT 1;", pg("sql-select.html#SQL-FOR-UPDATE-SHARE"));
  k!("FOR UPDATE OF", "SELECT ... FOR UPDATE OF <alias>[, ...] -- lock only the named rels (when JOIN'd).", "SELECT * FROM users u JOIN orders o ON u.id = o.user_id FOR UPDATE OF u;", pg("sql-select.html"));
  k!("FOR NO KEY UPDATE", "SELECT ... FOR NO KEY UPDATE -- weaker than FOR UPDATE; doesn't block parallel FK checks.", "SELECT * FROM users WHERE id = 1 FOR NO KEY UPDATE;", pg("sql-select.html"));
  k!("FOR KEY SHARE", "SELECT ... FOR KEY SHARE -- weakest row lock; blocks deletes but not non-key updates. Used by FK checks.", "SELECT id FROM users WHERE id = 1 FOR KEY SHARE;", pg("sql-select.html"));
  // ---- Advisory lock family ----
  k!("pg_advisory_lock", "pg_advisory_lock(key bigint) / pg_advisory_lock(key1 int, key2 int) -> void -- session-level advisory lock; blocks until acquired.", "SELECT pg_advisory_lock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_try_advisory_lock", "pg_try_advisory_lock(...) -> boolean -- non-blocking variant; returns false if held by another session.", "SELECT pg_try_advisory_lock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_advisory_xact_lock", "pg_advisory_xact_lock(...) -> void -- transaction-scoped (auto-releases at COMMIT/ROLLBACK).", "BEGIN; SELECT pg_advisory_xact_lock(42); -- ... COMMIT;", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_try_advisory_xact_lock", "Non-blocking + xact-scoped advisory lock.", "SELECT pg_try_advisory_xact_lock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_advisory_lock_shared", "Shared (read) variant; multiple sessions can hold concurrently.", "SELECT pg_advisory_lock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_try_advisory_lock_shared", "Non-blocking shared advisory lock.", "SELECT pg_try_advisory_lock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_advisory_xact_lock_shared", "Shared + xact-scoped advisory lock.", "BEGIN; SELECT pg_advisory_xact_lock_shared(42); COMMIT;", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_advisory_unlock", "pg_advisory_unlock(key bigint) -> boolean -- release one session-level lock. Returns false if not held.", "SELECT pg_advisory_unlock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_advisory_unlock_all", "pg_advisory_unlock_all() -> void -- release every session-level advisory lock at once.", "SELECT pg_advisory_unlock_all();", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  k!("pg_advisory_unlock_shared", "Release a shared session-level lock.", "SELECT pg_advisory_unlock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  // ---- LISTEN naming rules ----
  k!("LISTEN identifier rules", "LISTEN <channel> -- channel must be a SQL identifier (alphanumeric + `_`, case-folded unless quoted). Max length matches identifier limit (typically 63 bytes).", "LISTEN order_events;", pg("sql-listen.html"));
  k!("NOTIFY identifier rules", "NOTIFY <channel> [, '<payload>'] -- channel same rules as LISTEN. Payload is text <= 8000 bytes (NAMEDATALEN-1).", "NOTIFY order_events, 'shipped:42';", pg("sql-notify.html"));
  // ---- PL/pgSQL RETURN variants ----
  k!("RETURN QUERY", "PL/pgSQL RETURN QUERY <select_stmt> -- append the SELECT's rows to a set-returning function's result set.", "RETURN QUERY SELECT * FROM users WHERE active;", pg("plpgsql-control-structures.html#PLPGSQL-STATEMENTS-RETURNING"));
  k!("RETURN NEXT", "PL/pgSQL RETURN NEXT <expr> -- append one row to a set-returning function's result.", "FOR r IN SELECT * FROM users LOOP RETURN NEXT r; END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-STATEMENTS-RETURNING"));
  k!("RETURN QUERY EXECUTE", "PL/pgSQL RETURN QUERY EXECUTE <text> [USING <params>] -- append a dynamic SQL query's result.", "RETURN QUERY EXECUTE 'SELECT * FROM ' || quote_ident(t) USING id;", pg("plpgsql-control-structures.html#PLPGSQL-STATEMENTS-RETURNING"));
  k!("FOREACH IN SLICE", "PL/pgSQL FOREACH <var> SLICE <n> IN ARRAY <arr> LOOP -- iterate by sub-array slices (n>0 means n-dim slices).", "FOREACH row SLICE 1 IN ARRAY two_d_arr LOOP ... END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-FOREACH-ARRAY"));
  k!("OPEN FOR", "PL/pgSQL OPEN <cursor> FOR <select> -- bind a query to a refcursor variable.", "OPEN c FOR SELECT * FROM users WHERE id = uid;", pg("plpgsql-cursors.html#PLPGSQL-CURSOR-OPENING"));
  k!("OPEN FOR EXECUTE", "PL/pgSQL OPEN <cursor> FOR EXECUTE <sql_text> [USING <params>] -- bind dynamic SQL to a refcursor.", "OPEN c FOR EXECUTE 'SELECT * FROM ' || quote_ident(t);", pg("plpgsql-cursors.html#PLPGSQL-CURSOR-OPENING"));
  k!("REFCURSOR", "PL/pgSQL refcursor variable -- bind via OPEN, FETCH from later; pass between functions or to the client.", "DECLARE c refcursor; BEGIN OPEN c FOR SELECT 1; ...", pg("plpgsql-cursors.html"));
  // ---- GENERATED column inheritance subtlety ----
  k!("GENERATED COLUMN INHERITANCE", "Generated columns CANNOT be overridden by child tables in inheritance. Stored expressions are recomputed per row.", "-- VIRTUAL re-evaluates per read; STORED writes once at INSERT/UPDATE", pg("ddl-generated-columns.html"));
  k!("ALWAYS AS GENERATED", "Misnomer alias seen in some DBs -- in PG the correct order is `GENERATED ALWAYS AS (<expr>) {STORED|VIRTUAL}`.", "GENERATED ALWAYS AS (a*2) STORED", pg("ddl-generated-columns.html"));
  // ---- Explicit lock modes ----
  k!("ACCESS SHARE", "LOCK TABLE t IN ACCESS SHARE MODE -- weakest mode; only conflicts with ACCESS EXCLUSIVE. Acquired by every SELECT.", "LOCK TABLE t IN ACCESS SHARE MODE;", pg("explicit-locking.html#LOCKING-TABLES"));
  k!("ROW SHARE", "LOCK TABLE t IN ROW SHARE MODE -- acquired by SELECT FOR UPDATE / FOR SHARE.", "LOCK TABLE t IN ROW SHARE MODE;", pg("explicit-locking.html#LOCKING-TABLES"));
  k!("ROW EXCLUSIVE", "LOCK TABLE t IN ROW EXCLUSIVE MODE -- acquired by INSERT/UPDATE/DELETE/MERGE.", "LOCK TABLE t IN ROW EXCLUSIVE MODE;", pg("explicit-locking.html#LOCKING-TABLES"));
  k!("SHARE UPDATE EXCLUSIVE", "LOCK TABLE t IN SHARE UPDATE EXCLUSIVE MODE -- VACUUM/ANALYZE/CREATE INDEX CONCURRENTLY/ALTER TABLE VALIDATE.", "LOCK TABLE t IN SHARE UPDATE EXCLUSIVE MODE;", pg("explicit-locking.html#LOCKING-TABLES"));
  k!("SHARE ROW EXCLUSIVE", "LOCK TABLE t IN SHARE ROW EXCLUSIVE MODE -- blocks concurrent writers AND blocks SHARE-mode readers from acquiring it.", "LOCK TABLE t IN SHARE ROW EXCLUSIVE MODE;", pg("explicit-locking.html#LOCKING-TABLES"));
  k!("ACCESS EXCLUSIVE", "LOCK TABLE t IN ACCESS EXCLUSIVE MODE -- strongest; conflicts with every other mode. Acquired by ALTER/DROP/TRUNCATE/REINDEX (non-concurrent), VACUUM FULL, CLUSTER.", "LOCK TABLE t IN ACCESS EXCLUSIVE MODE;", pg("explicit-locking.html#LOCKING-TABLES"));
  k!("LOCK TABLE IN", "LOCK TABLE <t> IN <mode> MODE [NOWAIT] -- acquire an explicit relation lock.", "LOCK TABLE big IN ACCESS EXCLUSIVE MODE NOWAIT;", pg("sql-lock.html"));
  k!("NOWAIT", "Lock acquisition modifier: don't block; raise 55P03 if the lock isn't immediately available.", "LOCK TABLE t IN ACCESS EXCLUSIVE MODE NOWAIT;", pg("sql-lock.html"));
  // ---- Transaction modes ----
  k!("ISOLATION LEVEL", "BEGIN ISOLATION LEVEL { READ UNCOMMITTED | READ COMMITTED | REPEATABLE READ | SERIALIZABLE }", "BEGIN ISOLATION LEVEL REPEATABLE READ;", pg("sql-set-transaction.html"));
  k!("READ COMMITTED", "Default PG isolation: each statement sees a fresh snapshot. No phantom protection across statements.", "BEGIN ISOLATION LEVEL READ COMMITTED;", pg("transaction-iso.html#XACT-READ-COMMITTED"));
  k!("REPEATABLE READ", "PG isolation: each transaction sees a single snapshot at start; rejects concurrent updates with 40001.", "BEGIN ISOLATION LEVEL REPEATABLE READ;", pg("transaction-iso.html#XACT-REPEATABLE-READ"));
  k!("READ UNCOMMITTED", "Accepted spelling, treated as READ COMMITTED in PG -- there are no dirty reads.", "BEGIN ISOLATION LEVEL READ UNCOMMITTED;", pg("transaction-iso.html"));
  k!("DEFERRABLE", "Transaction mode: DEFERRABLE -- valid only with ISOLATION LEVEL SERIALIZABLE READ ONLY; the txn may wait at start instead of failing with 40001.", "BEGIN ISOLATION LEVEL SERIALIZABLE READ ONLY DEFERRABLE;", pg("sql-set-transaction.html"));
  k!("NOT DEFERRABLE", "Transaction mode: NOT DEFERRABLE (default).", "BEGIN NOT DEFERRABLE;", pg("sql-set-transaction.html"));
  // ---- VACUUM new options (PG17+) ----
  k!("ONLY_DATABASE_STATS", "VACUUM (ONLY_DATABASE_STATS) -- skip the per-relation pass and only refresh per-DB stats (PG17+).", "VACUUM (ONLY_DATABASE_STATS);", pg("sql-vacuum.html"));
  k!("SKIP_DATABASE_STATS", "VACUUM (SKIP_DATABASE_STATS) -- skip the per-DB stats refresh (PG17+); useful from cron loops.", "VACUUM (SKIP_DATABASE_STATS);", pg("sql-vacuum.html"));
  k!("BUFFER_USAGE_LIMIT", "VACUUM (BUFFER_USAGE_LIMIT '<size>') -- per-vacuum shared-buffer ring cap (PG16+).", "VACUUM (BUFFER_USAGE_LIMIT '32MB') big;", pg("sql-vacuum.html"));
  k!("PROCESS_MAIN", "VACUUM (PROCESS_MAIN [true|false]) -- skip the main relation pass and only vacuum the TOAST table (PG16+).", "VACUUM (PROCESS_MAIN false) toast_heavy;", pg("sql-vacuum.html"));
  k!("PROCESS_TOAST", "VACUUM (PROCESS_TOAST [true|false]) -- skip the TOAST companion table.", "VACUUM (PROCESS_TOAST false) t;", pg("sql-vacuum.html"));
  k!("INDEX_CLEANUP", "VACUUM (INDEX_CLEANUP { AUTO | ON | OFF }) -- override index cleanup heuristic.", "VACUUM (INDEX_CLEANUP off) big;", pg("sql-vacuum.html"));
  k!("DISABLE_PAGE_SKIPPING", "VACUUM (DISABLE_PAGE_SKIPPING) -- visit every page even when the visibility map says skip.", "VACUUM (DISABLE_PAGE_SKIPPING) t;", pg("sql-vacuum.html"));
  // ---- DECLARE cursor extras ----
  k!("NO SCROLL", "DECLARE <c> NO SCROLL CURSOR ... -- forbid backwards / random fetches; planner can pick streaming-only plans.", "DECLARE c NO SCROLL CURSOR FOR SELECT id FROM big;", pg("sql-declare.html"));
  k!("WITHOUT HOLD", "DECLARE <c> CURSOR WITHOUT HOLD -- closes at COMMIT (default).", "DECLARE c CURSOR WITHOUT HOLD FOR SELECT 1;", pg("sql-declare.html"));
  k!("BINARY CURSOR", "DECLARE <c> BINARY CURSOR ... -- returns rows in PG binary wire format; faster for protocol clients that decode it.", "DECLARE c BINARY CURSOR FOR SELECT 1;", pg("sql-declare.html"));
  k!("DECLARE BINARY", "Prefix used in `DECLARE <c> BINARY CURSOR FOR ...`.", "DECLARE c BINARY CURSOR FOR SELECT id FROM t;", pg("sql-declare.html"));
  // ---- FETCH / MOVE direction completeness ----
  k!("FETCH FORWARD", "FETCH FORWARD [<n> | ALL] FROM <c> -- read forwards (default direction).", "FETCH FORWARD 100 FROM c;", pg("sql-fetch.html"));
  k!("FETCH BACKWARD", "FETCH BACKWARD [<n> | ALL] FROM <c> -- read backwards; requires a SCROLL cursor.", "FETCH BACKWARD 50 FROM c;", pg("sql-fetch.html"));
  k!("FETCH ABSOLUTE", "FETCH ABSOLUTE <n> FROM <c> -- position the cursor at row <n> (1-based; negative counts from end). SCROLL only.", "FETCH ABSOLUTE 1 FROM c;", pg("sql-fetch.html"));
  k!("FETCH RELATIVE", "FETCH RELATIVE <n> FROM <c> -- move <n> rows from current. SCROLL only.", "FETCH RELATIVE -3 FROM c;", pg("sql-fetch.html"));
  k!("FETCH FIRST FROM", "FETCH FIRST FROM <c> -- position at the first row and fetch it.", "FETCH FIRST FROM c;", pg("sql-fetch.html"));
  k!("FETCH LAST FROM", "FETCH LAST FROM <c> -- position at the last row and fetch it.", "FETCH LAST FROM c;", pg("sql-fetch.html"));
  k!("FETCH PRIOR", "FETCH PRIOR FROM <c> -- move one row back and return it.", "FETCH PRIOR FROM c;", pg("sql-fetch.html"));
  k!("FETCH ALL", "FETCH ALL FROM <c> -- return every remaining row.", "FETCH ALL FROM c;", pg("sql-fetch.html"));
  k!("MOVE FORWARD", "MOVE FORWARD [<n> | ALL] IN <c> -- advance without returning rows.", "MOVE FORWARD 100 IN c;", pg("sql-move.html"));
  k!("MOVE BACKWARD", "MOVE BACKWARD [<n> | ALL] IN <c> -- step back without returning rows. SCROLL only.", "MOVE BACKWARD 10 IN c;", pg("sql-move.html"));
  k!("MOVE ABSOLUTE", "MOVE ABSOLUTE <n> IN <c> -- seek to row <n> without returning.", "MOVE ABSOLUTE 0 IN c;", pg("sql-move.html"));
  k!("MOVE RELATIVE", "MOVE RELATIVE <n> IN <c> -- seek <n> rows from current without returning.", "MOVE RELATIVE 5 IN c;", pg("sql-move.html"));
  k!("MOVE FIRST", "MOVE FIRST IN <c> -- seek to the first row.", "MOVE FIRST IN c;", pg("sql-move.html"));
  k!("MOVE LAST", "MOVE LAST IN <c> -- seek past the last row.", "MOVE LAST IN c;", pg("sql-move.html"));
  k!("MOVE NEXT", "MOVE NEXT IN <c> -- step forward one row.", "MOVE NEXT IN c;", pg("sql-move.html"));
  k!("MOVE PRIOR", "MOVE PRIOR IN <c> -- step backward one row.", "MOVE PRIOR IN c;", pg("sql-move.html"));
  k!("MOVE ALL", "MOVE ALL IN <c> -- seek to one past the last row (equivalent to MOVE FORWARD ALL).", "MOVE ALL IN c;", pg("sql-move.html"));
  // ---- COPY remaining surface ----
  k!("COPY FROM STDIN", "COPY <t> [(cols)] FROM STDIN [WITH (...)] -- stream client data into the table.", "COPY users (id, email) FROM STDIN WITH (FORMAT csv, HEADER);", pg("sql-copy.html"));
  k!("COPY TO STDOUT", "COPY <t-or-query> TO STDOUT [WITH (...)] -- stream rows out to the client.", "COPY (SELECT id FROM users) TO STDOUT;", pg("sql-copy.html"));
  k!("COPY FROM PROGRAM", "COPY <t> FROM PROGRAM '<shell>' [WITH (...)] -- pipe a shell command's stdout. Super-user only (security risk).", "COPY users FROM PROGRAM 'gunzip -c /tmp/users.csv.gz' WITH (FORMAT csv);", pg("sql-copy.html"));
  k!("COPY TO PROGRAM", "COPY <t-or-query> TO PROGRAM '<shell>' [WITH (...)] -- pipe rows to a shell command. Super-user only.", "COPY (SELECT id FROM users) TO PROGRAM 'gzip > /tmp/ids.gz';", pg("sql-copy.html"));
  // ---- psql meta-commands (NOT SQL) ----
  k!("\\d", "psql meta-command (not server SQL): describe an object. Variants: \\dt (tables), \\di (indexes), \\dv (views), \\df (functions), \\dn (schemas), \\du (roles), \\l (databases), \\dx (extensions).", "\\dt+ public.*", pg("app-psql.html"));
  k!("\\copy", "psql meta-command (client-side COPY) -- does NOT need superuser; reads/writes a file local to the psql process.", "\\copy users (id, email) FROM 'users.csv' WITH (FORMAT csv, HEADER);", pg("app-psql.html"));
  // ---- PL/pgSQL control structures ----
  k!("FOREACH", "PL/pgSQL FOREACH <var> [SLICE <n>] IN ARRAY <arr> LOOP ... END LOOP -- iterate over array elements (or sub-slices).", "FOREACH x IN ARRAY ids LOOP RAISE NOTICE '%', x; END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-FOREACH-ARRAY"));
  k!("EXIT WHEN", "PL/pgSQL EXIT WHEN <pred> -- break out of the innermost loop when pred is true.", "EXIT WHEN cnt > 100;", pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS"));
  k!("EXIT LOOP", "PL/pgSQL EXIT [<label>] -- break out of a labelled or innermost loop.", "<<outer>> LOOP EXIT outer; END LOOP outer;", pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS"));
  k!("CONTINUE WHEN", "PL/pgSQL CONTINUE WHEN <pred> -- skip to next iteration when pred is true.", "CONTINUE WHEN id IS NULL;", pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS"));
  k!("CONTINUE LOOP", "PL/pgSQL CONTINUE [<label>] -- jump to next iteration of a labelled or innermost loop.", "CONTINUE outer;", pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS"));
  k!("ASSERT", "PL/pgSQL ASSERT <pred> [, <msg>] -- raise an assertion-failure exception if pred is false. Honoured only when plpgsql.check_asserts = on.", "ASSERT cnt >= 0, 'cnt must be non-negative';", pg("plpgsql-errors-and-messages.html#PLPGSQL-STATEMENTS-ASSERT"));
  k!("GET DIAGNOSTICS", "PL/pgSQL GET [CURRENT] DIAGNOSTICS <var> = ROW_COUNT, <var> = PG_CONTEXT, ... -- pull stats about the last command.", "GET DIAGNOSTICS n = ROW_COUNT;", pg("plpgsql-statements.html#PLPGSQL-STATEMENTS-DIAGNOSTICS"));
  k!("GET STACKED DIAGNOSTICS", "PL/pgSQL GET STACKED DIAGNOSTICS <var> = MESSAGE_TEXT, ... -- read fields from the active exception (only inside an EXCEPTION block).", "GET STACKED DIAGNOSTICS msg = MESSAGE_TEXT;", pg("plpgsql-control-structures.html#PLPGSQL-EXCEPTION-DIAGNOSTICS"));
  k!("EXCEPTION WHEN", "PL/pgSQL block EXCEPTION WHEN <condition> [OR <c>...] THEN ... -- catch matching SQLSTATE / named errors.", "EXCEPTION WHEN unique_violation THEN RAISE NOTICE 'dup'; END;", pg("plpgsql-control-structures.html#PLPGSQL-ERROR-TRAPPING"));
  k!("WHEN OTHERS", "PL/pgSQL exception handler that matches any non-already-matched error. Use sparingly -- masks programmer errors.", "EXCEPTION WHEN OTHERS THEN RAISE; END;", pg("plpgsql-control-structures.html#PLPGSQL-ERROR-TRAPPING"));
  k!("RAISE USING", "PL/pgSQL RAISE [level] '<msg>' USING ERRCODE = '<state>', DETAIL = '...', HINT = '...', ... -- attach structured exception fields.", "RAISE EXCEPTION 'bad row %', id USING ERRCODE = 'P0001', HINT = 'reload config';", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE DEBUG", "PL/pgSQL RAISE DEBUG '<msg>' -- developer log level (controlled by client_min_messages).", "RAISE DEBUG 'in helper, x=%', x;", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE LOG", "PL/pgSQL RAISE LOG '<msg>' -- write to server log only (never to client).", "RAISE LOG 'background job ran';", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE INFO", "PL/pgSQL RAISE INFO '<msg>' -- always sent to the client.", "RAISE INFO 'progress: %%', pct;", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE WARNING", "PL/pgSQL RAISE WARNING '<msg>' -- always sent to the client at WARNING level.", "RAISE WARNING 'slow path triggered';", pg("plpgsql-errors-and-messages.html"));
  k!("PERFORM", "PL/pgSQL PERFORM <query> -- run a query and discard its result; required when a SELECT is used for side effects only.", "PERFORM pg_notify('chan', 'hi');", pg("plpgsql-statements.html#PLPGSQL-STATEMENTS-SQL-NORESULT"));
  // ---- Savepoint + prepared transaction ----
  k!("SAVEPOINT", "SAVEPOINT <name> -- mark a sub-transaction point; later commands can be undone with ROLLBACK TO SAVEPOINT <name>.", "SAVEPOINT before_risk; -- ... -- ROLLBACK TO SAVEPOINT before_risk;", pg("sql-savepoint.html"));
  k!("PREPARE TRANSACTION", "PREPARE TRANSACTION '<gid>' -- 2PC step 1: persist the transaction so it can be committed/rolled back by gid later. Requires max_prepared_transactions > 0.", "PREPARE TRANSACTION 'tx-42';", pg("sql-prepare-transaction.html"));
  k!("COMMIT PREPARED", "COMMIT PREPARED '<gid>' -- 2PC step 2: commit a previously PREPARE'd transaction.", "COMMIT PREPARED 'tx-42';", pg("sql-commit-prepared.html"));
  k!("ROLLBACK PREPARED", "ROLLBACK PREPARED '<gid>' -- 2PC abort: discard a previously PREPARE'd transaction without committing.", "ROLLBACK PREPARED 'tx-42';", pg("sql-rollback-prepared.html"));
  // ---- Loop / block forms ----
  k!("LOOP", "PL/pgSQL LOOP ... END LOOP -- unconditional loop; exit with EXIT [WHEN <pred>].", "LOOP EXIT WHEN cnt = 0; cnt := cnt - 1; END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS"));
  k!("WHILE LOOP", "PL/pgSQL WHILE <pred> LOOP ... END LOOP.", "WHILE cnt > 0 LOOP cnt := cnt - 1; END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-CONTROL-STRUCTURES-LOOPS"));
  k!("FOR IN REVERSE", "PL/pgSQL FOR <var> IN REVERSE <lo>..<hi> LOOP ... END LOOP -- iterate in descending order.", "FOR i IN REVERSE 10..1 LOOP ... END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-INTEGER-FOR"));
  k!("FOR IN", "PL/pgSQL FOR <var> IN <query> LOOP ... END LOOP -- iterate over a query result; <var> is a record / row variable.", "FOR r IN SELECT * FROM t LOOP RAISE NOTICE '%', r.id; END LOOP;", pg("plpgsql-control-structures.html#PLPGSQL-RECORDS-ITERATING"));
  // ---- Named exception conditions (PL/pgSQL EXCEPTION WHEN) ----
  k!("unique_violation", "SQLSTATE 23505 -- raised when a UNIQUE or PRIMARY KEY constraint is violated. Catch in PL/pgSQL: `EXCEPTION WHEN unique_violation THEN ...`.", "EXCEPTION WHEN unique_violation THEN ...", pg("errcodes-appendix.html"));
  k!("foreign_key_violation", "SQLSTATE 23503 -- raised when a FOREIGN KEY constraint is violated (parent row missing / dependent row exists).", "EXCEPTION WHEN foreign_key_violation THEN ...", pg("errcodes-appendix.html"));
  k!("not_null_violation", "SQLSTATE 23502 -- raised when a NOT NULL column gets NULL.", "EXCEPTION WHEN not_null_violation THEN ...", pg("errcodes-appendix.html"));
  k!("check_violation", "SQLSTATE 23514 -- raised when a CHECK constraint fails.", "EXCEPTION WHEN check_violation THEN ...", pg("errcodes-appendix.html"));
  k!("exclusion_violation", "SQLSTATE 23P01 -- raised when an EXCLUDE constraint matches.", "EXCEPTION WHEN exclusion_violation THEN ...", pg("errcodes-appendix.html"));
  k!("restrict_violation", "SQLSTATE 23001 -- raised by RESTRICT actions in foreign-key delete/update.", "EXCEPTION WHEN restrict_violation THEN ...", pg("errcodes-appendix.html"));
  k!("integrity_constraint_violation", "SQLSTATE 23000 -- generic parent for all 23xxx integrity violations; catches every named child.", "EXCEPTION WHEN integrity_constraint_violation THEN ...", pg("errcodes-appendix.html"));
  k!("deadlock_detected", "SQLSTATE 40P01 -- raised when the deadlock detector aborts this txn.", "EXCEPTION WHEN deadlock_detected THEN ...", pg("errcodes-appendix.html"));
  k!("serialization_failure", "SQLSTATE 40001 -- SSI/REPEATABLE READ conflict. Always retry from the application.", "EXCEPTION WHEN serialization_failure THEN ...", pg("errcodes-appendix.html"));
  k!("lock_not_available", "SQLSTATE 55P03 -- raised when NOWAIT couldn't acquire a lock.", "EXCEPTION WHEN lock_not_available THEN ...", pg("errcodes-appendix.html"));
  k!("no_data_found", "SQLSTATE P0002 -- raised by SELECT INTO STRICT when no row matches.", "EXCEPTION WHEN no_data_found THEN ...", pg("errcodes-appendix.html"));
  k!("too_many_rows", "SQLSTATE P0003 -- raised by SELECT INTO STRICT when more than one row matches.", "EXCEPTION WHEN too_many_rows THEN ...", pg("errcodes-appendix.html"));
  k!("query_canceled", "SQLSTATE 57014 -- raised when the query is cancelled (statement_timeout, pg_cancel_backend, client interrupt).", "EXCEPTION WHEN query_canceled THEN ...", pg("errcodes-appendix.html"));
  k!("admin_shutdown", "SQLSTATE 57P01 -- raised when the cluster shuts down while the session is connected.", "EXCEPTION WHEN admin_shutdown THEN ...", pg("errcodes-appendix.html"));
  k!("idle_in_transaction_session_timeout", "SQLSTATE 25P03 -- raised when idle_in_transaction_session_timeout fires.", "EXCEPTION WHEN idle_in_transaction_session_timeout THEN ...", pg("errcodes-appendix.html"));
  k!("invalid_text_representation", "SQLSTATE 22P02 -- value couldn't be parsed for its target type (e.g. 'abc' for int).", "EXCEPTION WHEN invalid_text_representation THEN ...", pg("errcodes-appendix.html"));
  k!("division_by_zero", "SQLSTATE 22012 -- arithmetic division by zero.", "EXCEPTION WHEN division_by_zero THEN ...", pg("errcodes-appendix.html"));
  k!("numeric_value_out_of_range", "SQLSTATE 22003 -- value won't fit the target numeric type (overflow).", "EXCEPTION WHEN numeric_value_out_of_range THEN ...", pg("errcodes-appendix.html"));
  k!("string_data_right_truncation", "SQLSTATE 22001 -- VARCHAR(n) / CHAR(n) overflow.", "EXCEPTION WHEN string_data_right_truncation THEN ...", pg("errcodes-appendix.html"));
  k!("invalid_authorization_specification", "SQLSTATE 28000 -- generic auth failure (wrong role/password).", "EXCEPTION WHEN invalid_authorization_specification THEN ...", pg("errcodes-appendix.html"));
  k!("insufficient_privilege", "SQLSTATE 42501 -- the role lacks the required privilege.", "EXCEPTION WHEN insufficient_privilege THEN ...", pg("errcodes-appendix.html"));
  k!("undefined_table", "SQLSTATE 42P01 -- table or view does not exist.", "EXCEPTION WHEN undefined_table THEN ...", pg("errcodes-appendix.html"));
  k!("undefined_column", "SQLSTATE 42703 -- referenced column does not exist.", "EXCEPTION WHEN undefined_column THEN ...", pg("errcodes-appendix.html"));
  k!("undefined_function", "SQLSTATE 42883 -- function with the given signature does not exist.", "EXCEPTION WHEN undefined_function THEN ...", pg("errcodes-appendix.html"));
  k!("duplicate_table", "SQLSTATE 42P07 -- CREATE TABLE / SELECT INTO target already exists.", "EXCEPTION WHEN duplicate_table THEN ...", pg("errcodes-appendix.html"));
  k!("connection_failure", "SQLSTATE 08006 -- generic connection-related error.", "EXCEPTION WHEN connection_failure THEN ...", pg("errcodes-appendix.html"));
  k!("disk_full", "SQLSTATE 53100 -- out of disk space.", "EXCEPTION WHEN disk_full THEN ...", pg("errcodes-appendix.html"));
  k!("data_exception", "SQLSTATE 22000 -- generic parent for 22xxx data-format errors.", "EXCEPTION WHEN data_exception THEN ...", pg("errcodes-appendix.html"));
  k!("syntax_error", "SQLSTATE 42601 -- generic SQL syntax error (rarely caught -- usually the goal is to never see it).", "EXCEPTION WHEN syntax_error THEN ...", pg("errcodes-appendix.html"));
  k!("plpgsql_error", "SQLSTATE P0001 -- raised by `RAISE` with no explicit ERRCODE; the default class for user RAISE.", "EXCEPTION WHEN plpgsql_error THEN ...", pg("errcodes-appendix.html"));
  k!("raise_exception", "SQLSTATE P0001 alias used by some clients -- same as plpgsql_error class.", "EXCEPTION WHEN raise_exception THEN ...", pg("errcodes-appendix.html"));
  k!("SQLSTATE", "PL/pgSQL EXCEPTION WHEN SQLSTATE '<5-char>' THEN ... -- match a raw SQLSTATE instead of a named condition.", "EXCEPTION WHEN SQLSTATE '40001' THEN PERFORM pg_sleep(0.1);", pg("plpgsql-control-structures.html#PLPGSQL-ERROR-TRAPPING"));
  k!("NULLS NOT DISTINCT", "NULLS NOT DISTINCT -- UNIQUE / index option treating NULLs as equal (PG15+).", "CREATE UNIQUE INDEX ... NULLS NOT DISTINCT", pg("sql-createindex.html"));
  k!("DEFAULTS", "INCLUDING DEFAULTS / EXCLUDING DEFAULTS -- LIKE table option.", "CREATE TABLE c (LIKE p INCLUDING DEFAULTS);", pg("sql-createtable.html"));
  k!("INCLUDING", "INCLUDING { ALL | DEFAULTS | CONSTRAINTS | INDEXES | STORAGE | COMMENTS | COMPRESSION | STATISTICS | IDENTITY | GENERATED } -- LIKE inheritance.", "LIKE parent INCLUDING ALL", pg("sql-createtable.html"));
  k!("EXCLUDING", "EXCLUDING { DEFAULTS | CONSTRAINTS | INDEXES | STORAGE | COMMENTS | COMPRESSION | STATISTICS | IDENTITY | GENERATED } -- LIKE inheritance.", "LIKE parent EXCLUDING INDEXES", pg("sql-createtable.html"));
  k!("ON_ERROR", "COPY (... ON_ERROR { STOP | IGNORE }) -- per-row error handling (PG17+).", "COPY t FROM 'p.csv' (ON_ERROR IGNORE);", pg("sql-copy.html"));
  k!("LOG_VERBOSITY", "COPY (... LOG_VERBOSITY { DEFAULT | VERBOSE }) -- error log detail (PG17+).", "COPY t FROM 'p.csv' (LOG_VERBOSITY VERBOSE);", pg("sql-copy.html"));
  k!("BUFFER_USAGE_LIMIT", "VACUUM/ANALYZE BUFFER_USAGE_LIMIT '<size>' (PG16+).", "VACUUM (BUFFER_USAGE_LIMIT '64 MB') t;", pg("sql-vacuum.html"));
  k!("PROCESS_TOAST", "VACUUM PROCESS_TOAST {true|false} (PG14+).", "VACUUM (PROCESS_TOAST false) t;", pg("sql-vacuum.html"));
  k!("PROCESS_MAIN", "VACUUM PROCESS_MAIN {true|false} (PG16+).", "VACUUM (PROCESS_MAIN false) t;", pg("sql-vacuum.html"));


  // ---- round 154 multi-word PG predicates / clauses ----
  k!("DISTINCT FROM", "DISTINCT FROM <expr> -- NULL-aware inequality predicate.", "WHERE a IS DISTINCT FROM b", pg("functions-comparison.html"));
  k!("IS DISTINCT FROM", "<a> IS DISTINCT FROM <b> -- true when a != b, treating NULL as a value.", "WHERE old.x IS DISTINCT FROM new.x", pg("functions-comparison.html"));
  k!("IS NOT DISTINCT FROM", "<a> IS NOT DISTINCT FROM <b> -- NULL-aware equality.", "WHERE OLD.x IS NOT DISTINCT FROM NEW.x", pg("functions-comparison.html"));
  k!("GENERATED ALWAYS", "GENERATED ALWAYS AS (<expr>) STORED -- computed column.", "amount_cents int GENERATED ALWAYS AS (amount * 100) STORED", pg("ddl-generated-columns.html"));
  k!("GENERATED BY DEFAULT", "GENERATED BY DEFAULT AS IDENTITY -- identity column, user value allowed.", "id int GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY", pg("sql-createtable.html"));
  k!("OVERRIDING SYSTEM VALUE", "INSERT ... OVERRIDING SYSTEM VALUE -- replace identity column value.", "INSERT INTO t OVERRIDING SYSTEM VALUE VALUES (...);", pg("sql-insert.html"));
  k!("OVERRIDING USER VALUE", "INSERT ... OVERRIDING USER VALUE -- accept user value for identity.", "INSERT INTO t OVERRIDING USER VALUE VALUES (...);", pg("sql-insert.html"));
  k!("ON UPDATE", "FK ON UPDATE { NO ACTION | CASCADE | SET NULL | SET DEFAULT | RESTRICT }.", "REFERENCES users(id) ON UPDATE CASCADE", pg("sql-createtable.html"));
  k!("ON DELETE", "FK ON DELETE { NO ACTION | CASCADE | SET NULL | SET DEFAULT | RESTRICT }.", "REFERENCES users(id) ON DELETE SET NULL", pg("sql-createtable.html"));
  k!("FORCE NOT NULL", "COPY ... FORCE NOT NULL (cols) -- treat empty as text, not NULL.", "COPY t FROM '...' CSV FORCE NOT NULL (col1, col2);", pg("sql-copy.html"));
  k!("FORCE QUOTE", "COPY ... FORCE QUOTE { * | (cols) } -- always quote these columns.", "COPY t TO '...' CSV FORCE QUOTE *;", pg("sql-copy.html"));


  // ---- round 155 multi-word DDL/aggregate kws ----
  k!("WITHIN GROUP", "WITHIN GROUP (ORDER BY <key>) -- ordered-set aggregates (e.g. percentile_cont).", "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY salary) FROM emp;", pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE"));
  k!("DROP DEFAULT", "ALTER COLUMN ... DROP DEFAULT -- remove a column's default expression.", "ALTER TABLE t ALTER COLUMN c DROP DEFAULT;", pg("sql-altertable.html"));
  k!("ADD COLUMN", "ALTER TABLE ... ADD COLUMN <name> <type> [constraints].", "ALTER TABLE t ADD COLUMN email text NOT NULL;", pg("sql-altertable.html"));
  k!("DROP COLUMN", "ALTER TABLE ... DROP COLUMN <name> [CASCADE|RESTRICT].", "ALTER TABLE t DROP COLUMN deleted_at;", pg("sql-altertable.html"));
  k!("RENAME COLUMN", "ALTER TABLE ... RENAME COLUMN <old> TO <new>.", "ALTER TABLE t RENAME COLUMN c TO created_at;", pg("sql-altertable.html"));
  k!("RENAME TO", "ALTER <object> ... RENAME TO <new_name> -- works on tables, indexes, sequences, views, schemas, roles, etc.", "ALTER TABLE users RENAME TO accounts;", pg("sql-altertable.html"));
  k!("ALTER COLUMN", "ALTER TABLE ... ALTER COLUMN <name> { TYPE ... | SET DEFAULT ... | DROP DEFAULT | SET NOT NULL | DROP NOT NULL | SET STATISTICS ... | SET STORAGE ... | RESET (...) }.", "ALTER TABLE t ALTER COLUMN c TYPE text USING c::text;", pg("sql-altertable.html"));
  k!("SET TABLESPACE", "ALTER TABLE/INDEX ... SET TABLESPACE <name> -- physical move.", "ALTER TABLE big SET TABLESPACE archive;", pg("sql-altertable.html"));
  k!("FOR EACH ROW", "CREATE TRIGGER ... FOR EACH ROW -- per-row firing.", "CREATE TRIGGER trg BEFORE UPDATE ON t FOR EACH ROW EXECUTE FUNCTION upd();", pg("sql-createtrigger.html"));
  k!("FOR EACH STATEMENT", "CREATE TRIGGER ... FOR EACH STATEMENT -- one fire per statement (default).", "CREATE TRIGGER trg AFTER TRUNCATE ON t FOR EACH STATEMENT EXECUTE FUNCTION log();", pg("sql-createtrigger.html"));
  k!("EXECUTE FUNCTION", "TRIGGER body: EXECUTE FUNCTION <fn>(<args>) -- preferred over EXECUTE PROCEDURE (PG11+).", "EXECUTE FUNCTION set_updated_at()", pg("sql-createtrigger.html"));
  k!("EXECUTE PROCEDURE", "TRIGGER body: EXECUTE PROCEDURE <fn>(<args>) -- legacy spelling.", "EXECUTE PROCEDURE set_updated_at()", pg("sql-createtrigger.html"));
  k!("NULLS DISTINCT", "UNIQUE / INDEX option: NULLS DISTINCT -- multiple NULLs do not conflict (default).", "CREATE UNIQUE INDEX ix ON t(col) NULLS DISTINCT;", pg("sql-createindex.html"));


  // ---- round 156 multi-word kws ----
  k!("FETCH FIRST", "SELECT ... FETCH FIRST <n> ROWS { ONLY | WITH TIES } -- SQL-standard limit.", "SELECT * FROM t ORDER BY id FETCH FIRST 10 ROWS ONLY;", pg("sql-select.html"));
  k!("FETCH NEXT", "Synonym of FETCH FIRST in SELECT.", "SELECT * FROM t ORDER BY id FETCH NEXT 10 ROWS ONLY;", pg("sql-select.html"));
  k!("NO INHERIT", "ALTER TABLE ... NO INHERIT <parent> -- remove inheritance link.", "ALTER TABLE child NO INHERIT parent;", pg("sql-altertable.html"));
  k!("ON COMMIT", "CREATE TEMP TABLE ... ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP } -- temp-table lifetime.", "CREATE TEMP TABLE staging (...) ON COMMIT DROP;", pg("sql-createtable.html"));
  k!("ROWS FROM", "FROM ROWS FROM (fn1(...), fn2(...)) -- multi-function FROM (PG9.4+).", "SELECT * FROM ROWS FROM (unnest(a), unnest(b));", pg("queries-table-expressions.html"));
  k!("WITH HOLD", "DECLARE ... CURSOR WITH HOLD -- cursor survives COMMIT.", "DECLARE c CURSOR WITH HOLD FOR SELECT 1;", pg("sql-declare.html"));
  k!("WITH GRANT OPTION", "GRANT ... TO <role> WITH GRANT OPTION -- grantee can re-grant.", "GRANT SELECT ON t TO alice WITH GRANT OPTION;", pg("sql-grant.html"));
  k!("WITH ADMIN OPTION", "GRANT <role> TO <member> WITH ADMIN OPTION -- can add other members.", "GRANT admins TO alice WITH ADMIN OPTION;", pg("sql-grant.html"));
  k!("TIME ZONE", "AT TIME ZONE <name> -- shift between zoned/unzoned timestamp.", "SELECT ts AT TIME ZONE 'UTC' AT TIME ZONE 'Europe/Berlin';", pg("functions-datetime.html"));
  k!("DEFAULT VALUES", "INSERT INTO t DEFAULT VALUES -- every column takes its default.", "INSERT INTO audit DEFAULT VALUES;", pg("sql-insert.html"));
  k!("WITH DATA", "CREATE TABLE AS ... WITH DATA / CREATE MATERIALIZED VIEW ... WITH DATA -- populate immediately (default).", "CREATE MATERIALIZED VIEW m AS SELECT 1 WITH DATA;", pg("sql-creatematerializedview.html"));
  k!("WITH NO DATA", "CREATE TABLE AS ... WITH NO DATA -- create empty, populate later.", "CREATE TABLE snap AS SELECT * FROM users WITH NO DATA;", pg("sql-createtableas.html"));
  k!("USING INDEX", "ADD CONSTRAINT ... USING INDEX <existing_index> -- back a PK/UNIQUE with an existing index.", "ALTER TABLE t ADD CONSTRAINT pk_t PRIMARY KEY USING INDEX ix_t_id;", pg("sql-altertable.html"));
  k!("USING INDEX TABLESPACE", "ADD CONSTRAINT ... USING INDEX TABLESPACE <space> -- create the backing index in a specific tablespace.", "ADD CONSTRAINT u UNIQUE (col) USING INDEX TABLESPACE archive", pg("sql-createtable.html"));
  k!("TABLES IN SCHEMA", "FOR / ADD / SET / DROP TABLES IN SCHEMA <name>[, ...] -- whole-schema publication target (PG15+).", "CREATE PUBLICATION p FOR TABLES IN SCHEMA public;", pg("sql-createpublication.html"));


  // ---- round 157 multi-word FK/RAISE/REPLICA kws ----
  k!("ON DELETE CASCADE", "FK ON DELETE CASCADE -- delete child rows when parent goes.", "REFERENCES users(id) ON DELETE CASCADE", pg("ddl-constraints.html#DDL-CONSTRAINTS-FK"));
  k!("ON DELETE SET NULL", "FK ON DELETE SET NULL -- null the child's FK column.", "REFERENCES users(id) ON DELETE SET NULL", pg("ddl-constraints.html#DDL-CONSTRAINTS-FK"));
  k!("ON DELETE SET DEFAULT", "FK ON DELETE SET DEFAULT -- set the FK column to its DEFAULT.", "REFERENCES users(id) ON DELETE SET DEFAULT", pg("ddl-constraints.html"));
  k!("ON DELETE RESTRICT", "FK ON DELETE RESTRICT -- refuse the parent delete (immediate).", "REFERENCES users(id) ON DELETE RESTRICT", pg("ddl-constraints.html"));
  k!("ON DELETE NO ACTION", "FK ON DELETE NO ACTION -- refuse at constraint check time (default).", "REFERENCES users(id) ON DELETE NO ACTION", pg("ddl-constraints.html"));
  k!("ON UPDATE CASCADE", "FK ON UPDATE CASCADE -- propagate PK updates to child rows.", "REFERENCES users(id) ON UPDATE CASCADE", pg("ddl-constraints.html"));
  k!("ON UPDATE SET NULL", "FK ON UPDATE SET NULL -- null the child column when parent PK changes.", "REFERENCES users(id) ON UPDATE SET NULL", pg("ddl-constraints.html"));
  k!("ON UPDATE RESTRICT", "FK ON UPDATE RESTRICT -- refuse the parent PK update.", "REFERENCES users(id) ON UPDATE RESTRICT", pg("ddl-constraints.html"));
  k!("ON UPDATE NO ACTION", "FK ON UPDATE NO ACTION -- refuse at check time (default).", "REFERENCES users(id) ON UPDATE NO ACTION", pg("ddl-constraints.html"));
  k!("RAISE EXCEPTION", "PL/pgSQL RAISE EXCEPTION 'msg' -- abort the transaction.", "RAISE EXCEPTION 'invalid: %', val;", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE NOTICE", "PL/pgSQL RAISE NOTICE 'msg' -- log to client at NOTICE level.", "RAISE NOTICE 'rows: %', cnt;", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE WARNING", "PL/pgSQL RAISE WARNING 'msg' -- log to client at WARNING level.", "RAISE WARNING 'deprecated path';", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE INFO", "PL/pgSQL RAISE INFO 'msg' -- log to client at INFO level.", "RAISE INFO 'reached step 2';", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE LOG", "PL/pgSQL RAISE LOG 'msg' -- write to server log only.", "RAISE LOG 'background task complete';", pg("plpgsql-errors-and-messages.html"));
  k!("RAISE DEBUG", "PL/pgSQL RAISE DEBUG 'msg' -- developer-visible diagnostic.", "RAISE DEBUG 'cache hit % / %', hit, total;", pg("plpgsql-errors-and-messages.html"));
  k!("MATCH FULL", "FK MATCH FULL -- all FK columns must be NULL or all not NULL.", "FOREIGN KEY (a, b) REFERENCES t(a, b) MATCH FULL", pg("sql-createtable.html"));
  k!("MATCH PARTIAL", "FK MATCH PARTIAL -- partial NULL allowed but other cols must match.", "MATCH PARTIAL", pg("sql-createtable.html"));
  k!("MATCH SIMPLE", "FK MATCH SIMPLE -- any FK column may be NULL (default).", "MATCH SIMPLE", pg("sql-createtable.html"));
  k!("REPLICA IDENTITY", "ALTER TABLE ... REPLICA IDENTITY { DEFAULT | FULL | NOTHING | USING INDEX <ix> }.", "ALTER TABLE t REPLICA IDENTITY FULL;", pg("sql-altertable.html"));
  k!("REPLICA IDENTITY FULL", "REPLICA IDENTITY FULL -- log every column on UPDATE/DELETE (heavy).", "ALTER TABLE t REPLICA IDENTITY FULL;", pg("sql-altertable.html"));
  k!("REPLICA IDENTITY NOTHING", "REPLICA IDENTITY NOTHING -- emit no old-tuple info (UPDATE/DELETE then carry only the new tuple).", "ALTER TABLE t REPLICA IDENTITY NOTHING;", pg("sql-altertable.html"));
  k!("REPLICA IDENTITY USING INDEX", "REPLICA IDENTITY USING INDEX <ix> -- use a NON-PK unique non-partial index.", "ALTER TABLE t REPLICA IDENTITY USING INDEX ux_email;", pg("sql-altertable.html"));
  k!("REPLICA IDENTITY DEFAULT", "REPLICA IDENTITY DEFAULT -- PK-based identity (default).", "ALTER TABLE t REPLICA IDENTITY DEFAULT;", pg("sql-altertable.html"));


  // ---- round 158 window-frame + trigger event multi-word kws ----
  k!("CURRENT ROW", "WINDOW frame bound: CURRENT ROW -- include the current row only.", "RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("UNBOUNDED PRECEDING", "WINDOW frame bound: UNBOUNDED PRECEDING -- start of the partition.", "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("UNBOUNDED FOLLOWING", "WINDOW frame bound: UNBOUNDED FOLLOWING -- end of the partition.", "ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("EXCLUDE CURRENT ROW", "WINDOW frame exclusion: omit the current row from the frame.", "EXCLUDE CURRENT ROW", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("EXCLUDE GROUP", "WINDOW frame exclusion: omit the current row + peers.", "EXCLUDE GROUP", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("EXCLUDE TIES", "WINDOW frame exclusion: omit the peers (keep current row).", "EXCLUDE TIES", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("EXCLUDE NO OTHERS", "WINDOW frame exclusion: keep everything (default).", "EXCLUDE NO OTHERS", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("RANGE BETWEEN", "RANGE BETWEEN <a> AND <b> -- value-based frame.", "RANGE BETWEEN 1 PRECEDING AND CURRENT ROW", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("ROWS BETWEEN", "ROWS BETWEEN <a> AND <b> -- physical-offset frame.", "ROWS BETWEEN 3 PRECEDING AND CURRENT ROW", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("GROUPS BETWEEN", "GROUPS BETWEEN <a> AND <b> -- peer-group frame.", "GROUPS BETWEEN 1 PRECEDING AND CURRENT ROW", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("SELECT INTO", "PL/pgSQL `SELECT <cols> INTO <vars> FROM ...`.", "SELECT id INTO v_id FROM t WHERE ...;", pg("plpgsql-statements.html"));
  k!("INSTEAD OF", "CREATE TRIGGER ... INSTEAD OF <event> ON <view> -- DML on a view.", "CREATE TRIGGER trg INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE FUNCTION ins();", pg("sql-createtrigger.html"));
  k!("BEFORE UPDATE", "CREATE TRIGGER ... BEFORE UPDATE ON <table>.", "CREATE TRIGGER trg BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION upd();", pg("sql-createtrigger.html"));
  k!("AFTER INSERT", "CREATE TRIGGER ... AFTER INSERT ON <table>.", "CREATE TRIGGER trg AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION post();", pg("sql-createtrigger.html"));
  k!("AFTER UPDATE", "CREATE TRIGGER ... AFTER UPDATE ON <table>.", "CREATE TRIGGER trg AFTER UPDATE ON users FOR EACH ROW EXECUTE FUNCTION post();", pg("sql-createtrigger.html"));
  k!("AFTER DELETE", "CREATE TRIGGER ... AFTER DELETE ON <table>.", "CREATE TRIGGER trg AFTER DELETE ON users FOR EACH ROW EXECUTE FUNCTION post();", pg("sql-createtrigger.html"));
  k!("BEFORE INSERT", "CREATE TRIGGER ... BEFORE INSERT ON <table>.", "CREATE TRIGGER trg BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION ins();", pg("sql-createtrigger.html"));
  k!("BEFORE DELETE", "CREATE TRIGGER ... BEFORE DELETE ON <table>.", "CREATE TRIGGER trg BEFORE DELETE ON users FOR EACH ROW EXECUTE FUNCTION del();", pg("sql-createtrigger.html"));
  k!("BEFORE TRUNCATE", "CREATE TRIGGER ... BEFORE TRUNCATE ON <table> (statement-level only).", "CREATE TRIGGER trg BEFORE TRUNCATE ON users FOR EACH STATEMENT EXECUTE FUNCTION before_trunc();", pg("sql-createtrigger.html"));
  k!("AFTER TRUNCATE", "CREATE TRIGGER ... AFTER TRUNCATE ON <table> (statement-level only).", "CREATE TRIGGER trg AFTER TRUNCATE ON users FOR EACH STATEMENT EXECUTE FUNCTION after_trunc();", pg("sql-createtrigger.html"));


  // ---- round 159 transaction + privilege multi-word kws ----
  k!("DEFAULT PRIVILEGES", "ALTER DEFAULT PRIVILEGES [FOR ROLE <r>] [IN SCHEMA <s>] {GRANT|REVOKE} ... -- post-creation default ACLs.", "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO readonly;", pg("sql-alterdefaultprivileges.html"));
  k!("SET CONSTRAINTS", "SET CONSTRAINTS { ALL | <name>[, ...] } { DEFERRED | IMMEDIATE } -- transaction-local constraint timing.", "SET CONSTRAINTS ALL DEFERRED;", pg("sql-set-constraints.html"));
  k!("SET ROLE", "SET [LOCAL | SESSION] ROLE <role> -- switch to another role's privileges.", "SET LOCAL ROLE readonly;", pg("sql-set-role.html"));
  k!("SET SESSION AUTHORIZATION", "SET SESSION AUTHORIZATION <role> -- masquerade as another role for the session (superuser only).", "SET SESSION AUTHORIZATION alice;", pg("sql-set-session-authorization.html"));
  k!("SET LOCAL", "SET LOCAL <param> = <value> -- transaction-scoped GUC change.", "SET LOCAL statement_timeout = '5s';", pg("sql-set.html"));
  k!("SET SESSION", "SET SESSION <param> = <value> -- session-scoped GUC change (default).", "SET SESSION work_mem = '64MB';", pg("sql-set.html"));
  k!("COMMIT AND CHAIN", "COMMIT AND CHAIN -- commit and immediately start a new transaction.", "COMMIT AND CHAIN;", pg("sql-commit.html"));
  k!("ROLLBACK AND CHAIN", "ROLLBACK AND CHAIN -- rollback and immediately start a new transaction.", "ROLLBACK AND CHAIN;", pg("sql-rollback.html"));
  k!("COMMIT AND NO CHAIN", "COMMIT AND NO CHAIN -- default; commit and stay outside a transaction.", "COMMIT AND NO CHAIN;", pg("sql-commit.html"));
  k!("ROLLBACK AND NO CHAIN", "ROLLBACK AND NO CHAIN -- default; rollback and stay outside a transaction.", "ROLLBACK AND NO CHAIN;", pg("sql-rollback.html"));
  k!("ROLLBACK TO SAVEPOINT", "ROLLBACK TO [SAVEPOINT] <name> -- undo back to a savepoint, keep the rest of the txn.", "ROLLBACK TO SAVEPOINT sp1;", pg("sql-rollback-to.html"));
  k!("RELEASE SAVEPOINT", "RELEASE [SAVEPOINT] <name> -- destroy a savepoint without rollback.", "RELEASE SAVEPOINT sp1;", pg("sql-release-savepoint.html"));
  k!("BEGIN TRANSACTION", "BEGIN TRANSACTION -- explicit SQL standard form of BEGIN.", "BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ;", pg("sql-begin.html"));
  k!("START TRANSACTION", "START TRANSACTION -- SQL standard form of BEGIN.", "START TRANSACTION READ ONLY;", pg("sql-start-transaction.html"));
  k!("ISOLATION LEVEL", "ISOLATION LEVEL { READ UNCOMMITTED | READ COMMITTED | REPEATABLE READ | SERIALIZABLE }.", "BEGIN ISOLATION LEVEL SERIALIZABLE;", pg("transaction-iso.html"));
  k!("READ ONLY", "Transaction mode: READ ONLY -- forbid writes.", "BEGIN READ ONLY;", pg("sql-set-transaction.html"));
  k!("READ WRITE", "Transaction mode: READ WRITE (default).", "BEGIN READ WRITE;", pg("sql-set-transaction.html"));
  k!("ALL TABLES", "ALTER DEFAULT PRIVILEGES ... ON ALL TABLES / GRANT ... ON ALL TABLES IN SCHEMA -- bulk privilege target.", "GRANT SELECT ON ALL TABLES IN SCHEMA public TO readonly;", pg("sql-grant.html"));
  k!("ALL FUNCTIONS", "GRANT ... ON ALL FUNCTIONS IN SCHEMA <s> -- bulk privilege target.", "GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO app;", pg("sql-grant.html"));
  k!("ALL PROCEDURES", "GRANT ... ON ALL PROCEDURES IN SCHEMA <s>.", "GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA public TO app;", pg("sql-grant.html"));
  k!("ALL SEQUENCES", "GRANT ... ON ALL SEQUENCES IN SCHEMA <s>.", "GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO app;", pg("sql-grant.html"));
  k!("ALL ROUTINES", "GRANT ... ON ALL ROUTINES IN SCHEMA <s> -- both functions and procedures (PG11+).", "GRANT EXECUTE ON ALL ROUTINES IN SCHEMA public TO app;", pg("sql-grant.html"));
  k!("ALL TYPES", "GRANT ... ON ALL TYPES IN SCHEMA <s>.", "GRANT USAGE ON ALL TYPES IN SCHEMA public TO app;", pg("sql-grant.html"));
  k!("ALL TABLESPACES", "GRANT ... ON ALL TABLESPACES (only valid for CREATE).", "GRANT CREATE ON TABLESPACE archive TO writer;", pg("sql-grant.html"));
  k!("ALL SCHEMAS", "ALTER DEFAULT PRIVILEGES ... ON SCHEMAS -- privilege over future schemas.", "ALTER DEFAULT PRIVILEGES GRANT USAGE ON SCHEMAS TO readonly;", pg("sql-alterdefaultprivileges.html"));


  // ---- round 169 ROLE attribute kws ----
  k!("LOGIN", "Role attribute: can authenticate.", "CREATE ROLE alice LOGIN PASSWORD '...';", pg("sql-createrole.html"));
  k!("NOLOGIN", "Role attribute: cannot authenticate (group role).", "CREATE ROLE admins NOLOGIN;", pg("sql-createrole.html"));
  k!("SUPERUSER", "Role attribute: bypass every permission check.", "CREATE ROLE root SUPERUSER;", pg("sql-createrole.html"));
  k!("NOSUPERUSER", "Role attribute: not a superuser (default).", "ALTER ROLE alice NOSUPERUSER;", pg("sql-createrole.html"));
  k!("CREATEDB", "Role attribute: may CREATE DATABASE.", "ALTER ROLE alice CREATEDB;", pg("sql-createrole.html"));
  k!("NOCREATEDB", "Role attribute: cannot CREATE DATABASE (default).", "ALTER ROLE alice NOCREATEDB;", pg("sql-createrole.html"));
  k!("CREATEROLE", "Role attribute: can create / manage other roles.", "ALTER ROLE alice CREATEROLE;", pg("sql-createrole.html"));
  k!("NOCREATEROLE", "Role attribute: cannot create other roles (default).", "ALTER ROLE alice NOCREATEROLE;", pg("sql-createrole.html"));
  k!("REPLICATION", "Role attribute: may initiate streaming replication.", "ALTER ROLE repl REPLICATION;", pg("sql-createrole.html"));
  k!("NOREPLICATION", "Role attribute: cannot initiate replication (default).", "ALTER ROLE repl NOREPLICATION;", pg("sql-createrole.html"));
  k!("BYPASSRLS", "Role attribute: bypass row-level security policies.", "ALTER ROLE alice BYPASSRLS;", pg("sql-createrole.html"));
  k!("NOBYPASSRLS", "Role attribute: subject to row-level security (default).", "ALTER ROLE alice NOBYPASSRLS;", pg("sql-createrole.html"));


  // ---- round 170 session control multi-word kws ----
  k!("DISCARD ALL", "DISCARD ALL -- session reset (drops temp tables, prepared stmts, plans, cursors, etc.).", "DISCARD ALL;", pg("sql-discard.html"));
  k!("DISCARD PLANS", "DISCARD PLANS -- forget cached query plans.", "DISCARD PLANS;", pg("sql-discard.html"));
  k!("DISCARD SEQUENCES", "DISCARD SEQUENCES -- forget session sequence state.", "DISCARD SEQUENCES;", pg("sql-discard.html"));
  k!("DISCARD TEMP", "DISCARD TEMP -- drop session-local temporary tables.", "DISCARD TEMP;", pg("sql-discard.html"));
  k!("DISCARD TEMPORARY", "DISCARD TEMPORARY -- same as DISCARD TEMP.", "DISCARD TEMPORARY;", pg("sql-discard.html"));
  k!("RESET ALL", "RESET ALL -- restore every session GUC to its default.", "RESET ALL;", pg("sql-reset.html"));
  k!("RESET ROLE", "RESET ROLE -- undo a SET ROLE.", "RESET ROLE;", pg("sql-reset.html"));

  k!("FILLFACTOR", "Storage parameter: leaf-page fill percentage (10-100). Lower leaves room for HOT updates.", "CREATE INDEX ... WITH (fillfactor = 80);", pg("sql-createindex.html#SQL-CREATEINDEX-STORAGE-PARAMETERS"));


  // ---- round 174 multi-word DDL clarifiers ----
  k!("FOREIGN DATA WRAPPER", "CREATE/ALTER/DROP FOREIGN DATA WRAPPER <name> -- FDW plugin.", "CREATE SERVER s FOREIGN DATA WRAPPER postgres_fdw OPTIONS (...);", pg("sql-createforeigndatawrapper.html"));
  k!("EVENT TRIGGER", "CREATE EVENT TRIGGER -- DDL-level trigger.", "CREATE EVENT TRIGGER trg ON ddl_command_end EXECUTE FUNCTION fn();", pg("sql-createeventtrigger.html"));
  k!("ACCESS METHOD", "CREATE/DROP ACCESS METHOD <name> TYPE INDEX HANDLER <fn>.", "CREATE ACCESS METHOD heap_v2 TYPE TABLE HANDLER heap_v2_handler;", pg("sql-create-access-method.html"));
  k!("USER MAPPING", "CREATE/DROP USER MAPPING FOR <role> SERVER <srv> OPTIONS (...).", "CREATE USER MAPPING FOR alice SERVER s OPTIONS (user 'a');", pg("sql-createusermapping.html"));
  k!("FOREIGN TABLE", "CREATE/DROP FOREIGN TABLE <name> SERVER <srv> -- FDW-backed relation.", "CREATE FOREIGN TABLE ext (...) SERVER s OPTIONS (table_name 'remote');", pg("sql-createforeigntable.html"));
  k!("TEXT SEARCH", "CREATE/DROP TEXT SEARCH { CONFIGURATION | DICTIONARY | PARSER | TEMPLATE }.", "CREATE TEXT SEARCH CONFIGURATION my (PARSER = default);", pg("sql-createtextsearchconfiguration.html"));
  k!("OWNED BY", "CREATE/ALTER SEQUENCE ... OWNED BY <tbl>.<col> | NONE -- tie sequence to a column.", "ALTER SEQUENCE s OWNED BY users.id;", pg("sql-altersequence.html"));
  k!("ROW LEVEL SECURITY", "ENABLE / DISABLE / FORCE / NO FORCE ROW LEVEL SECURITY.", "ALTER TABLE t ENABLE ROW LEVEL SECURITY;", pg("ddl-rowsecurity.html"));
  k!("SET STORAGE", "ALTER COLUMN ... SET STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN }.", "ALTER TABLE t ALTER COLUMN c SET STORAGE EXTERNAL;", pg("sql-altertable.html"));
  k!("SET STATISTICS", "ALTER COLUMN ... SET STATISTICS <n> -- per-column ANALYZE sample size.", "ALTER TABLE t ALTER COLUMN c SET STATISTICS 1000;", pg("sql-altertable.html"));
  k!("ADD GENERATED", "ALTER COLUMN ... ADD GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY -- promote column to identity.", "ALTER TABLE t ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY;", pg("sql-altertable.html"));
  k!("DROP IDENTITY", "ALTER COLUMN ... DROP IDENTITY [IF EXISTS] -- demote identity to plain column.", "ALTER TABLE t ALTER COLUMN id DROP IDENTITY;", pg("sql-altertable.html"));
  k!("RESTART IDENTITY", "TRUNCATE ... RESTART IDENTITY -- reset owned sequences.", "TRUNCATE t RESTART IDENTITY;", pg("sql-truncate.html"));
  k!("CONTINUE IDENTITY", "TRUNCATE ... CONTINUE IDENTITY -- keep current sequence values (default).", "TRUNCATE t CONTINUE IDENTITY;", pg("sql-truncate.html"));
  k!("AS IDENTITY", "<col> <type> GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY.", "id int GENERATED ALWAYS AS IDENTITY PRIMARY KEY", pg("sql-createtable.html"));
  k!("BY IDENTITY", "ALTER COLUMN ... SET GENERATED { ALWAYS | BY DEFAULT } -- the BY token used in this clause.", "ALTER TABLE t ALTER COLUMN id SET GENERATED BY DEFAULT;", pg("sql-altertable.html"));
  k!("FROM CURRENT", "ALTER ROLE / DATABASE ... SET <param> FROM CURRENT -- snapshot current GUC value.", "ALTER ROLE alice SET work_mem FROM CURRENT;", pg("sql-alterrole.html"));
  k!("USING METHOD", "CREATE INDEX ... USING <method> -- pick the index access method.", "CREATE INDEX ix ON t USING gin (data);", pg("sql-createindex.html"));
  k!("RETURNS TABLE", "CREATE FUNCTION ... RETURNS TABLE(<col> <type>, ...) -- set-returning function with named columns.", "CREATE FUNCTION top_users() RETURNS TABLE(id int, score int) AS $$ ... $$ LANGUAGE sql;", pg("sql-createfunction.html"));
  k!("RETURNS TRIGGER", "CREATE FUNCTION ... RETURNS TRIGGER -- trigger function shape.", "CREATE FUNCTION upd() RETURNS TRIGGER AS $$ BEGIN RETURN NEW; END; $$ LANGUAGE plpgsql;", pg("sql-createfunction.html"));
  k!("RETURNS SETOF", "CREATE FUNCTION ... RETURNS SETOF <type> -- multi-row return.", "CREATE FUNCTION fives() RETURNS SETOF int AS $$ SELECT generate_series(1,5) $$ LANGUAGE sql;", pg("sql-createfunction.html"));


  // ---- round 175 multi-word function attribute kws ----
  k!("LANGUAGE SQL", "CREATE FUNCTION ... LANGUAGE SQL -- pure SQL function body.", "CREATE FUNCTION add(int, int) RETURNS int AS $$ SELECT $1 + $2 $$ LANGUAGE SQL;", pg("sql-createfunction.html"));
  k!("LANGUAGE PLPGSQL", "CREATE FUNCTION ... LANGUAGE plpgsql -- procedural Postgres body.", "CREATE FUNCTION up() RETURNS void AS $$ BEGIN ... END; $$ LANGUAGE plpgsql;", pg("sql-createfunction.html"));
  k!("RESTRICT VERSION", "CREATE EXTENSION ... RESTRICT VERSION '<v>' -- pin to a specific version (rare).", "CREATE EXTENSION my_ext WITH VERSION '1.2.3';", pg("sql-createextension.html"));
  k!("GLOBAL TEMPORARY", "CREATE GLOBAL TEMPORARY TABLE ... -- SQL standard alias for TEMPORARY.", "CREATE GLOBAL TEMPORARY TABLE staging (...);", pg("sql-createtable.html"));
  k!("LOCAL TEMPORARY", "CREATE LOCAL TEMPORARY TABLE ... -- SQL standard alias for TEMPORARY.", "CREATE LOCAL TEMPORARY TABLE staging (...);", pg("sql-createtable.html"));
  k!("GLOBAL TEMP", "Same as GLOBAL TEMPORARY.", "CREATE GLOBAL TEMP TABLE staging (...);", pg("sql-createtable.html"));
  k!("LOCAL TEMP", "Same as LOCAL TEMPORARY.", "CREATE LOCAL TEMP TABLE staging (...);", pg("sql-createtable.html"));
  k!("WITH RECURSIVE", "WITH RECURSIVE <cte_name> (cols) AS (anchor UNION ALL recursive) ... -- recursive CTE.", "WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n<10) SELECT * FROM t;", pg("queries-with.html"));
  k!("OR REPLACE", "CREATE OR REPLACE FUNCTION / VIEW / TRIGGER / etc -- replace existing object.", "CREATE OR REPLACE VIEW v AS SELECT * FROM t;", pg("sql-createfunction.html"));
  k!("NOT VALID", "ALTER TABLE ... ADD CONSTRAINT ... NOT VALID -- skip the up-front scan; validate later.", "ALTER TABLE t ADD CONSTRAINT fk_x FOREIGN KEY (a) REFERENCES p(a) NOT VALID;", pg("sql-altertable.html"));
  k!("CONNECT BY", "Oracle hierarchical query syntax (NOT supported by PG).", "-- Oracle CONNECT BY ...", pg("appendix-keywords.html"));
  k!("START WITH", "Oracle hierarchical-query START WITH (NOT supported by PG).", "-- Oracle START WITH ...", pg("appendix-keywords.html"));
  k!("ON CONSTRAINT", "ON CONFLICT ON CONSTRAINT <name> -- target a specific named constraint.", "INSERT INTO t VALUES (...) ON CONFLICT ON CONSTRAINT uq_email DO UPDATE SET ...;", pg("sql-insert.html"));
  k!("SECURITY DEFINER", "CREATE FUNCTION ... SECURITY DEFINER -- run with the privileges of the function owner.", "CREATE FUNCTION priv_op() RETURNS void ... SECURITY DEFINER;", pg("sql-createfunction.html"));
  k!("SECURITY INVOKER", "CREATE FUNCTION ... SECURITY INVOKER (default) -- run with caller privileges.", "CREATE FUNCTION pub_op() RETURNS void ... SECURITY INVOKER;", pg("sql-createfunction.html"));
  k!("PARALLEL SAFE", "CREATE FUNCTION ... PARALLEL SAFE -- safe to run in parallel workers.", "CREATE FUNCTION add(int, int) RETURNS int ... PARALLEL SAFE;", pg("sql-createfunction.html"));
  k!("PARALLEL RESTRICTED", "CREATE FUNCTION ... PARALLEL RESTRICTED -- safe to run in leader but not workers.", "CREATE FUNCTION up() RETURNS int ... PARALLEL RESTRICTED;", pg("sql-createfunction.html"));
  k!("PARALLEL UNSAFE", "CREATE FUNCTION ... PARALLEL UNSAFE (default) -- forbid parallel execution.", "CREATE FUNCTION up() RETURNS int ... PARALLEL UNSAFE;", pg("sql-createfunction.html"));
  k!("RETURNS NULL ON NULL INPUT", "Synonym of STRICT in CREATE FUNCTION.", "CREATE FUNCTION ... RETURNS NULL ON NULL INPUT;", pg("sql-createfunction.html"));
  k!("CALLED ON NULL INPUT", "Default for CREATE FUNCTION -- run the body even when an argument is NULL.", "CREATE FUNCTION ... CALLED ON NULL INPUT;", pg("sql-createfunction.html"));


  // ---- round 176 multi-word clarifiers ----
  k!("AS RESTRICT", "CREATE CAST ... AS [ASSIGNMENT | IMPLICIT] -- omit for explicit-only (default).", "CREATE CAST (text AS my_t) WITH FUNCTION my_in;", pg("sql-createcast.html"));
  k!("AS ASSIGNMENT", "CREATE CAST ... AS ASSIGNMENT -- automatic in assignments.", "CREATE CAST (text AS my_t) WITH FUNCTION my_in AS ASSIGNMENT;", pg("sql-createcast.html"));
  k!("AS IMPLICIT", "CREATE CAST ... AS IMPLICIT -- automatic everywhere.", "CREATE CAST (text AS my_t) WITH FUNCTION my_in AS IMPLICIT;", pg("sql-createcast.html"));
  k!("AS ENUM", "CREATE TYPE <name> AS ENUM ('a', 'b', ...).", "CREATE TYPE status AS ENUM ('open','closed');", pg("sql-createtype.html"));
  k!("AS RANGE", "CREATE TYPE <name> AS RANGE (SUBTYPE = ..., ...).", "CREATE TYPE int_with_step AS RANGE (SUBTYPE = int4, SUBTYPE_OPCLASS = int4_ops);", pg("sql-createtype.html"));
  k!("WITH FUNCTION", "CREATE CAST ... WITH FUNCTION <fn>(args) -- use this function.", "CREATE CAST (text AS my_t) WITH FUNCTION my_in;", pg("sql-createcast.html"));
  k!("WITHOUT FUNCTION", "CREATE CAST ... WITHOUT FUNCTION -- binary-compatible cast.", "CREATE CAST (a AS b) WITHOUT FUNCTION;", pg("sql-createcast.html"));
  k!("WITH INOUT", "CREATE CAST ... WITH INOUT -- use type IO conversion.", "CREATE CAST (text AS my_t) WITH INOUT;", pg("sql-createcast.html"));
  k!("WITH OPTIONS", "ALTER FOREIGN TABLE ... ALTER COLUMN <c> OPTIONS (...) -- per-column FDW options.", "ALTER FOREIGN TABLE ext ALTER COLUMN c OPTIONS (column_name 'src');", pg("sql-alterforeigntable.html"));
  k!("WITH SCHEMA", "CREATE EXTENSION ... WITH SCHEMA <s> -- install objects into a specific schema.", "CREATE EXTENSION pgcrypto WITH SCHEMA crypto;", pg("sql-createextension.html"));
  k!("WITH VERSION", "CREATE EXTENSION ... WITH VERSION '<v>' -- pick a specific version.", "CREATE EXTENSION pgcrypto WITH VERSION '1.3';", pg("sql-createextension.html"));
  k!("WITH CASCADE", "CREATE EXTENSION ... WITH CASCADE -- auto-install required extensions.", "CREATE EXTENSION postgis CASCADE;", pg("sql-createextension.html"));
  k!("TO PUBLIC", "GRANT ... TO PUBLIC -- every role.", "GRANT SELECT ON t TO PUBLIC;", pg("sql-grant.html"));
  k!("ON ALL TABLES IN SCHEMA", "GRANT/REVOKE ... ON ALL TABLES IN SCHEMA <s> -- bulk-affect every existing table in the schema (does NOT apply to future tables; use ALTER DEFAULT PRIVILEGES for that).", "GRANT SELECT ON ALL TABLES IN SCHEMA app TO readonly;", pg("sql-grant.html"));
  k!("ON ALL FUNCTIONS IN SCHEMA", "GRANT/REVOKE ... ON ALL FUNCTIONS IN SCHEMA <s> -- bulk-affect every existing fn in the schema.", "GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("ON ALL SEQUENCES IN SCHEMA", "GRANT/REVOKE ... ON ALL SEQUENCES IN SCHEMA <s> -- bulk-affect every existing sequence in the schema.", "GRANT USAGE ON ALL SEQUENCES IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("ON ALL ROUTINES IN SCHEMA", "GRANT/REVOKE ... ON ALL ROUTINES IN SCHEMA <s> -- bulk-affect every fn AND procedure (PG11+).", "GRANT EXECUTE ON ALL ROUTINES IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("ON ALL PROCEDURES IN SCHEMA", "GRANT/REVOKE ... ON ALL PROCEDURES IN SCHEMA <s> -- bulk-affect every existing procedure in the schema.", "GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA app TO svc;", pg("sql-grant.html"));
  k!("TO CURRENT_USER", "GRANT ... TO CURRENT_USER -- the role that issued the statement.", "GRANT EXECUTE ON FUNCTION f() TO CURRENT_USER;", pg("sql-grant.html"));
  k!("TO SESSION_USER", "GRANT ... TO SESSION_USER -- the role the session authenticated as.", "GRANT EXECUTE ON FUNCTION f() TO SESSION_USER;", pg("sql-grant.html"));
  k!("DEPENDS ON EXTENSION", "ALTER ... DEPENDS ON EXTENSION <ext> -- mark object as auto-dropped with the extension.", "ALTER FUNCTION fn() DEPENDS ON EXTENSION pgcrypto;", pg("sql-altertable.html"));
  k!("DROP EXPRESSION", "ALTER TABLE ... ALTER COLUMN <c> DROP EXPRESSION [IF EXISTS] -- demote generated column.", "ALTER TABLE t ALTER COLUMN total DROP EXPRESSION;", pg("sql-altertable.html"));
  k!("ADD EXPRESSION", "(reserved; emerged in some discussions) -- ALTER COLUMN ADD EXPRESSION not currently supported by PG.", "-- not supported", pg("sql-altertable.html"));
  k!("VALIDATE CONSTRAINT", "ALTER TABLE ... VALIDATE CONSTRAINT <name> -- validate a previously NOT VALID constraint.", "ALTER TABLE t VALIDATE CONSTRAINT fk_x;", pg("sql-altertable.html"));
  k!("DEFERRABLE INITIALLY DEFERRED", "Constraint timing: DEFERRABLE INITIALLY DEFERRED.", "ADD CONSTRAINT fk_x ... DEFERRABLE INITIALLY DEFERRED", pg("sql-createtable.html"));
  k!("DEFERRABLE INITIALLY IMMEDIATE", "Constraint timing: DEFERRABLE INITIALLY IMMEDIATE.", "ADD CONSTRAINT fk_x ... DEFERRABLE INITIALLY IMMEDIATE", pg("sql-createtable.html"));
  k!("NOT DEFERRABLE", "Constraint timing: NOT DEFERRABLE (default).", "ADD CONSTRAINT fk_x ... NOT DEFERRABLE", pg("sql-createtable.html"));
  k!("PRESERVE ROWS", "ON COMMIT PRESERVE ROWS -- keep temporary-table rows past COMMIT (default).", "CREATE TEMP TABLE staging (...) ON COMMIT PRESERVE ROWS;", pg("sql-createtable.html"));
  k!("DELETE ROWS", "ON COMMIT DELETE ROWS -- truncate temporary-table rows at COMMIT.", "CREATE TEMP TABLE staging (...) ON COMMIT DELETE ROWS;", pg("sql-createtable.html"));


  // ---- round 177 trigger transition + view check multi-word kws ----
  k!("OLD TABLE", "REFERENCING OLD TABLE AS <alias> -- statement-level trigger transition relation.", "REFERENCING OLD TABLE AS old_rows", pg("sql-createtrigger.html"));
  k!("NEW TABLE", "REFERENCING NEW TABLE AS <alias> -- statement-level trigger transition relation.", "REFERENCING NEW TABLE AS new_rows", pg("sql-createtrigger.html"));
  k!("OLD AS", "REFERENCING OLD AS <alias> -- row-level trigger old row alias.", "REFERENCING OLD AS o NEW AS n", pg("sql-createtrigger.html"));
  k!("NEW AS", "REFERENCING NEW AS <alias> -- row-level trigger new row alias.", "REFERENCING OLD AS o NEW AS n", pg("sql-createtrigger.html"));
  k!("OLD ROW", "PL/pgSQL implicit row alias OLD -- represents the row BEFORE the event.", "IF OLD.status <> NEW.status THEN ...", pg("plpgsql-trigger.html"));
  k!("NEW ROW", "PL/pgSQL implicit row alias NEW -- represents the row AFTER the event.", "IF OLD.status <> NEW.status THEN ...", pg("plpgsql-trigger.html"));
  k!("TRANSITION", "CREATE TRIGGER ... REFERENCING { OLD | NEW } TABLE AS <alias> -- statement-level transition tables.", "REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows", pg("sql-createtrigger.html"));
  k!("UPDATE OF", "CREATE TRIGGER ... UPDATE OF <col>[, ...] -- column-list-scoped UPDATE event.", "CREATE TRIGGER trg AFTER UPDATE OF status ON orders ...", pg("sql-createtrigger.html"));
  k!("INSERT OR UPDATE", "CREATE TRIGGER ... INSERT OR UPDATE -- multi-event chain.", "CREATE TRIGGER trg AFTER INSERT OR UPDATE ON t ...", pg("sql-createtrigger.html"));
  k!("ALTER COLUMN TYPE", "ALTER TABLE ... ALTER COLUMN <c> TYPE <new_type> [USING <expr>] -- change column type.", "ALTER TABLE t ALTER COLUMN price TYPE numeric USING price::numeric;", pg("sql-altertable.html"));
  k!("DROP CONSTRAINT", "ALTER TABLE ... DROP CONSTRAINT [IF EXISTS] <name> [CASCADE|RESTRICT].", "ALTER TABLE t DROP CONSTRAINT fk_x;", pg("sql-altertable.html"));
  k!("RENAME CONSTRAINT", "ALTER TABLE ... RENAME CONSTRAINT <old> TO <new>.", "ALTER TABLE t RENAME CONSTRAINT fk_x TO fk_users_org;", pg("sql-altertable.html"));
  k!("WHERE CURRENT OF", "UPDATE/DELETE ... WHERE CURRENT OF <cursor> -- positioned update/delete (PG cursors).", "UPDATE t SET c = c + 1 WHERE CURRENT OF c1;", pg("sql-update.html"));
  k!("CURRENT OF", "Used in WHERE CURRENT OF <cursor>; identifies the row the cursor is positioned at.", "WHERE CURRENT OF c1", pg("sql-update.html"));
  k!("CHECK OPTION", "CREATE VIEW ... WITH CHECK OPTION -- forbid INSERT/UPDATE that wouldn't satisfy the view's WHERE.", "CREATE VIEW v AS SELECT * FROM t WHERE active WITH CHECK OPTION;", pg("sql-createview.html"));
  k!("WITH CHECK OPTION", "VIEW WITH CHECK OPTION -- same as CASCADED variant (default).", "WITH CHECK OPTION", pg("sql-createview.html"));
  k!("WITH LOCAL CHECK OPTION", "VIEW WITH LOCAL CHECK OPTION -- only check this view's predicate; ignore parent views.", "WITH LOCAL CHECK OPTION", pg("sql-createview.html"));
  k!("WITH CASCADED CHECK OPTION", "VIEW WITH CASCADED CHECK OPTION -- enforce predicates of this AND parent views (default).", "WITH CASCADED CHECK OPTION", pg("sql-createview.html"));


  // ---- round 178 multi-word DML / RLS / SET kws ----
  k!("INSTEAD OF INSERT", "CREATE TRIGGER ... INSTEAD OF INSERT ON <view> -- view writability.", "CREATE TRIGGER trg INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE FUNCTION ins();", pg("sql-createtrigger.html"));
  k!("INSTEAD OF UPDATE", "CREATE TRIGGER ... INSTEAD OF UPDATE ON <view>.", "CREATE TRIGGER trg INSTEAD OF UPDATE ON v FOR EACH ROW EXECUTE FUNCTION upd();", pg("sql-createtrigger.html"));
  k!("INSTEAD OF DELETE", "CREATE TRIGGER ... INSTEAD OF DELETE ON <view>.", "CREATE TRIGGER trg INSTEAD OF DELETE ON v FOR EACH ROW EXECUTE FUNCTION del();", pg("sql-createtrigger.html"));
  k!("FOR ALL", "CREATE POLICY ... FOR ALL -- policy applies to every DML.", "CREATE POLICY p ON t FOR ALL TO public USING (...);", pg("sql-createpolicy.html"));
  k!("FOR ROLE", "ALTER DEFAULT PRIVILEGES FOR ROLE <r> -- scope default ACL changes.", "ALTER DEFAULT PRIVILEGES FOR ROLE owner GRANT SELECT ON TABLES TO readonly;", pg("sql-alterdefaultprivileges.html"));
  k!("FOR USER", "ALTER DEFAULT PRIVILEGES FOR USER <r> -- same as FOR ROLE.", "ALTER DEFAULT PRIVILEGES FOR USER alice GRANT ...", pg("sql-alterdefaultprivileges.html"));
  k!("FOR PARTITION", "ALTER INDEX ... ATTACH PARTITION <partition_index> FOR PARTITION <child_index>. (Internal partitioning glue.)", "ALTER INDEX p_idx ATTACH PARTITION ch_idx;", pg("sql-alterindex.html"));
  k!("FOR EACH", "CREATE TRIGGER ... FOR EACH { ROW | STATEMENT }.", "FOR EACH ROW EXECUTE FUNCTION upd();", pg("sql-createtrigger.html"));
  k!("FORCE TYPE", "CREATE FUNCTION ... ARGTYPES + FORCE_TYPE -- not a standalone PG keyword (here for completeness).", "-- internal", pg("appendix-keywords.html"));
  k!("FORCE ROW LEVEL SECURITY", "ALTER TABLE ... FORCE ROW LEVEL SECURITY -- apply RLS even to the table owner.", "ALTER TABLE t FORCE ROW LEVEL SECURITY;", pg("sql-altertable.html"));
  k!("NO FORCE ROW LEVEL SECURITY", "ALTER TABLE ... NO FORCE ROW LEVEL SECURITY -- revert FORCE (default).", "ALTER TABLE t NO FORCE ROW LEVEL SECURITY;", pg("sql-altertable.html"));
  k!("ENABLE ROW LEVEL SECURITY", "ALTER TABLE ... ENABLE ROW LEVEL SECURITY -- enforce row-level security policies on the table.", "ALTER TABLE t ENABLE ROW LEVEL SECURITY;", pg("sql-altertable.html"));
  k!("DISABLE ROW LEVEL SECURITY", "ALTER TABLE ... DISABLE ROW LEVEL SECURITY -- stop enforcing RLS policies (default).", "ALTER TABLE t DISABLE ROW LEVEL SECURITY;", pg("sql-altertable.html"));
  k!("WITH CHECK", "CREATE POLICY ... WITH CHECK (<expr>) -- new/updated rows must satisfy <expr> for INSERT/UPDATE.", "CREATE POLICY p ON t FOR ALL USING (true) WITH CHECK (owner = current_user);", pg("sql-createpolicy.html"));
  k!("SET CONSTRAINTS DEFERRED", "SET CONSTRAINTS ALL DEFERRED -- defer checks until commit.", "SET CONSTRAINTS ALL DEFERRED;", pg("sql-set-constraints.html"));
  k!("SET CONSTRAINTS IMMEDIATE", "SET CONSTRAINTS ALL IMMEDIATE -- check at statement end.", "SET CONSTRAINTS ALL IMMEDIATE;", pg("sql-set-constraints.html"));
  k!("SET CONSTRAINTS ALL", "SET CONSTRAINTS ALL { DEFERRED | IMMEDIATE } -- affect every deferrable constraint.", "SET CONSTRAINTS ALL DEFERRED;", pg("sql-set-constraints.html"));
  k!("AT LOCAL", "<timestamptz> AT LOCAL -- convert to local time zone (PG17+).", "SELECT now() AT LOCAL;", pg("functions-datetime.html"));
  k!("INTERSECT ALL", "Set op: INTERSECT ALL -- keep duplicates.", "SELECT id FROM a INTERSECT ALL SELECT id FROM b;", pg("queries-union.html"));
  k!("EXCEPT ALL", "Set op: EXCEPT ALL -- keep duplicates.", "SELECT id FROM a EXCEPT ALL SELECT id FROM b;", pg("queries-union.html"));
  k!("UNION ALL", "Set op: UNION ALL -- keep duplicates (faster than UNION).", "SELECT id FROM a UNION ALL SELECT id FROM b;", pg("queries-union.html"));
  k!("INTERSECT DISTINCT", "Set op: INTERSECT [DISTINCT] -- explicit form of plain INTERSECT.", "a INTERSECT DISTINCT b", pg("queries-union.html"));
  k!("EXCEPT DISTINCT", "Set op: EXCEPT [DISTINCT] -- explicit form of plain EXCEPT.", "a EXCEPT DISTINCT b", pg("queries-union.html"));
  k!("UNION DISTINCT", "Set op: UNION [DISTINCT] -- explicit form of plain UNION.", "a UNION DISTINCT b", pg("queries-union.html"));
  k!("GROUP BY ALL", "GROUP BY ALL -- shorthand for grouping by every non-aggregate output column (NOT supported by PG; common in Snowflake/MySQL 8.0+).", "-- unsupported in PG", pg("appendix-keywords.html"));
  k!("GROUP BY DISTINCT", "GROUP BY DISTINCT (a, b, GROUPING SETS (...)) -- drop duplicate grouping sets (PG14+).", "GROUP BY DISTINCT GROUPING SETS ((a), (a,b))", pg("queries-table-expressions.html#QUERIES-GROUPING-SETS"));
  k!("SELECT DISTINCT", "SELECT DISTINCT <cols> ... -- dedupe entire rows.", "SELECT DISTINCT id FROM t;", pg("sql-select.html"));
  k!("SELECT DISTINCT ON", "SELECT DISTINCT ON (<expr>[, ...]) ... -- keep first row per distinct ON expression.", "SELECT DISTINCT ON (user_id) * FROM events ORDER BY user_id, created_at DESC;", pg("sql-select.html#SQL-DISTINCT"));


  // ---- round 179 multi-word starter kws ----
  k!("CREATE OR REPLACE FUNCTION", "CREATE OR REPLACE FUNCTION <name>(args) RETURNS ... -- replace existing fn.", "CREATE OR REPLACE FUNCTION up() RETURNS void ...;", pg("sql-createfunction.html"));
  k!("CREATE OR REPLACE PROCEDURE", "CREATE OR REPLACE PROCEDURE <name>(args) -- replace existing procedure.", "CREATE OR REPLACE PROCEDURE up() ...;", pg("sql-createprocedure.html"));
  k!("CREATE OR REPLACE VIEW", "CREATE OR REPLACE VIEW <name> AS SELECT ...; replace if columns/types match.", "CREATE OR REPLACE VIEW v AS SELECT * FROM t;", pg("sql-createview.html"));
  k!("CREATE OR REPLACE TRIGGER", "CREATE OR REPLACE TRIGGER <name> ... -- replace existing trigger (PG14+).", "CREATE OR REPLACE TRIGGER trg ...;", pg("sql-createtrigger.html"));
  k!("CREATE OR REPLACE RULE", "CREATE OR REPLACE RULE <name> AS ON <event> TO <table> DO ...;", "CREATE OR REPLACE RULE r AS ON SELECT TO v DO INSTEAD SELECT 1;", pg("sql-createrule.html"));
  k!("CREATE INDEX CONCURRENTLY", "CREATE INDEX CONCURRENTLY <name> ON <table>(...) -- non-blocking index build.", "CREATE INDEX CONCURRENTLY ix_email ON users(email);", pg("sql-createindex.html"));
  k!("CREATE UNIQUE INDEX", "CREATE UNIQUE INDEX <name> ON <table>(...) -- enforce uniqueness.", "CREATE UNIQUE INDEX ux_email ON users(email);", pg("sql-createindex.html"));
  k!("CREATE UNIQUE INDEX CONCURRENTLY", "CREATE UNIQUE INDEX CONCURRENTLY <name> ... -- non-blocking unique index build.", "CREATE UNIQUE INDEX CONCURRENTLY ux_email ON users(email);", pg("sql-createindex.html"));
  k!("CREATE TEMP TABLE", "CREATE TEMP TABLE <name> (...) -- session-local table.", "CREATE TEMP TABLE staging (id int);", pg("sql-createtable.html"));
  k!("CREATE TEMPORARY TABLE", "CREATE TEMPORARY TABLE <name> (...) -- same as TEMP.", "CREATE TEMPORARY TABLE staging (id int);", pg("sql-createtable.html"));
  k!("CREATE UNLOGGED TABLE", "CREATE UNLOGGED TABLE <name> (...) -- skipped from WAL; lost on crash.", "CREATE UNLOGGED TABLE cache (k text PRIMARY KEY, v text);", pg("sql-createtable.html"));
  k!("CREATE FOREIGN TABLE", "CREATE FOREIGN TABLE <name> (...) SERVER <srv> OPTIONS (...).", "CREATE FOREIGN TABLE remote (id int) SERVER s OPTIONS (table_name 'remote');", pg("sql-createforeigntable.html"));
  k!("CREATE EVENT TRIGGER", "CREATE EVENT TRIGGER <name> ON <event> EXECUTE FUNCTION <fn>().", "CREATE EVENT TRIGGER trg ON ddl_command_end EXECUTE FUNCTION dispatch();", pg("sql-createeventtrigger.html"));
  k!("DROP INDEX CONCURRENTLY", "DROP INDEX CONCURRENTLY [IF EXISTS] <name> -- non-blocking drop.", "DROP INDEX CONCURRENTLY ix_old;", pg("sql-dropindex.html"));
  k!("DROP CONSTRAINT IF EXISTS", "ALTER TABLE ... DROP CONSTRAINT IF EXISTS <name>.", "ALTER TABLE t DROP CONSTRAINT IF EXISTS fk_x;", pg("sql-altertable.html"));
  k!("DROP TABLE IF EXISTS", "DROP TABLE IF EXISTS <name>[, ...] [CASCADE|RESTRICT].", "DROP TABLE IF EXISTS staging;", pg("sql-droptable.html"));
  k!("DROP INDEX IF EXISTS", "DROP INDEX [CONCURRENTLY] IF EXISTS <name>.", "DROP INDEX IF EXISTS ix_old;", pg("sql-dropindex.html"));
  k!("DROP MATERIALIZED VIEW", "DROP MATERIALIZED VIEW [IF EXISTS] <name> [CASCADE].", "DROP MATERIALIZED VIEW IF EXISTS mv;", pg("sql-dropmaterializedview.html"));
  k!("DROP FOREIGN TABLE", "DROP FOREIGN TABLE [IF EXISTS] <name>.", "DROP FOREIGN TABLE IF EXISTS ext;", pg("sql-dropforeigntable.html"));
  k!("DROP EVENT TRIGGER", "DROP EVENT TRIGGER [IF EXISTS] <name>.", "DROP EVENT TRIGGER trg;", pg("sql-dropeventtrigger.html"));
  k!("DROP ACCESS METHOD", "DROP ACCESS METHOD [IF EXISTS] <name>.", "DROP ACCESS METHOD heap_v2;", pg("sql-drop-access-method.html"));


  // ---- round 180 multi-word ALTER starter kws ----
  k!("ALTER TABLE IF EXISTS", "ALTER TABLE IF EXISTS <name> ... -- skip silently if missing.", "ALTER TABLE IF EXISTS t ADD COLUMN x int;", pg("sql-altertable.html"));
  k!("ALTER TABLE ONLY", "ALTER TABLE ONLY <name> ... -- skip child tables in inheritance.", "ALTER TABLE ONLY parent ADD COLUMN x int;", pg("sql-altertable.html"));
  k!("ALTER MATERIALIZED VIEW", "ALTER MATERIALIZED VIEW <name> ...", "ALTER MATERIALIZED VIEW mv RENAME TO mv2;", pg("sql-altermaterializedview.html"));
  k!("ALTER EVENT TRIGGER", "ALTER EVENT TRIGGER <name> ...", "ALTER EVENT TRIGGER trg ENABLE REPLICA;", pg("sql-altereventtrigger.html"));
  k!("ALTER ACCESS METHOD", "ALTER ACCESS METHOD <name> RENAME TO <new>.", "ALTER ACCESS METHOD am RENAME TO am2;", pg("sql-alter-access-method.html"));
  k!("ALTER OPERATOR FAMILY", "ALTER OPERATOR FAMILY <name> USING <am> ADD/DROP ...", "ALTER OPERATOR FAMILY fam USING btree ADD OPERATOR 1 < (int, int);", pg("sql-alteroperatorfamily.html"));
  k!("ALTER OPERATOR CLASS", "ALTER OPERATOR CLASS <name> USING <am> RENAME TO <new>.", "ALTER OPERATOR CLASS oc USING btree RENAME TO oc2;", pg("sql-alteroperatorclass.html"));
  k!("ALTER FOREIGN TABLE", "ALTER FOREIGN TABLE <name> ...", "ALTER FOREIGN TABLE ext ALTER COLUMN c OPTIONS (column_name 'src');", pg("sql-alterforeigntable.html"));
  k!("ALTER FOREIGN DATA WRAPPER", "ALTER FOREIGN DATA WRAPPER <name> ...", "ALTER FOREIGN DATA WRAPPER pg_fdw HANDLER fn;", pg("sql-alterforeigndatawrapper.html"));
  k!("ALTER PUBLICATION", "ALTER PUBLICATION <name> ...", "ALTER PUBLICATION p ADD TABLE t;", pg("sql-alterpublication.html"));
  k!("ALTER SUBSCRIPTION", "ALTER SUBSCRIPTION <name> ...", "ALTER SUBSCRIPTION s REFRESH PUBLICATION;", pg("sql-altersubscription.html"));
  k!("ALTER POLICY", "ALTER POLICY <name> ON <table> ...", "ALTER POLICY p ON t USING (...) WITH CHECK (...);", pg("sql-alterpolicy.html"));
  k!("ALTER STATISTICS", "ALTER STATISTICS <name> ...", "ALTER STATISTICS s SET STATISTICS 1000;", pg("sql-alterstatistics.html"));
  k!("ALTER LANGUAGE", "ALTER LANGUAGE <name> RENAME TO <new>.", "ALTER LANGUAGE plpgsql RENAME TO ppl;", pg("sql-alterlanguage.html"));
  k!("ALTER CONVERSION", "ALTER CONVERSION <name> RENAME TO <new>.", "ALTER CONVERSION c RENAME TO c2;", pg("sql-alterconversion.html"));
  k!("ALTER COLLATION", "ALTER COLLATION <name> ...", "ALTER COLLATION fr_FR REFRESH VERSION;", pg("sql-altercollation.html"));
  k!("ALTER AGGREGATE", "ALTER AGGREGATE <name>(arg_types) ...", "ALTER AGGREGATE my_sum(bigint) OWNER TO admins;", pg("sql-alteraggregate.html"));
  k!("ALTER OPERATOR", "ALTER OPERATOR <op>(left_type, right_type) ...", "ALTER OPERATOR === (int, int) OWNER TO admins;", pg("sql-alteroperator.html"));
  k!("ALTER CAST", "ALTER CAST (src AS dst) ...", "ALTER CAST (text AS my_t) OWNER TO admins;", pg("sql-altercast.html"));
  k!("ALTER TYPE", "ALTER TYPE <name> ...", "ALTER TYPE status ADD VALUE 'archived';", pg("sql-altertype.html"));
  k!("ALTER DOMAIN", "ALTER DOMAIN <name> ...", "ALTER DOMAIN email_t SET DEFAULT NULL;", pg("sql-alterdomain.html"));
  k!("ALTER USER MAPPING", "ALTER USER MAPPING FOR <role> SERVER <srv> OPTIONS (...).", "ALTER USER MAPPING FOR alice SERVER s OPTIONS (SET user 'a');", pg("sql-alterusermapping.html"));
  k!("ALTER SEQUENCE IF EXISTS", "ALTER SEQUENCE IF EXISTS <name> ...", "ALTER SEQUENCE IF EXISTS s RESTART;", pg("sql-altersequence.html"));
  k!("ALTER VIEW IF EXISTS", "ALTER VIEW IF EXISTS <name> ...", "ALTER VIEW IF EXISTS v RENAME TO v2;", pg("sql-alterview.html"));


  // ---- round 181 COMMENT ON + DROP starter kws ----
  k!("COMMENT ON TABLE", "COMMENT ON TABLE <name> IS '...' -- attach a comment.", "COMMENT ON TABLE users IS 'authn users';", pg("sql-comment.html"));
  k!("COMMENT ON COLUMN", "COMMENT ON COLUMN <tbl>.<col> IS '...'.", "COMMENT ON COLUMN users.email IS 'primary email';", pg("sql-comment.html"));
  k!("COMMENT ON SCHEMA", "COMMENT ON SCHEMA <name> IS '...'.", "COMMENT ON SCHEMA app IS 'application objects';", pg("sql-comment.html"));
  k!("COMMENT ON DATABASE", "COMMENT ON DATABASE <name> IS '...'.", "COMMENT ON DATABASE mydb IS 'prod db';", pg("sql-comment.html"));
  k!("COMMENT ON FUNCTION", "COMMENT ON FUNCTION <name>(args) IS '...'.", "COMMENT ON FUNCTION add(int, int) IS 'sums args';", pg("sql-comment.html"));
  k!("COMMENT ON PROCEDURE", "COMMENT ON PROCEDURE <name>(args) IS '...'.", "COMMENT ON PROCEDURE up() IS 'startup';", pg("sql-comment.html"));
  k!("COMMENT ON INDEX", "COMMENT ON INDEX <name> IS '...'.", "COMMENT ON INDEX ix_email IS 'lookup index';", pg("sql-comment.html"));
  k!("COMMENT ON VIEW", "COMMENT ON VIEW <name> IS '...'.", "COMMENT ON VIEW recent_logs IS '24h subset';", pg("sql-comment.html"));
  k!("COMMENT ON MATERIALIZED VIEW", "COMMENT ON MATERIALIZED VIEW <name> IS '...'.", "COMMENT ON MATERIALIZED VIEW mv IS 'precomputed';", pg("sql-comment.html"));
  k!("COMMENT ON SEQUENCE", "COMMENT ON SEQUENCE <name> IS '...'.", "COMMENT ON SEQUENCE order_id_seq IS 'order PK';", pg("sql-comment.html"));
  k!("COMMENT ON TYPE", "COMMENT ON TYPE <name> IS '...'.", "COMMENT ON TYPE status IS 'order lifecycle';", pg("sql-comment.html"));
  k!("COMMENT ON DOMAIN", "COMMENT ON DOMAIN <name> IS '...'.", "COMMENT ON DOMAIN email_t IS 'RFC5322 email';", pg("sql-comment.html"));
  k!("COMMENT ON EXTENSION", "COMMENT ON EXTENSION <name> IS '...'.", "COMMENT ON EXTENSION pgcrypto IS 'hash + cipher';", pg("sql-comment.html"));
  k!("COMMENT ON ROLE", "COMMENT ON ROLE <name> IS '...'.", "COMMENT ON ROLE admins IS 'sysadmins';", pg("sql-comment.html"));
  k!("COMMENT ON TRIGGER", "COMMENT ON TRIGGER <name> ON <table> IS '...'.", "COMMENT ON TRIGGER trg ON users IS 'updates set';", pg("sql-comment.html"));
  k!("COMMENT ON CONSTRAINT", "COMMENT ON CONSTRAINT <name> ON <table> IS '...'.", "COMMENT ON CONSTRAINT fk_x ON t IS 'links to users';", pg("sql-comment.html"));
  k!("COMMENT ON POLICY", "COMMENT ON POLICY <name> ON <table> IS '...'.", "COMMENT ON POLICY p ON users IS 'tenant filter';", pg("sql-comment.html"));
  k!("DROP DATABASE", "DROP DATABASE [IF EXISTS] <name> -- destroy a database.", "DROP DATABASE staging;", pg("sql-dropdatabase.html"));
  k!("DROP SCHEMA", "DROP SCHEMA [IF EXISTS] <name>[, ...] [CASCADE|RESTRICT].", "DROP SCHEMA temp CASCADE;", pg("sql-dropschema.html"));
  k!("DROP ROLE", "DROP ROLE [IF EXISTS] <name>[, ...].", "DROP ROLE bob;", pg("sql-droprole.html"));
  k!("DROP USER", "DROP USER [IF EXISTS] <name>[, ...] -- alias for DROP ROLE.", "DROP USER bob;", pg("sql-dropuser.html"));
  k!("DROP GROUP", "DROP GROUP [IF EXISTS] <name>[, ...] -- alias for DROP ROLE.", "DROP GROUP admins;", pg("sql-dropgroup.html"));
  k!("DROP TABLESPACE", "DROP TABLESPACE [IF EXISTS] <name>.", "DROP TABLESPACE archive;", pg("sql-droptablespace.html"));
  k!("DROP EXTENSION", "DROP EXTENSION [IF EXISTS] <name>[, ...] [CASCADE].", "DROP EXTENSION pgcrypto;", pg("sql-dropextension.html"));
  k!("DROP PUBLICATION", "DROP PUBLICATION [IF EXISTS] <name>.", "DROP PUBLICATION pub1;", pg("sql-droppublication.html"));
  k!("DROP SUBSCRIPTION", "DROP SUBSCRIPTION [IF EXISTS] <name>.", "DROP SUBSCRIPTION sub1;", pg("sql-dropsubscription.html"));
  k!("DROP SERVER", "DROP SERVER [IF EXISTS] <name> [CASCADE].", "DROP SERVER myserv;", pg("sql-dropserver.html"));
  k!("DROP TRIGGER", "DROP TRIGGER [IF EXISTS] <name> ON <table> [CASCADE].", "DROP TRIGGER trg ON users;", pg("sql-droptrigger.html"));
  k!("DROP TYPE", "DROP TYPE [IF EXISTS] <name>[, ...] [CASCADE].", "DROP TYPE status;", pg("sql-droptype.html"));
  k!("DROP DOMAIN", "DROP DOMAIN [IF EXISTS] <name>[, ...] [CASCADE].", "DROP DOMAIN email_t;", pg("sql-dropdomain.html"));
  k!("DROP POLICY", "DROP POLICY [IF EXISTS] <name> ON <table>.", "DROP POLICY p ON users;", pg("sql-droppolicy.html"));


  // ---- round 182 CREATE multi-word starter kws ----
  k!("CREATE DATABASE", "CREATE DATABASE <name> [OWNER <r>] [TEMPLATE <t>] [ENCODING '<enc>'] [LOCALE '<loc>'] [TABLESPACE <ts>] [CONNECTION LIMIT <n>] [...].", "CREATE DATABASE mydb OWNER alice;", pg("sql-createdatabase.html"));
  k!("CREATE SCHEMA", "CREATE SCHEMA [IF NOT EXISTS] <name> [AUTHORIZATION <role>] [<schema_element> ...].", "CREATE SCHEMA app AUTHORIZATION admins;", pg("sql-createschema.html"));
  k!("CREATE ROLE", "CREATE ROLE <name> [WITH <attr> ...] -- new role.", "CREATE ROLE alice LOGIN PASSWORD '...';", pg("sql-createrole.html"));
  k!("CREATE USER", "CREATE USER <name> [WITH <attr> ...] -- alias for CREATE ROLE ... LOGIN.", "CREATE USER alice PASSWORD '...';", pg("sql-createuser.html"));
  k!("CREATE GROUP", "CREATE GROUP <name> [WITH <attr> ...] -- alias for CREATE ROLE ... NOLOGIN.", "CREATE GROUP admins;", pg("sql-creategroup.html"));
  k!("CREATE TABLESPACE", "CREATE TABLESPACE <name> [OWNER <role>] LOCATION '<dir>' [WITH (...)].", "CREATE TABLESPACE archive LOCATION '/mnt/cold';", pg("sql-createtablespace.html"));
  k!("CREATE EXTENSION", "CREATE EXTENSION [IF NOT EXISTS] <name> [WITH] [SCHEMA <s>] [VERSION '<v>'] [CASCADE].", "CREATE EXTENSION pgcrypto;", pg("sql-createextension.html"));
  k!("CREATE PUBLICATION", "CREATE PUBLICATION <name> [FOR ALL TABLES | FOR TABLE <t>[, ...] | FOR TABLES IN SCHEMA <s>] [WITH (publish = '...')].", "CREATE PUBLICATION p FOR ALL TABLES;", pg("sql-createpublication.html"));
  k!("CREATE SUBSCRIPTION", "CREATE SUBSCRIPTION <name> CONNECTION '<conn>' PUBLICATION <pub>[, ...] [WITH (...)].", "CREATE SUBSCRIPTION s CONNECTION 'host=... dbname=...' PUBLICATION p;", pg("sql-createsubscription.html"));
  k!("CREATE SERVER", "CREATE SERVER [IF NOT EXISTS] <name> [TYPE '<t>'] [VERSION '<v>'] FOREIGN DATA WRAPPER <fdw> OPTIONS (...).", "CREATE SERVER s FOREIGN DATA WRAPPER postgres_fdw OPTIONS (host 'h');", pg("sql-createserver.html"));
  k!("CREATE TRIGGER", "CREATE TRIGGER <name> {BEFORE|AFTER|INSTEAD OF} <event> ON <table> [FOR EACH {ROW|STATEMENT}] EXECUTE FUNCTION <fn>().", "CREATE TRIGGER trg BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION upd();", pg("sql-createtrigger.html"));
  k!("CREATE TYPE", "CREATE TYPE <name> AS (...) | AS ENUM (...) | AS RANGE (...) | (INPUT = ..., OUTPUT = ...).", "CREATE TYPE status AS ENUM ('open','closed');", pg("sql-createtype.html"));
  k!("CREATE DOMAIN", "CREATE DOMAIN <name> [AS] <base_type> [DEFAULT <expr>] [<constraint>...].", "CREATE DOMAIN email_t AS TEXT CHECK (VALUE ~ '@');", pg("sql-createdomain.html"));
  k!("CREATE POLICY", "CREATE POLICY <name> ON <table> [FOR <cmd>] [TO <role>] [USING (...)] [WITH CHECK (...)].", "CREATE POLICY p ON t FOR SELECT TO public USING (user_id = current_user_id());", pg("sql-createpolicy.html"));
  k!("CREATE FUNCTION", "CREATE FUNCTION <name>(<args>) RETURNS <rettype> AS $$ ... $$ LANGUAGE ...;", "CREATE FUNCTION add(int, int) RETURNS int AS $$ SELECT $1+$2 $$ LANGUAGE SQL;", pg("sql-createfunction.html"));
  k!("CREATE PROCEDURE", "CREATE PROCEDURE <name>(<args>) AS $$ ... $$ LANGUAGE plpgsql;", "CREATE PROCEDURE up() AS $$ BEGIN ... END; $$ LANGUAGE plpgsql;", pg("sql-createprocedure.html"));
  k!("CREATE SEQUENCE", "CREATE SEQUENCE [IF NOT EXISTS] <name> [AS smallint|integer|bigint] [INCREMENT BY <n>] [MINVALUE <n>] [MAXVALUE <n>] [START WITH <n>] [CACHE <n>] [CYCLE|NO CYCLE] [OWNED BY <tbl>.<col>|NONE].", "CREATE SEQUENCE s START 100;", pg("sql-createsequence.html"));
  k!("CREATE RULE", "CREATE [OR REPLACE] RULE <name> AS ON <event> TO <table> [WHERE <expr>] DO {ALSO|INSTEAD} {NOTHING|<cmd>}.", "CREATE RULE r AS ON SELECT TO v DO INSTEAD SELECT 1;", pg("sql-createrule.html"));
  k!("CREATE CAST", "CREATE CAST (src AS dst) {WITH FUNCTION <fn>(args) | WITHOUT FUNCTION | WITH INOUT} [AS ASSIGNMENT | AS IMPLICIT].", "CREATE CAST (text AS my_t) WITH FUNCTION my_in;", pg("sql-createcast.html"));
  k!("CREATE LANGUAGE", "CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE <name> HANDLER <fn> [INLINE <fn>] [VALIDATOR <fn>].", "CREATE LANGUAGE plpython3u;", pg("sql-createlanguage.html"));
  k!("CREATE OPERATOR", "CREATE OPERATOR <op> (FUNCTION = <fn>, LEFTARG = <type>, RIGHTARG = <type>, ...).", "CREATE OPERATOR === (FUNCTION = my_eq, LEFTARG = my_t, RIGHTARG = my_t);", pg("sql-createoperator.html"));
  k!("CREATE AGGREGATE", "CREATE AGGREGATE <name>(<arg_types>) (SFUNC = ..., STYPE = ..., ...).", "CREATE AGGREGATE sum_ints(int) (SFUNC = int4pl, STYPE = int);", pg("sql-createaggregate.html"));
  k!("CREATE CONVERSION", "CREATE [DEFAULT] CONVERSION <name> FOR '<src>' TO '<dst>' FROM <fn>.", "CREATE CONVERSION my_conv FOR 'UTF8' TO 'LATIN1' FROM utf8_to_latin1;", pg("sql-createconversion.html"));
  k!("CREATE COLLATION", "CREATE COLLATION [IF NOT EXISTS] <name> (LOCALE = '<loc>' | LC_COLLATE = '<lc>', LC_CTYPE = '<lc>', ...).", "CREATE COLLATION fr_FR (LOCALE = 'fr_FR.UTF-8');", pg("sql-createcollation.html"));
  k!("CREATE TRANSFORM", "CREATE TRANSFORM FOR <type> LANGUAGE <lang> (FROM SQL WITH FUNCTION <fn>, TO SQL WITH FUNCTION <fn>).", "CREATE TRANSFORM FOR hstore LANGUAGE plpython3u (...);", pg("sql-createtransform.html"));
  k!("CREATE STATISTICS", "CREATE STATISTICS [IF NOT EXISTS] <name> [(<kind>[, ...])] ON <col>[, ...] FROM <table>.", "CREATE STATISTICS s (dependencies) ON a, b FROM t;", pg("sql-createstatistics.html"));
  k!("CREATE FOREIGN DATA WRAPPER", "CREATE FOREIGN DATA WRAPPER <name> [HANDLER <fn>] [VALIDATOR <fn>] [OPTIONS (...)].", "CREATE FOREIGN DATA WRAPPER pg_fdw HANDLER pg_fdw_handler;", pg("sql-createforeigndatawrapper.html"));
  k!("CREATE USER MAPPING", "CREATE USER MAPPING [IF NOT EXISTS] FOR <role> SERVER <srv> OPTIONS (...).", "CREATE USER MAPPING FOR alice SERVER s OPTIONS (user 'a');", pg("sql-createusermapping.html"));
  k!("CREATE TEXT SEARCH CONFIGURATION", "CREATE TEXT SEARCH CONFIGURATION <name> (PARSER = <parser>) -- or COPY = <existing>.", "CREATE TEXT SEARCH CONFIGURATION my (PARSER = default);", pg("sql-createtextsearchconfiguration.html"));
  k!("CREATE TEXT SEARCH DICTIONARY", "CREATE TEXT SEARCH DICTIONARY <name> (TEMPLATE = <template>, ...).", "CREATE TEXT SEARCH DICTIONARY simple_d (TEMPLATE = simple);", pg("sql-createtextsearchdictionary.html"));
  k!("CREATE TEXT SEARCH PARSER", "CREATE TEXT SEARCH PARSER <name> (START = <fn>, GETTOKEN = <fn>, END = <fn>, LEXTYPES = <fn>).", "CREATE TEXT SEARCH PARSER my_p (...);", pg("sql-createtextsearchparser.html"));
  k!("CREATE TEXT SEARCH TEMPLATE", "CREATE TEXT SEARCH TEMPLATE <name> (INIT = <fn>, LEXIZE = <fn>).", "CREATE TEXT SEARCH TEMPLATE my_t (INIT = ..., LEXIZE = ...);", pg("sql-createtextsearchtemplate.html"));


  // ---- round 183 ALTER multi-word starter kws ----
  k!("ALTER FUNCTION", "ALTER FUNCTION <name>(args) ... -- rename / owner / schema / cost / rows / volatility / strict / parallel / leakproof / depends.", "ALTER FUNCTION fn(int) RENAME TO new_fn;", pg("sql-alterfunction.html"));
  k!("ALTER PROCEDURE", "ALTER PROCEDURE <name>(args) ... -- rename / owner / schema / strict.", "ALTER PROCEDURE up() OWNER TO admins;", pg("sql-alterprocedure.html"));
  k!("ALTER ROUTINE", "ALTER ROUTINE <name>(args) ... -- works for both FUNCTION and PROCEDURE.", "ALTER ROUTINE fn(int) RENAME TO new_fn;", pg("sql-alterroutine.html"));
  k!("ALTER INDEX", "ALTER INDEX <name> ... -- rename / owner / set tablespace / attach partition / set (...) / reset (...).", "ALTER INDEX ix RENAME TO ix2;", pg("sql-alterindex.html"));
  k!("ALTER VIEW", "ALTER VIEW <name> ... -- rename / owner / set schema / alter column.", "ALTER VIEW v RENAME TO v2;", pg("sql-alterview.html"));
  k!("ALTER SEQUENCE", "ALTER SEQUENCE <name> ... -- restart / minvalue / maxvalue / owned by / owner / schema.", "ALTER SEQUENCE s RESTART WITH 100;", pg("sql-altersequence.html"));
  k!("ALTER ROLE", "ALTER ROLE <name> ... -- attributes / RENAME / OWNER / SET <param>.", "ALTER ROLE alice CREATEDB;", pg("sql-alterrole.html"));
  k!("ALTER USER", "ALTER USER <name> ... -- alias for ALTER ROLE.", "ALTER USER alice WITH PASSWORD '...';", pg("sql-alteruser.html"));
  k!("ALTER GROUP", "ALTER GROUP <name> ... -- alias for ALTER ROLE for group operations.", "ALTER GROUP admins ADD USER alice;", pg("sql-altergroup.html"));
  k!("ALTER DATABASE", "ALTER DATABASE <name> ... -- rename / owner / set <param> / set tablespace.", "ALTER DATABASE mydb SET search_path = 'public';", pg("sql-alterdatabase.html"));
  k!("ALTER SCHEMA", "ALTER SCHEMA <name> ... -- rename / owner.", "ALTER SCHEMA app RENAME TO app2;", pg("sql-alterschema.html"));
  k!("ALTER EXTENSION", "ALTER EXTENSION <name> ... -- update [TO version] / set schema / add <member> / drop <member>.", "ALTER EXTENSION pgcrypto UPDATE;", pg("sql-alterextension.html"));
  k!("ALTER TRIGGER", "ALTER TRIGGER <name> ON <table> ... -- RENAME / DEPENDS ON EXTENSION.", "ALTER TRIGGER trg ON t RENAME TO trg2;", pg("sql-altertrigger.html"));
  k!("ALTER FUNCTION IF EXISTS", "ALTER FUNCTION IF EXISTS <name>(args) ... -- skip silently if missing.", "ALTER FUNCTION IF EXISTS fn(int) OWNER TO admins;", pg("sql-alterfunction.html"));
  k!("ALTER PROCEDURE IF EXISTS", "ALTER PROCEDURE IF EXISTS <name>(args) ...", "ALTER PROCEDURE IF EXISTS up() OWNER TO admins;", pg("sql-alterprocedure.html"));
  k!("ALTER INDEX IF EXISTS", "ALTER INDEX IF EXISTS <name> ...", "ALTER INDEX IF EXISTS ix RENAME TO ix2;", pg("sql-alterindex.html"));
  k!("ALTER MATERIALIZED VIEW IF EXISTS", "ALTER MATERIALIZED VIEW IF EXISTS <name> ...", "ALTER MATERIALIZED VIEW IF EXISTS mv RENAME TO mv2;", pg("sql-altermaterializedview.html"));
  k!("ALTER FOREIGN TABLE IF EXISTS", "ALTER FOREIGN TABLE IF EXISTS <name> ...", "ALTER FOREIGN TABLE IF EXISTS ext OPTIONS (ADD foo 'bar');", pg("sql-alterforeigntable.html"));
  k!("ALTER TYPE IF EXISTS", "ALTER TYPE IF EXISTS <name> ...", "ALTER TYPE IF EXISTS status ADD VALUE 'archived';", pg("sql-altertype.html"));
  k!("ALTER DOMAIN IF EXISTS", "ALTER DOMAIN IF EXISTS <name> ...", "ALTER DOMAIN IF EXISTS email_t SET DEFAULT NULL;", pg("sql-alterdomain.html"));


  // ---- round 184 GRANT / REVOKE multi-word starter kws ----
  k!("GRANT ALL", "GRANT ALL [PRIVILEGES] ON <target> TO <role> -- shorthand for all relevant privileges.", "GRANT ALL ON TABLE users TO admins;", pg("sql-grant.html"));
  k!("GRANT ALL PRIVILEGES", "GRANT ALL PRIVILEGES ON <target> TO <role> -- explicit form of GRANT ALL.", "GRANT ALL PRIVILEGES ON TABLE users TO admins;", pg("sql-grant.html"));
  k!("GRANT SELECT", "GRANT SELECT ON <table>[, ...] | <view> | <sequence> TO <role>.", "GRANT SELECT ON users TO readonly;", pg("sql-grant.html"));
  k!("GRANT INSERT", "GRANT INSERT ON <table>[, ...] TO <role>.", "GRANT INSERT ON users TO app;", pg("sql-grant.html"));
  k!("GRANT UPDATE", "GRANT UPDATE [(col[, ...])] ON <table> TO <role>.", "GRANT UPDATE (email) ON users TO app;", pg("sql-grant.html"));
  k!("GRANT DELETE", "GRANT DELETE ON <table>[, ...] TO <role>.", "GRANT DELETE ON staging TO app;", pg("sql-grant.html"));
  k!("GRANT TRUNCATE", "GRANT TRUNCATE ON <table>[, ...] TO <role>.", "GRANT TRUNCATE ON staging TO app;", pg("sql-grant.html"));
  k!("GRANT REFERENCES", "GRANT REFERENCES [(col[, ...])] ON <table> TO <role> -- allow FK creation.", "GRANT REFERENCES ON users TO app;", pg("sql-grant.html"));
  k!("GRANT TRIGGER", "GRANT TRIGGER ON <table>[, ...] TO <role> -- allow CREATE TRIGGER on the table.", "GRANT TRIGGER ON users TO app;", pg("sql-grant.html"));
  k!("GRANT USAGE", "GRANT USAGE ON SCHEMA / SEQUENCE / LANGUAGE / TYPE TO <role>.", "GRANT USAGE ON SCHEMA public TO app;", pg("sql-grant.html"));
  k!("GRANT EXECUTE", "GRANT EXECUTE ON FUNCTION / PROCEDURE / ROUTINE TO <role>.", "GRANT EXECUTE ON FUNCTION add(int, int) TO app;", pg("sql-grant.html"));
  k!("GRANT CONNECT", "GRANT CONNECT ON DATABASE <name> TO <role>.", "GRANT CONNECT ON DATABASE mydb TO app;", pg("sql-grant.html"));
  k!("GRANT TEMPORARY", "GRANT TEMPORARY ON DATABASE <name> TO <role> -- allow creating temp tables.", "GRANT TEMPORARY ON DATABASE mydb TO app;", pg("sql-grant.html"));
  k!("GRANT CREATE", "GRANT CREATE ON DATABASE / SCHEMA / TABLESPACE TO <role>.", "GRANT CREATE ON DATABASE mydb TO app;", pg("sql-grant.html"));
  k!("GRANT MAINTAIN", "GRANT MAINTAIN ON <table> TO <role> -- run VACUUM/ANALYZE/CLUSTER/REINDEX/LOCK (PG17+).", "GRANT MAINTAIN ON ALL TABLES IN SCHEMA public TO admins;", pg("sql-grant.html"));
  k!("REVOKE ALL", "REVOKE ALL [PRIVILEGES] ON <target> FROM <role>.", "REVOKE ALL ON TABLE users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE ALL PRIVILEGES", "REVOKE ALL PRIVILEGES ON <target> FROM <role>.", "REVOKE ALL PRIVILEGES ON TABLE users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE SELECT", "REVOKE SELECT ON <target> FROM <role>.", "REVOKE SELECT ON users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE INSERT", "REVOKE INSERT ON <target> FROM <role>.", "REVOKE INSERT ON users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE UPDATE", "REVOKE UPDATE [(col[, ...])] ON <target> FROM <role>.", "REVOKE UPDATE (email) ON users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE DELETE", "REVOKE DELETE ON <target> FROM <role>.", "REVOKE DELETE ON users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE USAGE", "REVOKE USAGE ON <target> FROM <role>.", "REVOKE USAGE ON SCHEMA public FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE EXECUTE", "REVOKE EXECUTE ON FUNCTION / PROCEDURE FROM <role>.", "REVOKE EXECUTE ON FUNCTION add(int, int) FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE GRANT OPTION FOR", "REVOKE GRANT OPTION FOR <priv> ON <target> FROM <role> -- revoke ability to re-grant.", "REVOKE GRANT OPTION FOR SELECT ON users FROM bob;", pg("sql-revoke.html"));
  k!("REVOKE ADMIN OPTION FOR", "REVOKE ADMIN OPTION FOR <role> FROM <member> -- revoke ability to add members.", "REVOKE ADMIN OPTION FOR admins FROM alice;", pg("sql-revoke.html"));


  // ---- round 185 JOIN multi-word kws + inheritance ----
  k!("FROM ONLY", "SELECT/UPDATE/DELETE ... FROM ONLY <parent> -- skip inherited child tables.", "DELETE FROM ONLY parent WHERE id = 1;", pg("ddl-inherit.html"));
  k!("FULL JOIN", "FULL JOIN <table> ON <pred> -- FULL OUTER JOIN.", "SELECT * FROM a FULL JOIN b ON a.id = b.id;", pg("queries-table-expressions.html#QUERIES-FROM"));
  k!("LEFT OUTER JOIN", "Same as LEFT JOIN -- explicit OUTER form.", "SELECT * FROM a LEFT OUTER JOIN b ON a.id = b.id;", pg("queries-table-expressions.html"));
  k!("RIGHT OUTER JOIN", "Same as RIGHT JOIN -- explicit OUTER form.", "SELECT * FROM a RIGHT OUTER JOIN b ON a.id = b.id;", pg("queries-table-expressions.html"));
  k!("NATURAL JOIN", "NATURAL [INNER|LEFT|RIGHT|FULL] JOIN <table> -- auto-join on common column names. Brittle.", "SELECT * FROM a NATURAL JOIN b;", pg("queries-table-expressions.html"));
  k!("NATURAL INNER JOIN", "NATURAL INNER JOIN <table> -- explicit form of NATURAL JOIN.", "SELECT * FROM a NATURAL INNER JOIN b;", pg("queries-table-expressions.html"));
  k!("JOIN LATERAL", "JOIN LATERAL (subq) ON true -- per-row correlated subquery.", "SELECT * FROM users u JOIN LATERAL (SELECT * FROM orders WHERE user_id = u.id LIMIT 5) o ON true;", pg("queries-table-expressions.html#QUERIES-LATERAL"));
  k!("LEFT JOIN LATERAL", "LEFT JOIN LATERAL (subq) ON true -- correlated subquery without filtering null rows.", "SELECT * FROM users u LEFT JOIN LATERAL (SELECT * FROM orders WHERE user_id = u.id LIMIT 1) o ON true;", pg("queries-table-expressions.html#QUERIES-LATERAL"));
  k!("INNER JOIN LATERAL", "INNER JOIN LATERAL (subq) ON true -- correlated subquery, inner-join filter on rows.", "FROM users u INNER JOIN LATERAL fn(u.id) ON true", pg("queries-table-expressions.html#QUERIES-LATERAL"));
  k!("CROSS JOIN LATERAL", "FROM ... CROSS JOIN LATERAL (subq) -- per-row correlated subquery without ON.", "FROM users u CROSS JOIN LATERAL fn(u.id)", pg("queries-table-expressions.html#QUERIES-LATERAL"));
  k!("JOIN ON", "JOIN ... ON <pred> -- explicit join condition.", "SELECT * FROM a JOIN b ON a.id = b.id;", pg("queries-table-expressions.html"));
  k!("JOIN USING", "JOIN ... USING (col[, ...]) -- common column equality.", "SELECT * FROM a JOIN b USING (id);", pg("queries-table-expressions.html"));
  k!("LEFT JOIN ON", "LEFT JOIN ... ON <pred>.", "SELECT * FROM a LEFT JOIN b ON a.id = b.id;", pg("queries-table-expressions.html"));
  k!("INNER JOIN ON", "INNER JOIN ... ON <pred>.", "SELECT * FROM a INNER JOIN b ON a.id = b.id;", pg("queries-table-expressions.html"));
  k!("LEFT JOIN USING", "LEFT JOIN ... USING (col[, ...]).", "SELECT * FROM a LEFT JOIN b USING (id);", pg("queries-table-expressions.html"));
  k!("INNER JOIN USING", "INNER JOIN ... USING (col[, ...]).", "SELECT * FROM a INNER JOIN b USING (id);", pg("queries-table-expressions.html"));
  k!("DELETE FROM ONLY", "DELETE FROM ONLY <parent> -- skip child tables in inheritance.", "DELETE FROM ONLY parent WHERE id = 1;", pg("sql-delete.html"));
  k!("UPDATE ONLY", "UPDATE ONLY <parent> SET ... -- skip child tables.", "UPDATE ONLY parent SET x = 1 WHERE id = 1;", pg("sql-update.html"));
  k!("TRUNCATE ONLY", "TRUNCATE ONLY <parent> -- skip child tables.", "TRUNCATE ONLY parent;", pg("sql-truncate.html"));


  // ---- round 186 MERGE + ON CONFLICT multi-word kws ----
  k!("MERGE INTO", "MERGE INTO <target> USING <source> ON <pred> WHEN ... THEN ... -- SQL standard upsert.", "MERGE INTO users u USING staging s ON s.id = u.id WHEN MATCHED THEN UPDATE SET email = s.email;", pg("sql-merge.html"));
  k!("USING TABLE", "MERGE INTO ... USING <table> ON ... -- source is a table.", "MERGE INTO users USING staging ON staging.id = users.id ...", pg("sql-merge.html"));
  k!("USING SELECT", "MERGE INTO ... USING (SELECT ...) <alias> ON ... -- source is a subquery.", "MERGE INTO t USING (SELECT * FROM s) src ON src.id = t.id ...", pg("sql-merge.html"));
  k!("WHEN MATCHED", "MERGE ... WHEN MATCHED [AND <expr>] THEN { UPDATE SET ... | DELETE | DO NOTHING }.", "WHEN MATCHED THEN UPDATE SET ...", pg("sql-merge.html"));
  k!("WHEN NOT MATCHED", "MERGE ... WHEN NOT MATCHED [AND <expr>] THEN { INSERT ... | DO NOTHING }.", "WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id);", pg("sql-merge.html"));
  k!("WHEN MATCHED THEN", "Same as WHEN MATCHED -- THEN introduces the action.", "WHEN MATCHED THEN UPDATE SET ...", pg("sql-merge.html"));
  k!("WHEN NOT MATCHED THEN", "Same as WHEN NOT MATCHED -- THEN introduces the action.", "WHEN NOT MATCHED THEN INSERT ...", pg("sql-merge.html"));
  k!("WHEN MATCHED AND", "WHEN MATCHED AND <pred> THEN <action> -- conditional MERGE branch.", "WHEN MATCHED AND s.deleted_at IS NOT NULL THEN DELETE", pg("sql-merge.html"));
  k!("WHEN NOT MATCHED AND", "WHEN NOT MATCHED AND <pred> THEN <action> -- conditional MERGE branch.", "WHEN NOT MATCHED AND s.is_active THEN INSERT (id) VALUES (s.id)", pg("sql-merge.html"));
  k!("DO UPDATE SET", "ON CONFLICT ... DO UPDATE SET <col> = <val>[, ...].", "ON CONFLICT (email) DO UPDATE SET updated_at = excluded.updated_at;", pg("sql-insert.html"));
  k!("ON CONFLICT DO NOTHING", "INSERT ... ON CONFLICT DO NOTHING -- skip conflicting rows.", "INSERT INTO t VALUES (...) ON CONFLICT DO NOTHING;", pg("sql-insert.html"));
  k!("ON CONFLICT DO UPDATE", "INSERT ... ON CONFLICT [...] DO UPDATE SET ... -- classic upsert.", "INSERT INTO t (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = excluded.name;", pg("sql-insert.html"));
  k!("ON CONFLICT DO UPDATE SET", "Same as ON CONFLICT DO UPDATE -- SET introduces assignments.", "ON CONFLICT (id) DO UPDATE SET name = excluded.name;", pg("sql-insert.html"));
  k!("WHEN NOT MATCHED BY SOURCE", "MERGE ... WHEN NOT MATCHED BY SOURCE -- PG17+: target rows without a source match.", "WHEN NOT MATCHED BY SOURCE THEN DELETE", pg("sql-merge.html"));
  k!("WHEN NOT MATCHED BY TARGET", "MERGE ... WHEN NOT MATCHED BY TARGET (alias for default WHEN NOT MATCHED).", "WHEN NOT MATCHED BY TARGET THEN INSERT (id) VALUES (s.id)", pg("sql-merge.html"));
  k!("ON CONFLICT ON CONSTRAINT", "INSERT ... ON CONFLICT ON CONSTRAINT <name> DO ... -- target a specific named constraint.", "ON CONFLICT ON CONSTRAINT uq_email DO UPDATE SET ...", pg("sql-insert.html"));
  k!("RETURNING ALL", "INSERT/UPDATE/DELETE/MERGE ... RETURNING * -- conventional shorthand for all cols.", "INSERT INTO t VALUES (...) RETURNING *;", pg("sql-insert.html"));
  // ---- PG18 RETURNING OLD / NEW rows ----
  k!("RETURNING OLD", "PG18+ RETURNING OLD.<col> / NEW.<col>: see pre- and post-image of the row touched by UPDATE/DELETE/MERGE. The OLD/NEW aliases must be qualified.", "UPDATE t SET v = v + 1 WHERE id = 1 RETURNING OLD.v, NEW.v;", pg("sql-update.html"));
  k!("RETURNING NEW", "PG18+ RETURNING NEW.<col>: the post-image of the row. Combine with RETURNING OLD to see the diff.", "UPDATE t SET v = v + 1 RETURNING NEW.v AS new_v, OLD.v AS old_v;", pg("sql-update.html"));
  k!("OLD.", "RETURNING OLD.<col>: row image before the UPDATE/DELETE/MERGE action (PG18+).", "UPDATE t SET v = v + 1 RETURNING OLD.v;", pg("sql-update.html"));
  k!("NEW.", "RETURNING NEW.<col>: row image after the UPDATE/INSERT/MERGE action (PG18+).", "UPDATE t SET v = v + 1 RETURNING NEW.v;", pg("sql-update.html"));
  // ---- PG18 virtual generated columns ----
  k!("GENERATED ALWAYS AS VIRTUAL", "PG18+ virtual generated column -- computed on read, no storage. Default for GENERATED ALWAYS AS unless STORED is specified.", "CREATE TABLE t (a int, b int GENERATED ALWAYS AS (a * 2) VIRTUAL);", pg("ddl-generated-columns.html"));
  k!("VIRTUAL", "PG18+ generated-column storage class: computed on read, no disk footprint. Counterpart of STORED.", "GENERATED ALWAYS AS (...) VIRTUAL", pg("ddl-generated-columns.html"));
  // ---- PG17 MAINTAIN privilege ----
  k!("MAINTAIN", "PG17+ table privilege granting VACUUM/ANALYZE/REINDEX/REFRESH MATERIALIZED VIEW/CLUSTER/LOCK TABLE without ownership. Useful for monitoring/cron roles.", "GRANT MAINTAIN ON TABLE users TO monitor_role;", pg("ddl-priv.html"));
  // ---- PG17 EXPLAIN options ----
  k!("EXPLAIN SERIALIZE", "PG17+ EXPLAIN (ANALYZE, SERIALIZE) -- measure cost of serializing the result to the wire format (binary or text).", "EXPLAIN (ANALYZE, SERIALIZE) SELECT * FROM big_table;", pg("sql-explain.html"));
  k!("SERIALIZE binary", "EXPLAIN (SERIALIZE binary) -- account for serialization to BINARY wire format.", "EXPLAIN (ANALYZE, SERIALIZE binary) SELECT ...;", pg("sql-explain.html"));
  k!("SERIALIZE text", "EXPLAIN (SERIALIZE text) -- account for serialization to TEXT wire format.", "EXPLAIN (ANALYZE, SERIALIZE text) SELECT ...;", pg("sql-explain.html"));
  k!("EXPLAIN MEMORY", "PG17+ EXPLAIN (MEMORY) -- report planner / executor memory usage.", "EXPLAIN (ANALYZE, MEMORY) SELECT ...;", pg("sql-explain.html"));
  // ---- LARGE OBJECT improvements ----
  k!("LARGE OBJECTS IN SCHEMA", "GRANT ... ON ALL LARGE OBJECTS IN SCHEMA <schema> -- batch grant across all LOs in a schema (PG17+).", "GRANT SELECT ON ALL LARGE OBJECTS IN SCHEMA public TO ro_role;", pg("sql-grant.html"));
  k!("ALL LARGE OBJECTS", "GRANT/REVOKE ON ALL LARGE OBJECTS -- bulk LO privilege change.", "GRANT SELECT ON ALL LARGE OBJECTS TO ro;", pg("sql-grant.html"));


  // ---- round 187 cast / identity multi-word kws ----
  k!("CAST AS", "CAST(<expr> AS <type>) -- explicit type conversion.", "SELECT CAST(price AS numeric(10,2)) FROM t;", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  k!("AS DECIMAL", "CAST(... AS DECIMAL(p, s)) -- alias of NUMERIC.", "SELECT CAST(x AS DECIMAL(10,2));", pg("datatype-numeric.html"));
  k!("AS TEXT", "CAST(... AS TEXT) -- to TEXT.", "SELECT CAST(id AS TEXT);", pg("datatype-character.html"));
  k!("AS NUMERIC", "CAST(... AS NUMERIC(p, s)) -- exact decimal.", "SELECT CAST(x AS NUMERIC(10,2));", pg("datatype-numeric.html"));
  k!("AS INT", "CAST(... AS INT) -- alias of INTEGER.", "SELECT CAST(price AS INT);", pg("datatype-numeric.html"));
  k!("AS BIGINT", "CAST(... AS BIGINT) -- 8-byte signed integer.", "SELECT CAST(price AS BIGINT);", pg("datatype-numeric.html"));
  k!("AS BOOLEAN", "CAST(... AS BOOLEAN) -- truthiness conversion.", "SELECT CAST(flag AS BOOLEAN);", pg("datatype-boolean.html"));
  k!("AS JSONB", "CAST(... AS JSONB) -- to binary JSON.", "SELECT CAST(payload AS JSONB);", pg("datatype-json.html"));
  k!("AS JSON", "CAST(... AS JSON) -- to text JSON (rarely needed; prefer JSONB).", "SELECT CAST(payload AS JSON);", pg("datatype-json.html"));
  k!("AS DATE", "CAST(... AS DATE) -- strip time component.", "SELECT CAST(ts AS DATE);", pg("datatype-datetime.html"));
  k!("AS TIME", "CAST(... AS TIME) -- time of day.", "SELECT CAST(ts AS TIME);", pg("datatype-datetime.html"));
  k!("AS TIMESTAMP", "CAST(... AS TIMESTAMP) -- timestamp WITHOUT time zone.", "SELECT CAST(ts AS TIMESTAMP);", pg("datatype-datetime.html"));
  k!("AS TIMESTAMPTZ", "CAST(... AS TIMESTAMPTZ) -- timestamp WITH time zone.", "SELECT CAST(ts AS TIMESTAMPTZ);", pg("datatype-datetime.html"));
  k!("GENERATED ALWAYS AS IDENTITY", "<col> <type> GENERATED ALWAYS AS IDENTITY -- system-generated identity, OVERRIDING SYSTEM VALUE required to override.", "id int GENERATED ALWAYS AS IDENTITY PRIMARY KEY", pg("sql-createtable.html"));
  k!("GENERATED BY DEFAULT AS IDENTITY", "<col> <type> GENERATED BY DEFAULT AS IDENTITY -- system-generated identity, user value accepted.", "id int GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY", pg("sql-createtable.html"));


  // ---- round 188 sort + predicate multi-word kws ----
  k!("ASC NULLS FIRST", "ORDER BY <col> ASC NULLS FIRST -- ascending with NULLs first.", "SELECT * FROM t ORDER BY id ASC NULLS FIRST;", pg("sql-select.html"));
  k!("ASC NULLS LAST", "ORDER BY <col> ASC NULLS LAST -- ascending with NULLs last (default for ASC).", "SELECT * FROM t ORDER BY id ASC NULLS LAST;", pg("sql-select.html"));
  k!("DESC NULLS FIRST", "ORDER BY <col> DESC NULLS FIRST -- descending with NULLs first (default for DESC).", "SELECT * FROM t ORDER BY id DESC NULLS FIRST;", pg("sql-select.html"));
  k!("DESC NULLS LAST", "ORDER BY <col> DESC NULLS LAST -- descending with NULLs last.", "SELECT * FROM t ORDER BY id DESC NULLS LAST;", pg("sql-select.html"));
  k!("ORDER BY ALL", "ORDER BY ALL -- non-PG extension (Snowflake/MySQL): order by every output column.", "-- unsupported in PG", pg("sql-select.html"));
  k!("BETWEEN SYMMETRIC", "<expr> BETWEEN SYMMETRIC <a> AND <b> -- swap a/b if a > b before checking.", "WHERE age BETWEEN SYMMETRIC 18 AND 65", pg("functions-comparison.html"));
  k!("BETWEEN ASYMMETRIC", "<expr> BETWEEN ASYMMETRIC <a> AND <b> -- default; refuse if a > b.", "WHERE age BETWEEN ASYMMETRIC 18 AND 65", pg("functions-comparison.html"));
  k!("NOT BETWEEN", "<expr> NOT BETWEEN <a> AND <b>.", "WHERE age NOT BETWEEN 18 AND 65", pg("functions-comparison.html"));
  k!("NOT BETWEEN SYMMETRIC", "<expr> NOT BETWEEN SYMMETRIC <a> AND <b>.", "WHERE age NOT BETWEEN SYMMETRIC 18 AND 65", pg("functions-comparison.html"));
  k!("NOT LIKE", "<expr> NOT LIKE '<pattern>' -- negated LIKE.", "WHERE email NOT LIKE '%@example.com'", pg("functions-matching.html"));
  k!("NOT ILIKE", "<expr> NOT ILIKE '<pattern>' -- negated case-insensitive LIKE.", "WHERE email NOT ILIKE '%@example.com'", pg("functions-matching.html"));
  k!("NOT SIMILAR TO", "<expr> NOT SIMILAR TO '<regex>' -- negated SIMILAR TO.", "WHERE name NOT SIMILAR TO '%(foo|bar)%'", pg("functions-matching.html"));
  k!("NOT IN", "<expr> NOT IN (<list> | <subq>) -- negated IN.", "WHERE id NOT IN (1, 2, 3)", pg("functions-comparison.html"));
  k!("NOT EXISTS", "NOT EXISTS (<subquery>) -- negated EXISTS.", "WHERE NOT EXISTS (SELECT 1 FROM t WHERE t.id = x.id)", pg("functions-comparison.html"));
  k!("ANY ARRAY", "<expr> = ANY (<array>) -- match any element.", "WHERE id = ANY ('{1,2,3}'::int[])", pg("functions-comparisons.html"));
  k!("ALL ARRAY", "<expr> <> ALL (<array>) -- distinct from every element.", "WHERE id <> ALL ('{1,2,3}'::int[])", pg("functions-comparisons.html"));
  k!("OFFSET ROWS", "OFFSET <n> ROWS -- SQL standard form of OFFSET.", "SELECT * FROM t OFFSET 20 ROWS;", pg("sql-select.html"));
  k!("OFFSET ROW", "OFFSET <n> ROW -- SQL standard form of OFFSET (singular form).", "SELECT * FROM t OFFSET 1 ROW;", pg("sql-select.html"));
  k!("FIRST ROWS", "FETCH FIRST <n> ROWS -- SQL standard limit clause.", "SELECT * FROM t FETCH FIRST 10 ROWS ONLY;", pg("sql-select.html"));
  k!("FIRST ROW", "FETCH FIRST <n> ROW -- SQL standard limit clause (singular form).", "SELECT * FROM t FETCH FIRST 1 ROW ONLY;", pg("sql-select.html"));
  k!("NEXT ROWS", "FETCH NEXT <n> ROWS -- SQL standard limit clause.", "SELECT * FROM t FETCH NEXT 10 ROWS ONLY;", pg("sql-select.html"));
  k!("NEXT ROW", "FETCH NEXT <n> ROW -- SQL standard limit clause (singular form).", "SELECT * FROM t FETCH NEXT 1 ROW ONLY;", pg("sql-select.html"));


  // ---- round 189 IS predicates + string builtins multi-word kws ----
  k!("IS TRUE", "<expr> IS TRUE -- handles NULL as NOT TRUE.", "WHERE flag IS TRUE", pg("functions-comparison.html"));
  k!("IS NOT TRUE", "<expr> IS NOT TRUE -- true when NULL or false.", "WHERE flag IS NOT TRUE", pg("functions-comparison.html"));
  k!("IS FALSE", "<expr> IS FALSE.", "WHERE flag IS FALSE", pg("functions-comparison.html"));
  k!("IS NOT FALSE", "<expr> IS NOT FALSE -- true when NULL or true.", "WHERE flag IS NOT FALSE", pg("functions-comparison.html"));
  k!("IS UNKNOWN", "<expr> IS UNKNOWN -- same as IS NULL for boolean expressions.", "WHERE flag IS UNKNOWN", pg("functions-comparison.html"));
  k!("IS NOT UNKNOWN", "<expr> IS NOT UNKNOWN -- same as IS NOT NULL for boolean expressions.", "WHERE flag IS NOT UNKNOWN", pg("functions-comparison.html"));
  k!("IS DOCUMENT", "<xml> IS DOCUMENT -- true when value is a well-formed XML document.", "WHERE col IS DOCUMENT", pg("functions-xml.html"));
  k!("IS NOT DOCUMENT", "<xml> IS NOT DOCUMENT.", "WHERE col IS NOT DOCUMENT", pg("functions-xml.html"));
  k!("IS NORMALIZED", "<text> IS [NFC|NFD|NFKC|NFKD] NORMALIZED -- Unicode normalization test (PG13+).", "WHERE col IS NFC NORMALIZED", pg("functions-string.html"));
  k!("IS NOT NORMALIZED", "<text> IS NOT [NFC|NFD|NFKC|NFKD] NORMALIZED.", "WHERE col IS NOT NORMALIZED", pg("functions-string.html"));
  k!("IS OF", "<expr> IS OF (type[, ...]) -- type membership check (rarely used).", "WHERE x IS OF (text, varchar)", pg("functions-comparison.html"));
  k!("IS NOT OF", "<expr> IS NOT OF (type[, ...]).", "WHERE x IS NOT OF (text)", pg("functions-comparison.html"));
  k!("EXTRACT FROM", "EXTRACT(<field> FROM <source>) -- extract date/time field.", "SELECT EXTRACT(YEAR FROM now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  k!("OVERLAY PLACING", "OVERLAY(<src> PLACING <new> FROM <start> [FOR <count>]) -- splice.", "SELECT OVERLAY('Txxxxas' PLACING 'hom' FROM 2 FOR 4);", pg("functions-string.html"));
  k!("POSITION IN", "POSITION(<needle> IN <haystack>) -- alias of strpos.", "SELECT POSITION('o' IN 'foo');", pg("functions-string.html"));
  k!("SUBSTRING FROM", "SUBSTRING(<src> FROM <start>) -- alias of substr.", "SELECT SUBSTRING('foobar' FROM 4);", pg("functions-string.html"));
  k!("SUBSTRING FOR", "SUBSTRING(<src> FOR <count>) -- alias of left.", "SELECT SUBSTRING('foobar' FOR 3);", pg("functions-string.html"));
  k!("TRIM FROM", "TRIM([BOTH|LEADING|TRAILING] [<chars>] FROM <text>).", "SELECT TRIM(BOTH 'x' FROM 'xxhellxx');", pg("functions-string.html"));
  k!("TRIM LEADING", "TRIM(LEADING [<chars>] FROM <text>).", "SELECT TRIM(LEADING ' ' FROM '  hello');", pg("functions-string.html"));
  k!("TRIM TRAILING", "TRIM(TRAILING [<chars>] FROM <text>).", "SELECT TRIM(TRAILING ' ' FROM 'hello  ');", pg("functions-string.html"));
  k!("TRIM BOTH", "TRIM(BOTH [<chars>] FROM <text>) -- same as default TRIM.", "SELECT TRIM(BOTH ' ' FROM '  hello  ');", pg("functions-string.html"));
  k!("COLLATION FOR", "COLLATION FOR (<expr>) -> text -- collation name derived from expression.", "SELECT COLLATION FOR ('hi'::text);", pg("functions-info.html"));
  k!("TYPE COERCION", "PG type coercion -- documented in the type-cast grammar; not a standalone keyword phrase.", "-- see CAST / ::", pg("typeconv.html"));


  // ---- round 190 final-stretch multi-word kws ----
  k!("XML PARSE", "XMLPARSE({DOCUMENT|CONTENT} <text>) -- parse text as XML.", "SELECT XMLPARSE(DOCUMENT '<r/>');", pg("functions-xml.html"));
  k!("XML SERIALIZE", "XMLSERIALIZE({DOCUMENT|CONTENT} <xml> AS <text_type>).", "SELECT XMLSERIALIZE(DOCUMENT payload AS text);", pg("functions-xml.html"));
  k!("WITHIN GROUP ORDER BY", "<agg>(...) WITHIN GROUP (ORDER BY <key>) -- ordered-set aggregate.", "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY salary) FROM emp;", pg("functions-aggregate.html"));
  k!("FILTER WHERE", "<agg>(...) FILTER (WHERE <pred>) -- selectively count.", "SELECT count(*) FILTER (WHERE flag) FROM t;", pg("sql-expressions.html#SYNTAX-AGGREGATES"));
  k!("OVER WINDOW", "<win_fn>(...) OVER <window_name> -- reference a named WINDOW clause.", "SELECT id, row_number() OVER w FROM t WINDOW w AS (PARTITION BY uid ORDER BY ts);", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("OVER PARTITION", "<win_fn>(...) OVER (PARTITION BY ...) -- inline window definition.", "SELECT row_number() OVER (PARTITION BY uid ORDER BY ts) FROM t;", pg("sql-expressions.html#SYNTAX-WINDOW-FUNCTIONS"));
  k!("LIMIT ALL", "LIMIT ALL -- explicit no-limit.", "SELECT * FROM t LIMIT ALL;", pg("sql-select.html"));
  k!("LIMIT NULL", "LIMIT NULL -- same as LIMIT ALL (no limit).", "SELECT * FROM t LIMIT NULL;", pg("sql-select.html"));
  k!("FETCH FIRST ROW ONLY", "FETCH FIRST 1 ROW ONLY -- singular form.", "SELECT * FROM t FETCH FIRST 1 ROW ONLY;", pg("sql-select.html"));
  k!("FETCH NEXT ROW ONLY", "FETCH NEXT 1 ROW ONLY.", "SELECT * FROM t FETCH NEXT 1 ROW ONLY;", pg("sql-select.html"));
  k!("FETCH FIRST ROWS ONLY", "FETCH FIRST <n> ROWS ONLY.", "SELECT * FROM t FETCH FIRST 10 ROWS ONLY;", pg("sql-select.html"));
  k!("FETCH NEXT ROWS ONLY", "FETCH NEXT <n> ROWS ONLY.", "SELECT * FROM t FETCH NEXT 10 ROWS ONLY;", pg("sql-select.html"));


  // ---- round 191 final multi-word kw audit ----
  k!("WITH OIDS", "CREATE TABLE ... WITH OIDS -- removed in PG12+. Modern code uses ctid.", "-- legacy: CREATE TABLE t (...) WITH OIDS;", pg("sql-createtable.html"));
  k!("WITHOUT OIDS", "CREATE TABLE ... WITHOUT OIDS -- default since PG12+ (the WITH OIDS option is gone).", "CREATE TABLE t (...) WITHOUT OIDS;", pg("sql-createtable.html"));
  k!("WITH OWNER", "CREATE DATABASE ... [WITH] OWNER <role>.", "CREATE DATABASE mydb OWNER alice;", pg("sql-createdatabase.html"));
  k!("OWNED BY NONE", "ALTER SEQUENCE ... OWNED BY NONE -- detach the sequence from any column.", "ALTER SEQUENCE s OWNED BY NONE;", pg("sql-altersequence.html"));
  k!("AS ON", "CREATE RULE ... AS ON <event> -- AS introduces the ON event in CREATE RULE.", "CREATE RULE r AS ON SELECT TO v DO INSTEAD SELECT 1;", pg("sql-createrule.html"));
  k!("DO INSTEAD", "CREATE RULE ... DO INSTEAD { NOTHING | <command> } -- replace original DML with this action.", "DO INSTEAD NOTHING", pg("sql-createrule.html"));
  k!("DO INSTEAD NOTHING", "CREATE RULE ... DO INSTEAD NOTHING -- swallow the original DML.", "CREATE RULE r AS ON INSERT TO v DO INSTEAD NOTHING;", pg("sql-createrule.html"));
  k!("DO ALSO", "CREATE RULE ... DO ALSO <command> -- run an extra action alongside the DML.", "CREATE RULE r AS ON INSERT TO v DO ALSO INSERT INTO audit ...;", pg("sql-createrule.html"));
  k!("ON SELECT", "CREATE RULE / VIEW ... ON SELECT -- VIEW-implementing rule.", "CREATE RULE _RETURN AS ON SELECT TO v DO INSTEAD SELECT ...;", pg("sql-createrule.html"));
  k!("ON INSERT", "CREATE RULE ... ON INSERT TO <table> DO ... -- INSERT-event rule.", "CREATE RULE r AS ON INSERT TO v DO ALSO log_insert();", pg("sql-createrule.html"));
  k!("EVENT TRIGGER DDL_COMMAND_START", "Event trigger event firing before a DDL.", "CREATE EVENT TRIGGER trg ON ddl_command_start EXECUTE FUNCTION fn();", pg("event-trigger-overview.html"));
  k!("EVENT TRIGGER DDL_COMMAND_END", "Event trigger event firing after a DDL.", "CREATE EVENT TRIGGER trg ON ddl_command_end EXECUTE FUNCTION fn();", pg("event-trigger-overview.html"));
  k!("EVENT TRIGGER SQL_DROP", "Event trigger event firing after a DROP.", "CREATE EVENT TRIGGER trg ON sql_drop EXECUTE FUNCTION audit_drop();", pg("event-trigger-overview.html"));
  k!("EVENT TRIGGER TABLE_REWRITE", "Event trigger event firing when a DDL rewrites a table.", "CREATE EVENT TRIGGER trg ON table_rewrite EXECUTE FUNCTION audit_rewrite();", pg("event-trigger-overview.html"));
  k!("DEFAULT NULL", "<col> <type> DEFAULT NULL -- explicit null default.", "deleted_at timestamptz DEFAULT NULL", pg("sql-createtable.html"));
  k!("DEFAULT TRUE", "<col> boolean DEFAULT TRUE.", "active boolean DEFAULT TRUE", pg("sql-createtable.html"));
  k!("DEFAULT FALSE", "<col> boolean DEFAULT FALSE.", "deleted boolean DEFAULT FALSE", pg("sql-createtable.html"));
  k!("CHECK NOT VALID", "ADD CONSTRAINT ... CHECK (...) NOT VALID -- skip the up-front scan.", "ALTER TABLE t ADD CONSTRAINT ck_age CHECK (age >= 0) NOT VALID;", pg("sql-altertable.html"));
  k!("ADD CHECK", "ALTER TABLE ... ADD CHECK (<expr>) -- unnamed check constraint.", "ALTER TABLE t ADD CHECK (price > 0);", pg("sql-altertable.html"));
  k!("ADD UNIQUE", "ALTER TABLE ... ADD UNIQUE (<cols>) -- unnamed unique constraint.", "ALTER TABLE t ADD UNIQUE (email);", pg("sql-altertable.html"));
  k!("ADD PRIMARY KEY", "ALTER TABLE ... ADD PRIMARY KEY (<cols>) -- unnamed primary key.", "ALTER TABLE t ADD PRIMARY KEY (id);", pg("sql-altertable.html"));
  k!("ADD FOREIGN KEY", "ALTER TABLE ... ADD FOREIGN KEY (<cols>) REFERENCES <other>(<cols>) -- unnamed FK.", "ALTER TABLE child ADD FOREIGN KEY (parent_id) REFERENCES parent(id);", pg("sql-altertable.html"));
  k!("ADD EXCLUDE", "ALTER TABLE ... ADD EXCLUDE USING <method> (<col> WITH <op>, ...) -- exclusion constraint.", "ALTER TABLE bookings ADD EXCLUDE USING gist (room WITH =, during WITH &&);", pg("sql-createtable.html"));


  // ---- round 192 SQL-standard spelling kws ----
  k!("BIT VARYING", "BIT VARYING(<n>) -- SQL-standard spelling of VARBIT.", "v BIT VARYING(8)", pg("datatype-bit.html"));
  k!("CHARACTER VARYING", "CHARACTER VARYING(<n>) -- SQL-standard spelling of VARCHAR.", "name CHARACTER VARYING(255)", pg("datatype-character.html"));
  k!("DOUBLE PRECISION", "DOUBLE PRECISION -- 8-byte IEEE float (alias FLOAT8).", "ratio DOUBLE PRECISION", pg("datatype-numeric.html"));
  k!("WITH TIME ZONE", "TIME / TIMESTAMP WITH TIME ZONE -- store + retrieve with zone awareness.", "ts TIMESTAMP WITH TIME ZONE", pg("datatype-datetime.html"));
  k!("WITHOUT TIME ZONE", "TIME / TIMESTAMP WITHOUT TIME ZONE -- store the raw value (default).", "ts TIMESTAMP WITHOUT TIME ZONE", pg("datatype-datetime.html"));
  k!("TIME WITH TIME ZONE", "Standard spelling of TIMETZ.", "open_at TIME WITH TIME ZONE", pg("datatype-datetime.html"));
  k!("TIME WITHOUT TIME ZONE", "Standard spelling of TIME.", "open_at TIME WITHOUT TIME ZONE", pg("datatype-datetime.html"));
  k!("TIMESTAMP WITH TIME ZONE", "Standard spelling of TIMESTAMPTZ.", "ts TIMESTAMP WITH TIME ZONE", pg("datatype-datetime.html"));
  k!("TIMESTAMP WITHOUT TIME ZONE", "Standard spelling of TIMESTAMP.", "ts TIMESTAMP WITHOUT TIME ZONE", pg("datatype-datetime.html"));
  k!("REFERENCES OLD", "Trigger REFERENCING OLD AS <alias> / OLD TABLE AS <alias>.", "REFERENCING OLD TABLE AS old_rows", pg("sql-createtrigger.html"));
  k!("REFERENCES NEW", "Trigger REFERENCING NEW AS <alias> / NEW TABLE AS <alias>.", "REFERENCING NEW TABLE AS new_rows", pg("sql-createtrigger.html"));
  k!("USING JOIN", "FROM ... [type] JOIN <other> USING (col[, ...]) -- common-column join.", "SELECT * FROM a JOIN b USING (id);", pg("queries-table-expressions.html"));
  k!("AS QUERY", "PREPARE / CREATE PROCEDURE / CREATE VIEW ... AS <query> -- introduces the prepared statement or body.", "PREPARE plan AS SELECT * FROM t;", pg("sql-prepare.html"));
  k!("DEFAULT EXPRESSION", "<col> <type> DEFAULT <expr> -- column default expression.", "id uuid DEFAULT gen_random_uuid()", pg("sql-createtable.html"));
  k!("STORED EXPRESSION", "<col> <type> GENERATED ALWAYS AS (<expr>) STORED -- materialised computed column.", "total numeric GENERATED ALWAYS AS (qty * price) STORED", pg("ddl-generated-columns.html"));
  k!("PRIMARY KEY", "PRIMARY KEY [(<cols>)] -- column/table constraint; implies NOT NULL + UNIQUE, builds a btree.", "id uuid PRIMARY KEY DEFAULT gen_random_uuid()", pg("ddl-constraints.html#DDL-CONSTRAINTS-PRIMARY-KEYS"));
  k!("FOREIGN KEY", "FOREIGN KEY (<col>[, ...]) REFERENCES <ref_table>[(<ref_col>[, ...])] [ON DELETE ...] [ON UPDATE ...] [MATCH ...].", "FOREIGN KEY (uid) REFERENCES users(id) ON DELETE CASCADE", pg("ddl-constraints.html#DDL-CONSTRAINTS-FK"));
  k!("EXCLUDE USING", "EXCLUDE USING <method> (<col> WITH <op>[, ...]) -- index-backed exclusion constraint.", "EXCLUDE USING gist (room WITH =, period WITH &&)", pg("ddl-constraints.html#DDL-CONSTRAINTS-EXCLUSION"));
  k!("UNIQUE NULLS NOT DISTINCT", "PG15+: UNIQUE NULLS NOT DISTINCT -- treat NULL == NULL for uniqueness check.", "CREATE UNIQUE INDEX i ON t (x) NULLS NOT DISTINCT", pg("sql-createtable.html#SQL-CREATETABLE-UNIQUE"));
  k!("AT TIME ZONE", "<timestamptz> AT TIME ZONE '<tz>' -- convert between zones (or rotate a timestamp into a zone).", "SELECT now() AT TIME ZONE 'UTC';", pg("functions-datetime.html#FUNCTIONS-DATETIME-ZONECONVERT"));
  k!("WITH ORDINALITY", "FROM <srf>(...) WITH ORDINALITY -- append a `ordinality` bigint column numbering output rows 1..N.", "SELECT * FROM unnest(ARRAY['a','b']) WITH ORDINALITY AS u(v, idx);", pg("queries-table-expressions.html#QUERIES-TABLEFUNCTIONS"));
  k!("AS MATERIALIZED", "WITH cte AS MATERIALIZED (<body>) ... -- force the CTE to materialise (default for recursive / multiple uses).", "WITH big AS MATERIALIZED (SELECT * FROM heavy_query) SELECT * FROM big WHERE active;", pg("queries-with.html#QUERIES-WITH-MATERIALIZATION"));
  k!("AS NOT MATERIALIZED", "WITH cte AS NOT MATERIALIZED (<body>) ... -- planner may inline the CTE body into the main query.", "WITH cheap AS NOT MATERIALIZED (SELECT id FROM t WHERE active) SELECT * FROM cheap;", pg("queries-with.html#QUERIES-WITH-MATERIALIZATION"));
  k!("FORCE NULL", "COPY ... WITH (FORCE_NULL (<col>[, ...])) -- treat the quoted form of NULL as NULL for these CSV columns.", "COPY t FROM '/tmp/d.csv' WITH (FORMAT csv, FORCE_NULL (notes));", pg("sql-copy.html"));

  m
}
