//! SQL built-in function table. Append new functions with the `f!` macro.

use crate::entry::{Entry, Kind, pg};
use std::collections::HashMap;

pub fn build() -> HashMap<&'static str, Entry> {
  let mut m = HashMap::new();
  macro_rules! f {
    ($label:expr, $sig:expr, $doc:expr, $example:expr, $url:expr) => {
      m.insert(
        $label,
        Entry { label: $label, kind: Kind::Function, doc: $doc, signature: Some($sig), example: $example, url: $url },
      );
    };
  }

  // Aggregates
  f!(
    "count",
    "count(* | expr) -> bigint",
    "Number of rows. count(*) counts every row; count(expr) counts non-null expr.",
    "SELECT count(*) FROM users;",
    pg("functions-aggregate.html")
  );
  f!(
    "sum",
    "sum(numeric) -> numeric",
    "Sum of values. NULL when no rows.",
    "SELECT sum(amount) FROM orders;",
    pg("functions-aggregate.html")
  );
  f!(
    "avg",
    "avg(numeric) -> numeric",
    "Arithmetic mean.",
    "SELECT avg(price) FROM products;",
    pg("functions-aggregate.html")
  );
  f!(
    "min",
    "min(any) -> any",
    "Minimum value across rows.",
    "SELECT min(created_at) FROM events;",
    pg("functions-aggregate.html")
  );
  f!(
    "max",
    "max(any) -> any",
    "Maximum value across rows.",
    "SELECT max(score) FROM games;",
    pg("functions-aggregate.html")
  );
  f!(
    "array_agg",
    "array_agg(any [ORDER BY ...]) -> array",
    "Aggregate inputs into an array.",
    "SELECT user_id, array_agg(role) FROM user_roles GROUP BY user_id;",
    pg("functions-aggregate.html")
  );
  f!(
    "string_agg",
    "string_agg(text, delim text [ORDER BY ...]) -> text",
    "Concatenate text values with a delimiter.",
    "SELECT string_agg(name, ', ') FROM users;",
    pg("functions-aggregate.html")
  );
  f!(
    "json_agg",
    "json_agg(any) -> json",
    "Aggregate rows into a JSON array.",
    "SELECT user_id, json_agg(orders.*) FROM orders GROUP BY user_id;",
    pg("functions-aggregate.html")
  );
  f!(
    "jsonb_agg",
    "jsonb_agg(any) -> jsonb",
    "Aggregate rows into a JSONB array.",
    "SELECT jsonb_agg(orders.*) FROM orders;",
    pg("functions-aggregate.html")
  );

  // Conditional
  f!(
    "coalesce",
    "coalesce(a, b, ...) -> any",
    "Return the first non-null argument.",
    "SELECT coalesce(nickname, name, 'anonymous') FROM users;",
    pg("functions-conditional.html")
  );
  f!(
    "nullif",
    "nullif(a, b) -> any",
    "Return NULL when a = b, else a.",
    "SELECT nullif(value, '') FROM t;",
    pg("functions-conditional.html")
  );
  f!(
    "greatest",
    "greatest(a, b, ...) -> any",
    "Largest non-null argument.",
    "SELECT greatest(a, b, c) FROM t;",
    pg("functions-conditional.html")
  );
  f!(
    "least",
    "least(a, b, ...) -> any",
    "Smallest non-null argument.",
    "SELECT least(a, b, c) FROM t;",
    pg("functions-conditional.html")
  );

  // String
  f!(
    "lower",
    "lower(text) -> text",
    "Convert to lowercase.",
    "WHERE lower(email) = 'a@x.io'",
    pg("functions-string.html")
  );
  f!(
    "upper",
    "upper(text) -> text",
    "Convert to uppercase.",
    "SELECT upper(country_code) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "length",
    "length(text) -> int",
    "Character count (bytes for non-UTF8 / bytea).",
    "WHERE length(name) > 0",
    pg("functions-string.html")
  );
  f!(
    "char_length",
    "char_length(text) -> int",
    "Character count -- counts characters, not bytes. Alias: `character_length`.",
    "SELECT char_length(name) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "character_length",
    "character_length(text) -> int",
    "Character count -- SQL-standard spelling, same as `char_length`.",
    "SELECT character_length(name) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "substring",
    "substring(text FROM start [FOR len]) -> text",
    "Slice a string. 1-based.",
    "SELECT substring(email FROM 1 FOR 3) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "trim",
    "trim([leading|trailing|both] [chars] FROM text) -> text",
    "Strip whitespace or chars from ends.",
    "SELECT trim('  hello  ');",
    pg("functions-string.html")
  );
  f!(
    "concat",
    "concat(any [, any ...]) -> text",
    "Concatenate. NULLs become empty (unlike ||).",
    "SELECT concat(first, ' ', last) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "replace",
    "replace(text, from text, to text) -> text",
    "Replace every occurrence.",
    "SELECT replace(name, '_', ' ') FROM t;",
    pg("functions-string.html")
  );
  f!(
    "split_part",
    "split_part(text, delim text, n int) -> text",
    "1-based nth field after splitting.",
    "SELECT split_part(email, '@', 2) FROM users;",
    pg("functions-string.html")
  );

  // Date / time
  f!(
    "now",
    "now() -> timestamptz",
    "Current statement timestamp.",
    "INSERT INTO events (at) VALUES (now());",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT")
  );
  f!(
    "current_date",
    "current_date -> date",
    "Today's date in the session time zone.",
    "WHERE birth_date = current_date",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT")
  );
  f!(
    "age",
    "age(end timestamptz, start timestamptz) -> interval",
    "Symbolic interval (years, months, days) between two timestamps.",
    "SELECT age(now(), birth_date) FROM users;",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT")
  );
  f!(
    "date_trunc",
    "date_trunc(field text, source) -> timestamptz",
    "Truncate to a precision (year, month, day, hour, week, ...).",
    "SELECT date_trunc('day', created_at), count(*) FROM events GROUP BY 1;",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-TRUNC")
  );
  f!(
    "extract",
    "extract(field FROM source) -> numeric",
    "SQL-standard form of date_part.",
    "SELECT extract(year FROM created_at) FROM users;",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT")
  );
  f!(
    "to_char",
    "to_char(value, format text) -> text",
    "Format a value as text via a template string.",
    "SELECT to_char(now(), 'YYYY-MM-DD HH24:MI');",
    pg("functions-formatting.html")
  );

  // Math
  f!(
    "abs",
    "abs(numeric) -> numeric",
    "Absolute value.",
    "SELECT abs(balance) FROM accounts;",
    pg("functions-math.html")
  );
  f!(
    "round",
    "round(numeric [, digits int]) -> numeric",
    "Round half away from zero.",
    "SELECT round(price, 2) FROM products;",
    pg("functions-math.html")
  );

  // UUID
  f!(
    "gen_random_uuid",
    "gen_random_uuid() -> uuid",
    "New UUIDv4. Available in Postgres 13+ without extension.",
    "id UUID PRIMARY KEY DEFAULT gen_random_uuid()",
    pg("functions-uuid.html")
  );

  // Window
  f!(
    "row_number",
    "row_number() OVER (...) -> bigint",
    "Sequential row number within the window partition.",
    "SELECT row_number() OVER (PARTITION BY user_id ORDER BY at DESC) FROM events;",
    pg("functions-window.html")
  );
  f!(
    "rank",
    "rank() OVER (...) -> bigint",
    "Ranking with gaps after ties.",
    "SELECT rank() OVER (ORDER BY score DESC) FROM games;",
    pg("functions-window.html")
  );
  f!(
    "dense_rank",
    "dense_rank() OVER (...) -> bigint",
    "Ranking without gaps after ties.",
    "SELECT dense_rank() OVER (ORDER BY score DESC) FROM games;",
    pg("functions-window.html")
  );
  f!(
    "lag",
    "lag(expr [, offset [, default]]) OVER (...) -> any",
    "Value of previous row within the window.",
    "SELECT lag(created_at) OVER (ORDER BY created_at) FROM events;",
    pg("functions-window.html")
  );
  f!(
    "lead",
    "lead(expr [, offset [, default]]) OVER (...) -> any",
    "Value of next row within the window.",
    "SELECT lead(created_at) OVER (ORDER BY created_at) FROM events;",
    pg("functions-window.html")
  );

  // JSON / array
  f!(
    "json_build_object",
    "json_build_object(k, v, ...) -> json",
    "Build a JSON object from alternating keys and values.",
    "SELECT json_build_object('id', id, 'name', name) FROM users;",
    pg("functions-json.html")
  );
  f!(
    "jsonb_build_object",
    "jsonb_build_object(k, v, ...) -> jsonb",
    "Build a JSONB object.",
    "SELECT jsonb_build_object('id', id, 'name', name) FROM users;",
    pg("functions-json.html")
  );
  f!(
    "unnest",
    "unnest(array) -> setof any",
    "Expand an array into a row set.",
    "SELECT id, unnest(tags) AS tag FROM posts;",
    pg("functions-array.html")
  );

  // -----------------------------------------------------------------------
  // Extended string functions (LEFT / RIGHT and friends).
  // -----------------------------------------------------------------------
  f!(
    "left",
    "left(text, n int) -> text",
    "First n characters of a string (negative n drops trailing chars).",
    "SELECT left(email, 5) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "right",
    "right(text, n int) -> text",
    "Last n characters of a string (negative n drops leading chars).",
    "SELECT right(phone, 4) FROM contacts;",
    pg("functions-string.html")
  );
  f!(
    "lpad",
    "lpad(text, len int [, fill text]) -> text",
    "Left-pad to length using fill (default space).",
    "SELECT lpad(id::text, 8, '0') FROM users;",
    pg("functions-string.html")
  );
  f!(
    "rpad",
    "rpad(text, len int [, fill text]) -> text",
    "Right-pad to length using fill (default space).",
    "SELECT rpad(name, 20) FROM users;",
    pg("functions-string.html")
  );
  f!(
    "btrim",
    "btrim(text [, chars]) -> text",
    "Trim chars (or whitespace) from both ends.",
    "SELECT btrim('  hi  ');",
    pg("functions-string.html")
  );
  f!(
    "ascii",
    "ascii(text) -> int",
    "Unicode code point of the first character.",
    "SELECT ascii('A');",
    pg("functions-string.html")
  );
  f!("chr", "chr(int) -> text", "Character for a Unicode code point.", "SELECT chr(65);", pg("functions-string.html"));
  f!(
    "repeat",
    "repeat(text, n int) -> text",
    "Repeat text n times.",
    "SELECT repeat('ab', 3);",
    pg("functions-string.html")
  );
  f!(
    "reverse",
    "reverse(text) -> text",
    "Reverse the characters of a string.",
    "SELECT reverse('abc');",
    pg("functions-string.html")
  );
  f!(
    "starts_with",
    "starts_with(string, prefix) -> boolean",
    "True if string starts with prefix.",
    "WHERE starts_with(email, 'admin@')",
    pg("functions-string.html")
  );
  f!(
    "octet_length",
    "octet_length(text | bytea) -> int",
    "Length in bytes (not characters).",
    "SELECT octet_length(body) FROM posts;",
    pg("functions-string.html")
  );
  f!(
    "bit_length",
    "bit_length(text | bytea) -> int",
    "Length in bits.",
    "SELECT bit_length(body) FROM posts;",
    pg("functions-string.html")
  );
  f!(
    "translate",
    "translate(string, from, to) -> text",
    "Character-by-character map.",
    "SELECT translate('1234', '13', 'ab');",
    pg("functions-string.html")
  );
  f!(
    "overlay",
    "overlay(text PLACING text FROM int [FOR int]) -> text",
    "Replace a substring at a position.",
    "SELECT overlay('abcdef' PLACING 'XY' FROM 3 FOR 2);",
    pg("functions-string.html")
  );
  f!(
    "format",
    "format(format text, args ...) -> text",
    "printf-style formatter for SQL. %s / %I / %L.",
    "SELECT format('Hello, %s!', name) FROM users;",
    pg("functions-string.html#FUNCTIONS-STRING-FORMAT")
  );
  f!(
    "regexp_match",
    "regexp_match(text, pattern [, flags]) -> text[]",
    "First match as array of capture groups.",
    "SELECT regexp_match(email, '^(.+)@(.+)$') FROM users;",
    pg("functions-matching.html")
  );
  f!(
    "regexp_matches",
    "regexp_matches(text, pattern [, flags]) -> setof text[]",
    "All matches as a set of arrays.",
    "SELECT regexp_matches(body, '\\w+', 'g') FROM posts;",
    pg("functions-matching.html")
  );
  f!(
    "regexp_replace",
    "regexp_replace(text, pattern, replacement [, flags]) -> text",
    "Regex substitute.",
    "SELECT regexp_replace(phone, '\\D', '', 'g') FROM contacts;",
    pg("functions-matching.html")
  );
  f!(
    "regexp_split_to_array",
    "regexp_split_to_array(text, pattern [, flags]) -> text[]",
    "Split by regex; result as array.",
    "SELECT regexp_split_to_array('a,b;c', '[,;]');",
    pg("functions-matching.html")
  );
  f!(
    "regexp_split_to_table",
    "regexp_split_to_table(text, pattern [, flags]) -> setof text",
    "Split by regex; result as rows.",
    "SELECT regexp_split_to_table('a,b;c', '[,;]');",
    pg("functions-matching.html")
  );

  // -----------------------------------------------------------------------
  // Numeric / math
  // -----------------------------------------------------------------------
  f!(
    "ceiling",
    "ceiling(numeric) -> numeric",
    "Alias for ceil. Smallest integer >= argument.",
    "SELECT ceiling(price / 100.0) FROM products;",
    pg("functions-math.html")
  );
  f!(
    "trunc",
    "trunc(numeric [, digits int]) -> numeric",
    "Truncate toward zero.",
    "SELECT trunc(price, 2) FROM products;",
    pg("functions-math.html")
  );
  f!(
    "sign",
    "sign(numeric) -> numeric",
    "-1, 0, or 1.",
    "SELECT sign(balance) FROM accounts;",
    pg("functions-math.html")
  );
  f!(
    "mod",
    "mod(a numeric, b numeric) -> numeric",
    "Remainder.",
    "SELECT mod(id, 10) FROM users;",
    pg("functions-math.html")
  );
  f!("exp", "exp(numeric) -> numeric", "e raised to argument.", "SELECT exp(1);", pg("functions-math.html"));
  f!("ln", "ln(numeric) -> numeric", "Natural log.", "SELECT ln(2.718);", pg("functions-math.html"));
  f!(
    "log",
    "log(numeric) | log(base, numeric)",
    "Base-10 log, or log to arbitrary base.",
    "SELECT log(1000);",
    pg("functions-math.html")
  );
  f!("pi", "pi() -> double", "3.14159...", "SELECT pi();", pg("functions-math.html"));
  f!(
    "degrees",
    "degrees(numeric) -> numeric",
    "Radians to degrees.",
    "SELECT degrees(pi());",
    pg("functions-math.html")
  );
  f!(
    "radians",
    "radians(numeric) -> numeric",
    "Degrees to radians.",
    "SELECT radians(180);",
    pg("functions-math.html")
  );
  f!(
    "setseed",
    "setseed(double) -> void",
    "Seed the RNG for reproducible random().",
    "SELECT setseed(0.5);",
    pg("functions-math.html")
  );

  // -----------------------------------------------------------------------
  // Date / time extras
  // -----------------------------------------------------------------------
  f!(
    "date_part",
    "date_part(field text, source) -> double",
    "Extract a numeric field. Older form of EXTRACT.",
    "SELECT date_part('year', created_at) FROM users;",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT")
  );
  f!(
    "to_date",
    "to_date(text, format text) -> date",
    "Parse a string into a date.",
    "SELECT to_date('2025-01-15', 'YYYY-MM-DD');",
    pg("functions-formatting.html")
  );
  f!(
    "to_timestamp",
    "to_timestamp(text, format text) -> timestamptz",
    "Parse a string into a timestamptz.",
    "SELECT to_timestamp('2025-01-15 14:00', 'YYYY-MM-DD HH24:MI');",
    pg("functions-formatting.html")
  );
  f!(
    "to_number",
    "to_number(text, format text) -> numeric",
    "Parse a string into a numeric.",
    "SELECT to_number('12,345', '99,999');",
    pg("functions-formatting.html")
  );
  f!(
    "make_date",
    "make_date(year, month, day) -> date",
    "Build a date from components.",
    "SELECT make_date(2026, 1, 15);",
    pg("functions-datetime.html")
  );
  f!(
    "make_time",
    "make_time(hour, min, sec double) -> time",
    "Build a time from components.",
    "SELECT make_time(14, 0, 0);",
    pg("functions-datetime.html")
  );
  f!(
    "make_timestamptz",
    "make_timestamptz(y, m, d, h, mi, s, [tz]) -> timestamptz",
    "Build a timestamptz from components.",
    "SELECT make_timestamptz(2026,1,15,14,0,0);",
    pg("functions-datetime.html")
  );
  f!(
    "justify_interval",
    "justify_interval(interval) -> interval",
    "Normalise interval (carry days into months).",
    "SELECT justify_interval(interval '40 days');",
    pg("functions-datetime.html")
  );
  f!(
    "statement_timestamp",
    "statement_timestamp() -> timestamptz",
    "Timestamp at the start of the current statement (close to now()).",
    "SELECT statement_timestamp();",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT")
  );
  f!(
    "transaction_timestamp",
    "transaction_timestamp() -> timestamptz",
    "Timestamp at the start of the current transaction.",
    "SELECT transaction_timestamp();",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT")
  );
  f!(
    "clock_timestamp",
    "clock_timestamp() -> timestamptz",
    "Current wall clock; changes during a transaction.",
    "SELECT clock_timestamp();",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT")
  );

  // -----------------------------------------------------------------------
  // Set-returning helpers
  // -----------------------------------------------------------------------
  f!(
    "generate_series",
    "generate_series(start, stop [, step]) -> setof <T>",
    "Produce a sequence of integers, dates, or timestamps.",
    "SELECT * FROM generate_series(1, 10);",
    pg("functions-srf.html")
  );
  f!(
    "generate_subscripts",
    "generate_subscripts(array, dim) -> setof int",
    "Indices of an array along a dimension.",
    "SELECT i, arr[i] FROM data, generate_subscripts(arr, 1) AS s(i);",
    pg("functions-srf.html")
  );

  // -----------------------------------------------------------------------
  // Encoding / hashing / UUID variants
  // -----------------------------------------------------------------------
  f!(
    "encode",
    "encode(bytea, format text) -> text",
    "Encode bytea as base64 / hex / escape.",
    "SELECT encode(sha256('hi'), 'hex');",
    pg("functions-binarystring.html")
  );
  f!(
    "decode",
    "decode(text, format text) -> bytea",
    "Decode text as base64 / hex / escape.",
    "SELECT decode('48656c6c6f', 'hex');",
    pg("functions-binarystring.html")
  );
  f!(
    "digest",
    "digest(text|bytea, algo text) -> bytea",
    "Hash with pgcrypto. md5 / sha1 / sha224 / sha256 / sha384 / sha512.",
    "SELECT encode(digest('hi', 'sha256'), 'hex');",
    pg("pgcrypto.html")
  );
  f!(
    "sha1",
    "sha1(bytea) -> bytea",
    "SHA-1 binary (pgcrypto).",
    "SELECT encode(sha1('hi'), 'hex');",
    pg("pgcrypto.html")
  );
  f!(
    "sha512",
    "sha512(bytea) -> bytea",
    "SHA-512 binary (pgcrypto).",
    "SELECT encode(sha512('hi'), 'hex');",
    pg("pgcrypto.html")
  );

  // -----------------------------------------------------------------------
  // JSON / JSONB extras
  // -----------------------------------------------------------------------
  f!(
    "json_object",
    "json_object(...) -> json",
    "Build a JSON object from a list of key/value pairs or an array.",
    "SELECT json_object('id', id, 'n', n) FROM t;",
    pg("functions-json.html")
  );
  f!(
    "jsonb_object",
    "jsonb_object(...) -> jsonb",
    "JSONB version of json_object.",
    "SELECT jsonb_object('id', id) FROM t;",
    pg("functions-json.html")
  );
  f!(
    "jsonb_set",
    "jsonb_set(target jsonb, path text[], new_value jsonb [, create_missing bool]) -> jsonb",
    "Replace or add a value at a JSON path.",
    "UPDATE users SET meta = jsonb_set(meta, '{role}', '\"admin\"');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_path_query",
    "jsonb_path_query(jsonb, jsonpath) -> setof jsonb",
    "Run a JSONPath query.",
    "SELECT jsonb_path_query(data, '$.items[*].price') FROM orders;",
    pg("functions-json.html")
  );
  f!(
    "json_array_length",
    "json_array_length(json) -> int",
    "Number of elements in a JSON array.",
    "SELECT json_array_length(tags) FROM posts;",
    pg("functions-json.html")
  );
  f!(
    "jsonb_array_length",
    "jsonb_array_length(jsonb) -> int",
    "Number of elements in a JSONB array.",
    "SELECT jsonb_array_length(tags) FROM posts;",
    pg("functions-json.html")
  );

  // -----------------------------------------------------------------------
  // Array helpers
  // -----------------------------------------------------------------------
  f!(
    "array_length",
    "array_length(arr, dim int) -> int",
    "Length along a dimension.",
    "SELECT array_length(roles, 1) FROM users;",
    pg("functions-array.html")
  );
  f!(
    "array_position",
    "array_position(arr, elem) -> int",
    "1-based index of first match.",
    "SELECT array_position(roles, 'admin');",
    pg("functions-array.html")
  );
  f!(
    "array_append",
    "array_append(arr, elem) -> array",
    "Append element; `||` is shorter.",
    "UPDATE users SET roles = array_append(roles, 'admin') WHERE id = $1;",
    pg("functions-array.html")
  );
  f!(
    "array_remove",
    "array_remove(arr, elem) -> array",
    "Drop every occurrence of element.",
    "SELECT array_remove(roles, 'banned') FROM users;",
    pg("functions-array.html")
  );
  f!(
    "array_to_string",
    "array_to_string(arr, sep text [, null_str]) -> text",
    "Join array elements with separator.",
    "SELECT array_to_string(tags, ', ') FROM posts;",
    pg("functions-array.html")
  );
  f!(
    "string_to_array",
    "string_to_array(text, delim [, null_str]) -> text[]",
    "Split a string into an array.",
    "SELECT string_to_array('a,b,c', ',');",
    pg("functions-array.html")
  );
  f!(
    "cardinality",
    "cardinality(array) -> int",
    "Total number of elements (across all dimensions).",
    "SELECT cardinality(matrix) FROM data;",
    pg("functions-array.html")
  );

  // -----------------------------------------------------------------------
  // Aggregate extras
  // -----------------------------------------------------------------------
  f!(
    "bool_and",
    "bool_and(boolean) -> boolean",
    "True when every row is true.",
    "SELECT bool_and(active) FROM users;",
    pg("functions-aggregate.html")
  );
  f!(
    "bool_or",
    "bool_or(boolean) -> boolean",
    "True when any row is true.",
    "SELECT bool_or(active) FROM users;",
    pg("functions-aggregate.html")
  );
  f!(
    "every",
    "every(boolean) -> boolean",
    "SQL-standard alias of bool_and.",
    "SELECT every(active) FROM users;",
    pg("functions-aggregate.html")
  );
  f!(
    "mode",
    "mode() WITHIN GROUP (ORDER BY ...) -> any",
    "Ordered-set aggregate: the most-frequent value of the sort expression.",
    "SELECT mode() WITHIN GROUP (ORDER BY status) FROM orders;",
    pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE")
  );
  f!(
    "percentile_cont",
    "percentile_cont(fraction) WITHIN GROUP (ORDER BY ...) -> numeric",
    "Continuous percentile. p50 -> median.",
    "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY age) FROM users;",
    pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE")
  );
  f!(
    "percentile_disc",
    "percentile_disc(fraction) WITHIN GROUP (ORDER BY ...) -> any",
    "Discrete percentile.",
    "SELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY age) FROM users;",
    pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE")
  );
  f!(
    "stddev",
    "stddev(numeric) -> numeric",
    "Sample standard deviation.",
    "SELECT stddev(price) FROM products;",
    pg("functions-aggregate.html")
  );
  f!(
    "variance",
    "variance(numeric) -> numeric",
    "Sample variance.",
    "SELECT variance(price) FROM products;",
    pg("functions-aggregate.html")
  );

  // -----------------------------------------------------------------------
  // Type-system / introspection helpers commonly seen in queries
  // -----------------------------------------------------------------------
  f!(
    "pg_typeof",
    "pg_typeof(any) -> regtype",
    "Get the type of an expression.",
    "SELECT pg_typeof(now());",
    pg("functions-info.html")
  );
  f!(
    "format_type",
    "format_type(type_oid, typemod) -> text",
    "SQL-name of a type given its OID and typmod. Useful for catalog introspection.",
    "SELECT format_type(atttypid, atttypmod) FROM pg_attribute WHERE attname = 'id';",
    pg("functions-info.html")
  );
  f!(
    "obj_description",
    "obj_description(object_oid [, catalog_name text]) -> text",
    "Comment attached to a database object (table, function, ...).",
    "SELECT obj_description('users'::regclass);",
    pg("functions-info.html#FUNCTIONS-INFO-COMMENT")
  );
  f!(
    "col_description",
    "col_description(table_oid, column_number) -> text",
    "Comment on a specific column.",
    "SELECT col_description('users'::regclass, 1);",
    pg("functions-info.html#FUNCTIONS-INFO-COMMENT")
  );
  f!(
    "shobj_description",
    "shobj_description(object_oid, shared_catalog_name text) -> text",
    "Comment on a shared (cluster-wide) object such as a role or database.",
    "SELECT shobj_description(d.oid, 'pg_database') FROM pg_database d;",
    pg("functions-info.html#FUNCTIONS-INFO-COMMENT")
  );
  f!(
    "bit_count",
    "bit_count(bytea | bit) -> bigint",
    "Population count -- number of 1-bits in the argument (PG 14+).",
    "SELECT bit_count(B'10110100');",
    pg("functions-bitstring.html")
  );
  f!(
    "gen_random_bytes",
    "gen_random_bytes(n int) -> bytea (pgcrypto)",
    "Cryptographically strong random bytes. Requires the pgcrypto extension.",
    "SELECT gen_random_bytes(16);",
    pg("pgcrypto.html")
  );
  f!(
    "crypt",
    "crypt(password text, salt text) -> text (pgcrypto)",
    "One-way password hash. Pair with `gen_salt('bf')` to verify with `crypt(password, stored_hash) = stored_hash`.",
    "SELECT crypt('pwd', gen_salt('bf'));",
    pg("pgcrypto.html")
  );
  f!(
    "gen_salt",
    "gen_salt(algorithm text [, iter int]) -> text (pgcrypto)",
    "Generate a salt for crypt(). Algorithms: bf, md5, xdes, des.",
    "SELECT gen_salt('bf');",
    pg("pgcrypto.html")
  );
  f!(
    "hmac",
    "hmac(data text|bytea, key text|bytea, type text) -> bytea (pgcrypto)",
    "Keyed-hash MAC. Algorithms: md5, sha1, sha224, sha256, sha384, sha512.",
    "SELECT hmac('msg', 'key', 'sha256');",
    pg("pgcrypto.html")
  );
  f!(
    "to_hex",
    "to_hex(int | bigint) -> text",
    "Hex-string representation of an integer.",
    "SELECT to_hex(255);  -- 'ff'",
    pg("functions-string.html#FUNCTIONS-STRING-OTHER")
  );
  f!(
    "normalize",
    "normalize(text [, form NFC|NFD|NFKC|NFKD]) -> text",
    "Unicode normalization (PG 13+). Default form is NFC.",
    "SELECT normalize('café', NFC);",
    pg("functions-string.html#FUNCTIONS-STRING-OTHER")
  );
  f!(
    "current_setting",
    "current_setting(name text [, missing_ok bool]) -> text",
    "Read a run-time parameter.",
    "SELECT current_setting('search_path');",
    pg("functions-admin.html")
  );
  f!(
    "set_config",
    "set_config(name text, new_value text, is_local bool) -> text",
    "Write a run-time parameter.",
    "SELECT set_config('search_path', 'app, public', false);",
    pg("functions-admin.html")
  );
  f!(
    "current_user",
    "current_user -> name",
    "Current SQL session user.",
    "SELECT current_user;",
    pg("functions-info.html")
  );
  f!(
    "session_user",
    "session_user -> name",
    "Original session user (before SET ROLE).",
    "SELECT session_user;",
    pg("functions-info.html")
  );
  f!(
    "current_database",
    "current_database() -> name",
    "Active database name.",
    "SELECT current_database();",
    pg("functions-info.html")
  );
  f!(
    "current_schema",
    "current_schema() -> name",
    "First entry in search_path.",
    "SELECT current_schema();",
    pg("functions-info.html")
  );
  f!("version", "version() -> text", "Postgres server version banner.", "SELECT version();", pg("functions-info.html"));

  // --- Math (more) ------------------------------------------------------
  f!(
    "floor",
    "floor(numeric) -> numeric",
    "Largest integer <= argument.",
    "SELECT floor(3.7);",
    pg("functions-math.html")
  );
  f!(
    "sqrt",
    "sqrt(double precision) -> double precision",
    "Square root.",
    "SELECT sqrt(144);",
    pg("functions-math.html")
  );
  f!(
    "power",
    "power(a numeric, b numeric) -> numeric",
    "a raised to the b.",
    "SELECT power(2, 10);",
    pg("functions-math.html")
  );
  f!(
    "random",
    "random() -> double precision",
    "Uniform [0,1). Reseed with setseed.",
    "SELECT random() * 100;",
    pg("functions-math.html")
  );
  f!(
    "width_bucket",
    "width_bucket(value, lo, hi, count) -> int",
    "Histogram bucket index in [0, count+1].",
    "SELECT width_bucket(score, 0, 100, 10);",
    pg("functions-math.html")
  );
  f!(
    "md5",
    "md5(text) -> text",
    "MD5 hex digest. Use only for checksumming -- not cryptography.",
    "SELECT md5('hello');",
    pg("functions-binarystring.html")
  );

  // --- String (more) ---------------------------------------------------
  f!(
    "position",
    "position(substring IN string) -> int",
    "1-based index of substring; 0 when not found.",
    "SELECT position('lo' IN 'hello');",
    pg("functions-string.html")
  );
  f!(
    "strpos",
    "strpos(text, substring) -> int",
    "Same as position() with comma syntax.",
    "SELECT strpos('hello', 'lo');",
    pg("functions-string.html")
  );
  f!(
    "regexp_count",
    "regexp_count(text, pattern[, start[, flags]]) -> int",
    "Number of matches of the pattern in the string (PG 15+).",
    "SELECT regexp_count('aaaa', 'a');",
    pg("functions-matching.html")
  );
  f!(
    "regexp_substr",
    "regexp_substr(text, pattern[, start[, n[, flags]]]) -> text",
    "n-th match of pattern (PG 15+).",
    "SELECT regexp_substr('foo123bar456', '\\d+', 1, 2);",
    pg("functions-matching.html")
  );

  // --- JSON / JSONB (more) ---------------------------------------------
  f!(
    "to_jsonb",
    "to_jsonb(anyelement) -> jsonb",
    "Convert a SQL value to its jsonb representation.",
    "SELECT to_jsonb(ROW(1,'hi'));",
    pg("functions-json.html")
  );
  f!(
    "to_json",
    "to_json(anyelement) -> json",
    "Same as to_jsonb but produces json (not binary).",
    "SELECT to_json(ARRAY[1,2,3]);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_pretty",
    "jsonb_pretty(jsonb) -> text",
    "Pretty-printed JSON with indentation.",
    "SELECT jsonb_pretty('{\"a\":1}'::jsonb);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_typeof",
    "jsonb_typeof(jsonb) -> text",
    "object / array / string / number / boolean / null.",
    "SELECT jsonb_typeof('[1,2]'::jsonb);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_strip_nulls",
    "jsonb_strip_nulls(jsonb) -> jsonb",
    "Recursively drop keys with null values.",
    "SELECT jsonb_strip_nulls('{\"a\":1,\"b\":null}'::jsonb);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_object_keys",
    "jsonb_object_keys(jsonb) -> setof text",
    "Top-level keys as a set.",
    "SELECT * FROM jsonb_object_keys('{\"a\":1,\"b\":2}'::jsonb);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_each",
    "jsonb_each(jsonb) -> setof (key text, value jsonb)",
    "Iterate top-level pairs.",
    "SELECT k, v FROM jsonb_each('{\"a\":1}'::jsonb) AS (k text, v jsonb);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_array_elements",
    "jsonb_array_elements(jsonb) -> setof jsonb",
    "Expand a jsonb array to a set of jsonb values.",
    "SELECT * FROM jsonb_array_elements('[1,2,3]'::jsonb);",
    pg("functions-json.html")
  );

  // --- Window (more) ----------------------------------------------------
  f!(
    "ntile",
    "ntile(n int) -> int",
    "Bucket rows into n approximately equal groups.",
    "SELECT ntile(4) OVER (ORDER BY score) FROM scores;",
    pg("functions-window.html")
  );
  f!(
    "first_value",
    "first_value(value) -> same",
    "Value from the first row of the window frame.",
    "SELECT first_value(amount) OVER (PARTITION BY user_id ORDER BY ts);",
    pg("functions-window.html")
  );
  f!(
    "last_value",
    "last_value(value) -> same",
    "Value from the last row of the window frame. Watch the default frame -- usually wants UNBOUNDED FOLLOWING.",
    "SELECT last_value(amount) OVER (PARTITION BY user_id ORDER BY ts \
        ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING);",
    pg("functions-window.html")
  );
  f!(
    "nth_value",
    "nth_value(value, n int) -> same",
    "n-th row's value in the frame (1-based). Returns NULL if frame is shorter than n.",
    "SELECT nth_value(amount, 2) OVER w FROM orders WINDOW w AS (...);",
    pg("functions-window.html")
  );

  // --- Array (more) -----------------------------------------------------
  f!(
    "array_replace",
    "array_replace(array, old, new) -> array",
    "Replace every element equal to old.",
    "SELECT array_replace(ARRAY[1,2,2,3], 2, 9);",
    pg("functions-array.html")
  );
  f!(
    "array_cat",
    "array_cat(a array, b array) -> array",
    "Concatenate two arrays. The `||` operator does the same.",
    "SELECT array_cat(ARRAY[1,2], ARRAY[3]);",
    pg("functions-array.html")
  );
  f!(
    "array_dims",
    "array_dims(array) -> text",
    "Bounds of each dimension.",
    "SELECT array_dims(ARRAY[[1,2],[3,4]]);",
    pg("functions-array.html")
  );

  // --- Range -----------------------------------------------------------
  f!(
    "range_agg",
    "range_agg(anyrange) -> anymultirange",
    "Aggregate ranges into a multirange (PG 14+).",
    "SELECT range_agg(span) FROM bookings;",
    pg("functions-range.html")
  );
  f!(
    "multirange",
    "multirange(VARIADIC anyrange) -> anymultirange",
    "Build a multirange constant from explicit ranges.",
    "SELECT multirange(int4range(1,4), int4range(10,12));",
    pg("functions-range.html")
  );
  f!(
    "range_merge",
    "range_merge(anyrange, anyrange) -> anyrange",
    "Smallest range that contains both inputs (may include the gap between them).",
    "SELECT range_merge(int4range(1,5), int4range(10,20));",
    pg("functions-range.html")
  );
  f!(
    "int4multirange",
    "int4multirange(VARIADIC int4range) -> int4multirange",
    "Constructor for int4multirange from explicit int4range pieces (PG 14+).",
    "SELECT int4multirange(int4range(1,4), int4range(10,12));",
    pg("rangetypes.html#RANGETYPES-BUILTIN")
  );
  f!(
    "isempty",
    "isempty(anyrange) -> boolean",
    "True when the range or multirange contains no points.",
    "SELECT isempty(int4range(1,1));",
    pg("functions-range.html")
  );
  f!(
    "lower_inc",
    "lower_inc(anyrange) -> boolean",
    "True when the lower bound is inclusive.",
    "SELECT lower_inc(int4range(1,4));",
    pg("functions-range.html")
  );
  f!(
    "upper_inc",
    "upper_inc(anyrange) -> boolean",
    "True when the upper bound is inclusive.",
    "SELECT upper_inc(int4range(1,4));",
    pg("functions-range.html")
  );

  // --- Network (inet / cidr) ------------------------------------------
  f!(
    "host",
    "host(inet) -> text",
    "Address without the mask.",
    "SELECT host('192.168.0.1/24'::inet);",
    pg("functions-net.html")
  );
  f!(
    "network",
    "network(inet) -> cidr",
    "Network containing the address.",
    "SELECT network('192.168.0.1/24'::inet);",
    pg("functions-net.html")
  );
  f!(
    "netmask",
    "netmask(inet) -> inet",
    "Netmask for the address.",
    "SELECT netmask('192.168.0.1/24'::inet);",
    pg("functions-net.html")
  );
  f!(
    "set_masklen",
    "set_masklen(inet, int) -> inet",
    "Change the netmask length, keeping the address.",
    "SELECT set_masklen('192.168.0.1/32'::inet, 24);",
    pg("functions-net.html")
  );

  // --- System / utility -----------------------------------------------
  f!(
    "nextval",
    "nextval(regclass) -> bigint",
    "Advance a sequence and return the new value.",
    "SELECT nextval('users_id_seq');",
    pg("functions-sequence.html")
  );
  f!(
    "currval",
    "currval(regclass) -> bigint",
    "Last sequence value generated in THIS session.",
    "SELECT currval('users_id_seq');",
    pg("functions-sequence.html")
  );
  f!(
    "setval",
    "setval(regclass, bigint [, is_called bool]) -> bigint",
    "Force the sequence to a specific value.",
    "SELECT setval('users_id_seq', 1000);",
    pg("functions-sequence.html")
  );
  f!(
    "pg_sleep",
    "pg_sleep(double precision) -> void",
    "Block for N seconds. Useful in tests; never in hot paths.",
    "SELECT pg_sleep(0.5);",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-DELAY")
  );
  f!(
    "pg_sleep_for",
    "pg_sleep_for(interval) -> void",
    "Block for the given interval (more readable than `pg_sleep(seconds)`).",
    "SELECT pg_sleep_for('5 seconds');",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-DELAY")
  );
  f!(
    "pg_sleep_until",
    "pg_sleep_until(timestamp with time zone) -> void",
    "Block until the given wall-clock timestamp.",
    "SELECT pg_sleep_until(now() + interval '1 minute');",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-DELAY")
  );
  f!(
    "pg_size_pretty",
    "pg_size_pretty(bigint) -> text",
    "Human-readable size (kB, MB, GB).",
    "SELECT pg_size_pretty(pg_total_relation_size('users'));",
    pg("functions-admin.html#FUNCTIONS-ADMIN-DBOBJECT")
  );
  f!(
    "pg_get_functiondef",
    "pg_get_functiondef(regprocedure) -> text",
    "Round-trippable CREATE FUNCTION text.",
    "SELECT pg_get_functiondef('public.my_fn()'::regprocedure);",
    pg("functions-info.html")
  );
  f!(
    "pg_get_indexdef",
    "pg_get_indexdef(regclass) -> text",
    "CREATE INDEX text for an index.",
    "SELECT pg_get_indexdef('users_email_idx'::regclass);",
    pg("functions-info.html")
  );
  f!(
    "pg_advisory_lock",
    "pg_advisory_lock(bigint) -> void",
    "Acquire a session-level advisory lock (cooperative, not enforced).",
    "SELECT pg_advisory_lock(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_try_advisory_lock",
    "pg_try_advisory_lock(bigint) -> boolean",
    "Non-blocking advisory lock attempt.",
    "SELECT pg_try_advisory_lock(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_advisory_unlock",
    "pg_advisory_unlock(bigint) -> boolean",
    "Release a session-level advisory lock.",
    "SELECT pg_advisory_unlock(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_advisory_unlock_all",
    "pg_advisory_unlock_all() -> void",
    "Release all session-level advisory locks held by the session.",
    "SELECT pg_advisory_unlock_all();",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_advisory_xact_lock",
    "pg_advisory_xact_lock(bigint) -> void",
    "Transaction-scoped advisory lock; released at COMMIT/ROLLBACK.",
    "SELECT pg_advisory_xact_lock(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_try_advisory_xact_lock",
    "pg_try_advisory_xact_lock(bigint) -> boolean",
    "Non-blocking transaction-scoped advisory lock attempt.",
    "SELECT pg_try_advisory_xact_lock(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_advisory_lock_shared",
    "pg_advisory_lock_shared(bigint) -> void",
    "Acquire a session-level shared advisory lock.",
    "SELECT pg_advisory_lock_shared(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_try_advisory_lock_shared",
    "pg_try_advisory_lock_shared(bigint) -> boolean",
    "Non-blocking session-level shared advisory lock attempt.",
    "SELECT pg_try_advisory_lock_shared(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "pg_advisory_xact_lock_shared",
    "pg_advisory_xact_lock_shared(bigint) -> void",
    "Transaction-scoped shared advisory lock.",
    "SELECT pg_advisory_xact_lock_shared(42);",
    pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS")
  );
  f!(
    "hashtext",
    "hashtext(text) -> integer",
    "Hash a text value into an integer (useful for advisory-lock keys).",
    "SELECT pg_advisory_lock(hashtext('job-x'));",
    pg("functions-string.html")
  );
  f!(
    "hashtextextended",
    "hashtextextended(text, bigint) -> bigint",
    "Extended (64-bit) hash of a text value with a seed.",
    "SELECT hashtextextended('k', 0);",
    pg("functions-string.html")
  );
  f!(
    "hashbpchar",
    "hashbpchar(character) -> integer",
    "Hash function for `character`/`bpchar`.",
    "SELECT hashbpchar('abc'::char(3));",
    pg("functions-string.html")
  );
  f!(
    "make_interval",
    "make_interval(years int, months int, weeks int, days int, hours int, mins int, secs double) -> interval",
    "Build an interval from explicit fields. All args have defaults; use named-arg syntax.",
    "SELECT make_interval(days := 7, hours := 12);",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CONSTRUCT")
  );
  f!(
    "make_time",
    "make_time(hour int, min int, sec double) -> time",
    "Build a TIME value from explicit hour/minute/second.",
    "SELECT make_time(7, 30, 15);",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CONSTRUCT")
  );
  f!(
    "make_timestamp",
    "make_timestamp(y int, m int, d int, h int, mi int, sec double) -> timestamp",
    "Build a TIMESTAMP from y/m/d/h/m/s.",
    "SELECT make_timestamp(2026, 5, 26, 9, 0, 0);",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CONSTRUCT")
  );
  f!(
    "make_timestamptz",
    "make_timestamptz(y int, m int, d int, h int, mi int, sec double, tz text) -> timestamptz",
    "Build a TIMESTAMP WITH TIME ZONE.",
    "SELECT make_timestamptz(2026, 5, 26, 9, 0, 0, 'UTC');",
    pg("functions-datetime.html#FUNCTIONS-DATETIME-CONSTRUCT")
  );
  // String helpers commonly used in dynamic SQL.
  f!(
    "quote_literal",
    "quote_literal(text) -> text",
    "Quote a literal so it's a safe SQL string literal.",
    "SELECT quote_literal($$it's$$);",
    pg("functions-string.html")
  );
  f!(
    "quote_ident",
    "quote_ident(text) -> text",
    "Quote an identifier so it parses as a single name.",
    "SELECT quote_ident('weird name');",
    pg("functions-string.html")
  );
  f!(
    "quote_nullable",
    "quote_nullable(anyelement) -> text",
    "Like quote_literal but renders NULL as the unquoted token.",
    "SELECT quote_nullable(NULL);",
    pg("functions-string.html")
  );
  f!(
    "translate",
    "translate(text, from text, to text) -> text",
    "Per-character substitution.",
    "SELECT translate('hello','el','EL');",
    pg("functions-string.html")
  );
  f!(
    "repeat",
    "repeat(text, n int) -> text",
    "Repeat the input n times.",
    "SELECT repeat('ab', 3);",
    pg("functions-string.html")
  );
  f!(
    "reverse",
    "reverse(text) -> text",
    "Reverse the characters.",
    "SELECT reverse('hello');",
    pg("functions-string.html")
  );
  f!(
    "replace",
    "replace(text, from text, to text) -> text",
    "Replace every occurrence.",
    "SELECT replace('a b a','a','x');",
    pg("functions-string.html")
  );
  f!(
    "split_part",
    "split_part(text, sep text, n int) -> text",
    "Return the n-th field after splitting on sep.",
    "SELECT split_part('a,b,c', ',', 2);",
    pg("functions-string.html")
  );
  f!(
    "strpos",
    "strpos(haystack text, needle text) -> integer",
    "1-based index of needle in haystack, 0 if not found.",
    "SELECT strpos('hello world','world');",
    pg("functions-string.html")
  );
  f!(
    "btrim",
    "btrim(text, chars text) -> text",
    "Trim chars from both ends (default whitespace).",
    "SELECT btrim('  hi  ');",
    pg("functions-string.html")
  );
  f!(
    "ltrim",
    "ltrim(text, chars text) -> text",
    "Trim chars from the left.",
    "SELECT ltrim('xxxhi','x');",
    pg("functions-string.html")
  );
  f!(
    "rtrim",
    "rtrim(text, chars text) -> text",
    "Trim chars from the right.",
    "SELECT rtrim('hiyyy','y');",
    pg("functions-string.html")
  );
  f!(
    "initcap",
    "initcap(text) -> text",
    "Capitalize the first letter of each word.",
    "SELECT initcap('hello WORLD');",
    pg("functions-string.html")
  );
  f!(
    "octet_length",
    "octet_length(text|bytea) -> integer",
    "Length in bytes.",
    "SELECT octet_length('héllo');",
    pg("functions-string.html")
  );
  f!(
    "bit_length",
    "bit_length(text|bytea|bit) -> integer",
    "Length in bits.",
    "SELECT bit_length('a');",
    pg("functions-string.html")
  );
  f!(
    "ord",
    "ord(text) -> integer",
    "Codepoint of the first character.",
    "SELECT ord('A');",
    pg("functions-string.html")
  );
  // Bit / byte ops on bytea/bit.
  f!(
    "get_bit",
    "get_bit(bytea|bit, n int) -> integer",
    "Extract the n-th bit.",
    "SELECT get_bit(B'10101010', 3);",
    pg("functions-binarystring.html")
  );
  f!(
    "set_bit",
    "set_bit(bytea|bit, n int, v int) -> same",
    "Set the n-th bit to v.",
    "SELECT set_bit(B'10000000', 3, 1);",
    pg("functions-binarystring.html")
  );
  f!(
    "get_byte",
    "get_byte(bytea, n int) -> integer",
    "Extract the n-th byte.",
    "SELECT get_byte('\\xDEADBEEF'::bytea, 1);",
    pg("functions-binarystring.html")
  );
  f!(
    "set_byte",
    "set_byte(bytea, n int, v int) -> bytea",
    "Set the n-th byte to v.",
    "SELECT set_byte('\\xDEAD'::bytea, 0, 255);",
    pg("functions-binarystring.html")
  );
  // Conversion / encoding helpers.
  f!(
    "convert_from",
    "convert_from(bytea, src_encoding text) -> text",
    "Decode bytes from a specific encoding to TEXT.",
    "SELECT convert_from(E'\\\\xC3\\\\xA9'::bytea, 'UTF8');",
    pg("functions-string.html")
  );
  f!(
    "convert_to",
    "convert_to(text, dest_encoding text) -> bytea",
    "Encode TEXT into bytes in the requested encoding.",
    "SELECT convert_to('hi', 'UTF8');",
    pg("functions-string.html")
  );
  f!(
    "convert",
    "convert(bytea, src text, dest text) -> bytea",
    "Recode bytes between encodings.",
    "SELECT convert('hi'::bytea, 'LATIN1', 'UTF8');",
    pg("functions-string.html")
  );
  // pg_catalog admin helpers.
  f!(
    "pg_relation_filepath",
    "pg_relation_filepath(regclass) -> text",
    "Path (relative to data directory) of the relation's main file.",
    "SELECT pg_relation_filepath('users');",
    pg("functions-admin.html")
  );
  f!(
    "pg_get_viewdef",
    "pg_get_viewdef(regclass) -> text",
    "SELECT statement that defines a view.",
    "SELECT pg_get_viewdef('my_view'::regclass);",
    pg("functions-info.html")
  );
  f!(
    "pg_get_function_arguments",
    "pg_get_function_arguments(oid) -> text",
    "Argument signature of a function.",
    "SELECT pg_get_function_arguments(p.oid) FROM pg_proc p;",
    pg("functions-info.html")
  );
  f!(
    "pg_get_function_result",
    "pg_get_function_result(oid) -> text",
    "Return-type signature of a function.",
    "SELECT pg_get_function_result(p.oid) FROM pg_proc p;",
    pg("functions-info.html")
  );
  f!(
    "pg_get_functiondef",
    "pg_get_functiondef(oid) -> text",
    "Full CREATE FUNCTION text.",
    "SELECT pg_get_functiondef('public.fn'::regproc);",
    pg("functions-info.html")
  );
  f!(
    "pg_get_triggerdef",
    "pg_get_triggerdef(oid) -> text",
    "CREATE TRIGGER text.",
    "SELECT pg_get_triggerdef(t.oid) FROM pg_trigger t LIMIT 1;",
    pg("functions-info.html")
  );
  f!(
    "pg_get_constraintdef",
    "pg_get_constraintdef(oid) -> text",
    "Constraint definition text.",
    "SELECT pg_get_constraintdef(c.oid) FROM pg_constraint c LIMIT 1;",
    pg("functions-info.html")
  );
  f!(
    "pg_terminate_backend",
    "pg_terminate_backend(pid int) -> boolean",
    "Terminate a backend by PID (needs pg_signal_backend role).",
    "SELECT pg_terminate_backend(12345);",
    pg("functions-admin.html")
  );
  f!(
    "pg_cancel_backend",
    "pg_cancel_backend(pid int) -> boolean",
    "Cancel the current query on a backend by PID.",
    "SELECT pg_cancel_backend(12345);",
    pg("functions-admin.html")
  );
  f!(
    "pg_trigger_depth",
    "pg_trigger_depth() -> integer",
    "Current nesting level of PG triggers (0 outside any trigger).",
    "SELECT pg_trigger_depth();",
    pg("functions-info.html")
  );
  f!(
    "array_ndims",
    "array_ndims(anyarray) -> integer",
    "Number of dimensions of the array.",
    "SELECT array_ndims(ARRAY[[1,2],[3,4]]);",
    pg("functions-array.html")
  );
  f!(
    "array_upper",
    "array_upper(anyarray, dim int) -> integer",
    "Upper bound of the requested dimension.",
    "SELECT array_upper(ARRAY[10,20,30], 1);",
    pg("functions-array.html")
  );
  f!(
    "array_lower",
    "array_lower(anyarray, dim int) -> integer",
    "Lower bound of the requested dimension.",
    "SELECT array_lower(ARRAY[10,20,30], 1);",
    pg("functions-array.html")
  );
  f!(
    "array_dims",
    "array_dims(anyarray) -> text",
    "Textual representation of the array dimensions.",
    "SELECT array_dims(ARRAY[[1,2],[3,4]]);",
    pg("functions-array.html")
  );
  f!(
    "array_prepend",
    "array_prepend(elt anyelement, arr anyarray) -> anyarray",
    "Prepend an element.",
    "SELECT array_prepend(0, ARRAY[1,2,3]);",
    pg("functions-array.html")
  );
  f!(
    "array_append",
    "array_append(arr anyarray, elt anyelement) -> anyarray",
    "Append an element (also operator `||`).",
    "SELECT array_append(ARRAY[1,2], 3);",
    pg("functions-array.html")
  );
  f!(
    "array_remove",
    "array_remove(arr anyarray, elt anyelement) -> anyarray",
    "Remove all occurrences of elt from the array.",
    "SELECT array_remove(ARRAY[1,2,3,2], 2);",
    pg("functions-array.html")
  );
  f!(
    "array_replace",
    "array_replace(arr anyarray, from anyelement, to anyelement) -> anyarray",
    "Replace every from with to.",
    "SELECT array_replace(ARRAY[1,2,3,2], 2, 99);",
    pg("functions-array.html")
  );
  f!(
    "array_cat",
    "array_cat(a anyarray, b anyarray) -> anyarray",
    "Concatenate two arrays.",
    "SELECT array_cat(ARRAY[1,2], ARRAY[3,4]);",
    pg("functions-array.html")
  );
  f!(
    "array_position",
    "array_position(arr anyarray, elt anyelement) -> integer",
    "1-based index of elt, NULL if not present.",
    "SELECT array_position(ARRAY[10,20,30,40], 30);",
    pg("functions-array.html")
  );
  f!(
    "array_positions",
    "array_positions(arr anyarray, elt anyelement) -> int[]",
    "All 1-based indexes of elt.",
    "SELECT array_positions(ARRAY[1,2,1,3], 1);",
    pg("functions-array.html")
  );
  f!(
    "array_to_string",
    "array_to_string(arr anyarray, sep text [, null_str text]) -> text",
    "Join array elements with sep.",
    "SELECT array_to_string(ARRAY[1,2,3], ',', '*');",
    pg("functions-array.html")
  );
  f!(
    "string_to_array",
    "string_to_array(text, sep text [, null_str text]) -> text[]",
    "Split text into a text[].",
    "SELECT string_to_array('a,b,,c', ',');",
    pg("functions-array.html")
  );
  f!(
    "cardinality",
    "cardinality(anyarray) -> integer",
    "Total element count across all dimensions.",
    "SELECT cardinality(ARRAY[1,2,3]);",
    pg("functions-array.html")
  );
  f!(
    "trim_array",
    "trim_array(anyarray, n int) -> anyarray",
    "Return all but the last n elements.",
    "SELECT trim_array(ARRAY[1,2,3,4], 1);",
    pg("functions-array.html")
  );
  // current_* time/date are SQL-standard functions; they also work without parens.
  f!(
    "current_time",
    "current_time [(p int)] -> time with time zone",
    "Current TIME WITH TIME ZONE (optional precision).",
    "SELECT current_time(3);",
    pg("functions-datetime.html")
  );
  f!(
    "current_timestamp",
    "current_timestamp [(p int)] -> timestamp with time zone",
    "Current TIMESTAMPTZ (optional precision).",
    "SELECT current_timestamp(0);",
    pg("functions-datetime.html")
  );
  f!(
    "current_date",
    "current_date -> date",
    "Current DATE (no parens).",
    "SELECT current_date;",
    pg("functions-datetime.html")
  );
  f!(
    "clock_timestamp",
    "clock_timestamp() -> timestamp with time zone",
    "Wall-clock TIMESTAMPTZ; changes within a transaction.",
    "SELECT clock_timestamp();",
    pg("functions-datetime.html")
  );
  f!(
    "statement_timestamp",
    "statement_timestamp() -> timestamp with time zone",
    "TIMESTAMPTZ of statement start.",
    "SELECT statement_timestamp();",
    pg("functions-datetime.html")
  );
  f!(
    "transaction_timestamp",
    "transaction_timestamp() -> timestamp with time zone",
    "Alias for now(); TIMESTAMPTZ of transaction start.",
    "SELECT transaction_timestamp();",
    pg("functions-datetime.html")
  );
  f!(
    "timeofday",
    "timeofday() -> text",
    "Wall-clock time as text (legacy).",
    "SELECT timeofday();",
    pg("functions-datetime.html")
  );
  f!(
    "substr",
    "substr(text, start int [, len int]) -> text",
    "Positional substring (PG alias for SQL-standard substring(... FROM n FOR m)).",
    "SELECT substr('hello', 2, 3);",
    pg("functions-string.html")
  );
  f!(
    "trim",
    "trim([LEADING|TRAILING|BOTH] [chars] FROM text) -> text | trim(text [, chars]) -> text",
    "Trim characters from both/leading/trailing.",
    "SELECT trim(BOTH 'x' FROM 'xxhixx');",
    pg("functions-string.html")
  );
  f!(
    "char_length",
    "char_length(text) -> integer",
    "Character count of text.",
    "SELECT char_length('héllo');",
    pg("functions-string.html")
  );
  f!(
    "character_length",
    "character_length(text) -> integer",
    "Character count of text (SQL-standard spelling).",
    "SELECT character_length('héllo');",
    pg("functions-string.html")
  );
  f!("md5", "md5(text|bytea) -> text", "MD5 hash hex digest.", "SELECT md5('hello');", pg("functions-string.html"));
  f!(
    "position",
    "position(needle IN haystack) -> integer",
    "SQL-standard 1-based index of needle.",
    "SELECT position('world' IN 'hello world');",
    pg("functions-string.html")
  );
  f!("cbrt", "cbrt(double) -> double precision", "Cube root.", "SELECT cbrt(27);", pg("functions-math.html"));
  f!("gcd", "gcd(int, int) -> integer", "Greatest common divisor.", "SELECT gcd(12, 18);", pg("functions-math.html"));
  f!("lcm", "lcm(int, int) -> integer", "Least common multiple.", "SELECT lcm(4, 6);", pg("functions-math.html"));
  f!(
    "scale",
    "scale(numeric) -> integer",
    "Scale of a numeric value (digits after decimal).",
    "SELECT scale(1.230);",
    pg("functions-math.html")
  );
  f!(
    "min_scale",
    "min_scale(numeric) -> integer",
    "Minimum scale needed to represent value exactly.",
    "SELECT min_scale(1.230);",
    pg("functions-math.html")
  );
  f!(
    "trim_scale",
    "trim_scale(numeric) -> numeric",
    "Strip trailing zeros after decimal point.",
    "SELECT trim_scale(1.2300);",
    pg("functions-math.html")
  );
  f!(
    "width_bucket",
    "width_bucket(operand, b1, b2, count int) -> integer",
    "Histogram bucket number for operand in count equal-width buckets between b1 and b2.",
    "SELECT width_bucket(5.0, 0, 10, 4);",
    pg("functions-math.html")
  );
  // Statistical / regression aggregates.
  f!(
    "regr_slope",
    "regr_slope(y, x) -> double precision",
    "Slope of the linear regression line.",
    "SELECT regr_slope(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_intercept",
    "regr_intercept(y, x) -> double precision",
    "Intercept of the linear regression line.",
    "SELECT regr_intercept(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_r2",
    "regr_r2(y, x) -> double precision",
    "Square of the correlation coefficient (R²).",
    "SELECT regr_r2(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_count",
    "regr_count(y, x) -> bigint",
    "Count of input rows in which both inputs are non-null.",
    "SELECT regr_count(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_avgx",
    "regr_avgx(y, x) -> double precision",
    "Average of the x values.",
    "SELECT regr_avgx(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_avgy",
    "regr_avgy(y, x) -> double precision",
    "Average of the y values.",
    "SELECT regr_avgy(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_sxx",
    "regr_sxx(y, x) -> double precision",
    "Sum of squares of the x values.",
    "SELECT regr_sxx(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_syy",
    "regr_syy(y, x) -> double precision",
    "Sum of squares of the y values.",
    "SELECT regr_syy(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "regr_sxy",
    "regr_sxy(y, x) -> double precision",
    "Sum of products of x*y.",
    "SELECT regr_sxy(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "array_to_json",
    "array_to_json(anyarray [, pretty bool]) -> json",
    "Convert an array to a JSON array.",
    "SELECT array_to_json(ARRAY[1,2,3]);",
    pg("functions-json.html")
  );
  f!(
    "row_to_json",
    "row_to_json(record [, pretty bool]) -> json",
    "Convert a row to a JSON object.",
    "SELECT row_to_json(t) FROM users t;",
    pg("functions-json.html")
  );
  f!(
    "date_bin",
    "date_bin(interval, source ts, origin ts) -> timestamp(tz)",
    "Snap a timestamp to the nearest bucket of size `interval` aligned to `origin`.",
    "SELECT date_bin('15 minutes', now(), '2000-01-01');",
    pg("functions-datetime.html")
  );
  f!(
    "timezone",
    "timezone(zone text, ts timestamp(tz)) -> timestamp(tz) | timezone(zone, time) -> time",
    "SQL-standard alternate to `AT TIME ZONE`.",
    "SELECT timezone('UTC', now());",
    pg("functions-datetime.html")
  );
  f!(
    "isfinite",
    "isfinite(date|timestamp|interval) -> boolean",
    "True when value is not -infinity/+infinity.",
    "SELECT isfinite(now());",
    pg("functions-datetime.html")
  );
  // Text search internals.
  f!(
    "numnode",
    "numnode(tsquery) -> integer",
    "Number of lexemes + operators in a tsquery.",
    "SELECT numnode(to_tsquery('a & b'));",
    pg("textsearch-features.html")
  );
  f!(
    "ts_lexize",
    "ts_lexize(dict regdictionary, token text) -> text[]",
    "Apply a text-search dictionary to a token.",
    "SELECT ts_lexize('simple', 'hello');",
    pg("functions-textsearch.html")
  );
  f!(
    "ts_parse",
    "ts_parse(parser_name text, text) -> SETOF (tokid int, token text)",
    "Tokenize text using a parser.",
    "SELECT * FROM ts_parse('default', 'hello world');",
    pg("functions-textsearch.html")
  );
  f!(
    "ts_token_type",
    "ts_token_type(parser_name text) -> SETOF (tokid int, alias text, descr text)",
    "List token types a parser recognises.",
    "SELECT * FROM ts_token_type('default');",
    pg("functions-textsearch.html")
  );
  f!(
    "ts_debug",
    "ts_debug(config_name text, doc text) -> SETOF (alias, descr, token, dictionaries, dictionary, lexemes)",
    "Diagnose tokenization of a string.",
    "SELECT * FROM ts_debug('english', 'hello world');",
    pg("functions-textsearch.html")
  );
  f!(
    "tsvector_to_array",
    "tsvector_to_array(tsvector) -> text[]",
    "Lexeme array from a tsvector.",
    "SELECT tsvector_to_array(to_tsvector('hello world'));",
    pg("functions-textsearch.html")
  );
  f!(
    "array_to_tsvector",
    "array_to_tsvector(text[]) -> tsvector",
    "Build a tsvector from an array of lexemes.",
    "SELECT array_to_tsvector(ARRAY['a', 'b']);",
    pg("functions-textsearch.html")
  );
  f!(
    "strip",
    "strip(tsvector) -> tsvector",
    "Strip positions/weights from a tsvector.",
    "SELECT strip(to_tsvector('hello world'));",
    pg("functions-textsearch.html")
  );
  f!(
    "length",
    "length(text|bit|tsvector|...) -> integer",
    "Length / lexeme count depending on argument.",
    "SELECT length(to_tsvector('hello world'));",
    pg("functions-string.html")
  );

  // ---- Trigonometric ----
  f!("sin", "sin(double precision) -> double precision", "Sine, radians.", "SELECT sin(0);", pg("functions-math.html"));
  f!(
    "cos",
    "cos(double precision) -> double precision",
    "Cosine, radians.",
    "SELECT cos(0);",
    pg("functions-math.html")
  );
  f!(
    "tan",
    "tan(double precision) -> double precision",
    "Tangent, radians.",
    "SELECT tan(0);",
    pg("functions-math.html")
  );
  f!(
    "asin",
    "asin(double precision) -> double precision",
    "Arc sine, radians.",
    "SELECT asin(1);",
    pg("functions-math.html")
  );
  f!(
    "acos",
    "acos(double precision) -> double precision",
    "Arc cosine, radians.",
    "SELECT acos(1);",
    pg("functions-math.html")
  );
  f!(
    "atan",
    "atan(double precision) -> double precision",
    "Arc tangent, radians.",
    "SELECT atan(1);",
    pg("functions-math.html")
  );
  f!(
    "atan2",
    "atan2(y double, x double) -> double precision",
    "Arc tangent of y/x, radians.",
    "SELECT atan2(1, 1);",
    pg("functions-math.html")
  );

  // ---- Bit aggregates ----
  f!(
    "bit_and",
    "bit_and(integer) -> integer",
    "Bitwise AND of all non-null values.",
    "SELECT bit_and(flags) FROM perms;",
    pg("functions-aggregate.html")
  );
  f!(
    "bit_or",
    "bit_or(integer) -> integer",
    "Bitwise OR of all non-null values.",
    "SELECT bit_or(flags) FROM perms;",
    pg("functions-aggregate.html")
  );
  f!(
    "bit_xor",
    "bit_xor(integer) -> integer",
    "Bitwise XOR of all non-null values (PG14+).",
    "SELECT bit_xor(checksum) FROM blocks;",
    pg("functions-aggregate.html")
  );

  // ---- Stats aggregates ----
  f!(
    "corr",
    "corr(y double, x double) -> double precision",
    "Correlation coefficient.",
    "SELECT corr(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "covar_pop",
    "covar_pop(y, x) -> double precision",
    "Population covariance.",
    "SELECT covar_pop(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "covar_samp",
    "covar_samp(y, x) -> double precision",
    "Sample covariance.",
    "SELECT covar_samp(price, qty) FROM items;",
    pg("functions-aggregate.html")
  );
  f!(
    "stddev_pop",
    "stddev_pop(numeric) -> numeric",
    "Population standard deviation.",
    "SELECT stddev_pop(grade) FROM tests;",
    pg("functions-aggregate.html")
  );
  f!(
    "stddev_samp",
    "stddev_samp(numeric) -> numeric",
    "Sample standard deviation.",
    "SELECT stddev_samp(grade) FROM tests;",
    pg("functions-aggregate.html")
  );
  f!(
    "var_pop",
    "var_pop(numeric) -> numeric",
    "Population variance.",
    "SELECT var_pop(grade) FROM tests;",
    pg("functions-aggregate.html")
  );
  f!(
    "var_samp",
    "var_samp(numeric) -> numeric",
    "Sample variance.",
    "SELECT var_samp(grade) FROM tests;",
    pg("functions-aggregate.html")
  );

  // ---- Full-text search ----
  f!(
    "to_tsvector",
    "to_tsvector([config regconfig,] text) -> tsvector",
    "Convert text to a tsvector.",
    "SELECT to_tsvector('english', 'duck typing');",
    pg("textsearch-controls.html")
  );
  f!(
    "to_tsquery",
    "to_tsquery([config,] text) -> tsquery",
    "Convert query text to tsquery.",
    "SELECT to_tsquery('english', 'duck & typing');",
    pg("textsearch-controls.html")
  );
  f!(
    "plainto_tsquery",
    "plainto_tsquery([config,] text) -> tsquery",
    "Plain phrase -> tsquery (no operators).",
    "SELECT plainto_tsquery('duck typing');",
    pg("textsearch-controls.html")
  );
  f!(
    "phraseto_tsquery",
    "phraseto_tsquery([config,] text) -> tsquery",
    "Phrase tsquery using `<->`.",
    "SELECT phraseto_tsquery('duck typing');",
    pg("textsearch-controls.html")
  );
  f!(
    "websearch_to_tsquery",
    "websearch_to_tsquery([config,] text) -> tsquery",
    "Web-search-style tsquery (quotes, OR, -term).",
    "SELECT websearch_to_tsquery('\"duck typing\" OR rust');",
    pg("textsearch-controls.html")
  );
  f!(
    "ts_rank",
    "ts_rank(tsvector, tsquery) -> real",
    "Rank a document against a query.",
    "SELECT ts_rank(doc, q) FROM ...",
    pg("textsearch-controls.html")
  );
  f!(
    "ts_rank_cd",
    "ts_rank_cd(tsvector, tsquery) -> real",
    "Cover density rank.",
    "SELECT ts_rank_cd(doc, q) FROM ...",
    pg("textsearch-controls.html")
  );
  f!(
    "ts_headline",
    "ts_headline([config,] text, tsquery) -> text",
    "Snippet with matched terms highlighted.",
    "SELECT ts_headline('the duck quacks', q);",
    pg("textsearch-controls.html")
  );

  // ---- JSONB path (SQL/JSON) ----
  f!(
    "jsonb_path_exists",
    "jsonb_path_exists(jsonb, jsonpath) -> boolean",
    "True if any item matches the path.",
    "SELECT jsonb_path_exists(doc, '$.a.b ? (@ > 0)');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_path_match",
    "jsonb_path_match(jsonb, jsonpath) -> boolean",
    "Match path returning a single boolean.",
    "SELECT jsonb_path_match(doc, 'exists($.x)');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_path_query_first",
    "jsonb_path_query_first(jsonb, jsonpath) -> jsonb",
    "First matching item or NULL.",
    "SELECT jsonb_path_query_first(doc, '$.items[0]');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_path_query_array",
    "jsonb_path_query_array(jsonb, jsonpath) -> jsonb",
    "All matches packed into a jsonb array.",
    "SELECT jsonb_path_query_array(doc, '$.items[*]');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_insert",
    "jsonb_insert(target jsonb, path text[], new jsonb [, insert_after bool]) -> jsonb",
    "Insert a value at a jsonb path.",
    "SELECT jsonb_insert('{\"a\":[1]}'::jsonb, '{a,1}', '2');",
    pg("functions-json.html")
  );

  // ---- Object lookups (regclass et al.) ----
  f!(
    "to_regclass",
    "to_regclass(text) -> regclass",
    "OID lookup; NULL when missing (vs `::regclass` which errors).",
    "SELECT to_regclass('public.users');",
    pg("functions-info.html#FUNCTIONS-INFO-OBJECT")
  );
  f!(
    "to_regproc",
    "to_regproc(text) -> regproc",
    "OID lookup for a function name.",
    "SELECT to_regproc('lower');",
    pg("functions-info.html#FUNCTIONS-INFO-OBJECT")
  );
  f!(
    "to_regtype",
    "to_regtype(text) -> regtype",
    "OID lookup for a type name.",
    "SELECT to_regtype('int4');",
    pg("functions-info.html#FUNCTIONS-INFO-OBJECT")
  );
  f!(
    "to_regnamespace",
    "to_regnamespace(text) -> regnamespace",
    "OID lookup for a schema.",
    "SELECT to_regnamespace('public');",
    pg("functions-info.html#FUNCTIONS-INFO-OBJECT")
  );
  f!(
    "to_regrole",
    "to_regrole(text) -> regrole",
    "OID lookup for a role.",
    "SELECT to_regrole('postgres');",
    pg("functions-info.html#FUNCTIONS-INFO-OBJECT")
  );
  f!(
    "pg_get_userbyid",
    "pg_get_userbyid(oid) -> name",
    "Role name for OID.",
    "SELECT pg_get_userbyid(10);",
    pg("functions-info.html")
  );
  f!(
    "pg_get_serial_sequence",
    "pg_get_serial_sequence(table_name, column_name) -> text",
    "Sequence backing a SERIAL/IDENTITY column.",
    "SELECT pg_get_serial_sequence('users', 'id');",
    pg("functions-info.html")
  );

  // ---- Size & stats ----
  f!(
    "pg_table_size",
    "pg_table_size(regclass) -> bigint",
    "On-disk size of a table (excluding indexes, TOAST sums separately).",
    "SELECT pg_size_pretty(pg_table_size('users'));",
    pg("functions-admin.html")
  );
  f!(
    "pg_indexes_size",
    "pg_indexes_size(regclass) -> bigint",
    "Total size of all indexes attached to a relation.",
    "SELECT pg_size_pretty(pg_indexes_size('users'));",
    pg("functions-admin.html")
  );
  f!(
    "pg_relation_size",
    "pg_relation_size(regclass [, fork]) -> bigint",
    "Size of one fork of a relation.",
    "SELECT pg_relation_size('users');",
    pg("functions-admin.html")
  );
  f!(
    "pg_total_relation_size",
    "pg_total_relation_size(regclass) -> bigint",
    "Total disk usage of a relation including indexes + TOAST.",
    "SELECT pg_size_pretty(pg_total_relation_size('users'));",
    pg("functions-admin.html")
  );
  f!(
    "pg_database_size",
    "pg_database_size(name) -> bigint",
    "Total disk usage of a database.",
    "SELECT pg_size_pretty(pg_database_size('app'));",
    pg("functions-admin.html")
  );

  // ---- Enums / arrays ----
  f!(
    "enum_first",
    "enum_first(anyenum) -> anyenum",
    "First label of an enum.",
    "SELECT enum_first(NULL::status);",
    pg("functions-enum.html")
  );
  f!(
    "enum_last",
    "enum_last(anyenum) -> anyenum",
    "Last label of an enum.",
    "SELECT enum_last(NULL::status);",
    pg("functions-enum.html")
  );
  f!(
    "enum_range",
    "enum_range([anyenum [, anyenum]]) -> anyarray",
    "Array of enum labels in declared order.",
    "SELECT enum_range(NULL::status);",
    pg("functions-enum.html")
  );
  f!(
    "array_fill",
    "array_fill(anyelement, int[] [, int[]]) -> anyarray",
    "Create an array filled with copies of one value.",
    "SELECT array_fill(0, ARRAY[3]);",
    pg("functions-array.html")
  );

  // ---- Misc heavy traffic ----
  f!(
    "concat_ws",
    "concat_ws(sep text, args...) -> text",
    "Concatenate non-null args with separator.",
    "SELECT concat_ws(', ', first, middle, last);",
    pg("functions-string.html")
  );
  f!(
    "to_regoperator",
    "to_regoperator(text) -> regoperator",
    "OID lookup for an operator with operand types.",
    "SELECT to_regoperator('=(int,int)');",
    pg("functions-info.html#FUNCTIONS-INFO-OBJECT")
  );
  f!(
    "pg_current_xact_id",
    "pg_current_xact_id() -> xid8",
    "Current transaction's xid8 (read-write).",
    "SELECT pg_current_xact_id();",
    pg("functions-info.html")
  );
  f!(
    "pg_xact_status",
    "pg_xact_status(xid8) -> text",
    "Status of a transaction: committed / in progress / aborted.",
    "SELECT pg_xact_status(pg_current_xact_id());",
    pg("functions-info.html")
  );

  // ---- Comparison / NULL counting ----
  // Full-text search helpers.
  f!(
    "setweight",
    "setweight(tsvector, weight char) -> tsvector",
    "Tag every lexeme with a weight letter (A/B/C/D).",
    "SELECT setweight(to_tsvector('title'), 'A');",
    pg("textsearch-features.html#TEXTSEARCH-MANIPULATE-TSVECTOR")
  );
  f!(
    "ts_headline",
    "ts_headline([config], doc, query [, options]) -> text",
    "Highlight query matches in a document.",
    "SELECT ts_headline('eng', body, query) FROM docs;",
    pg("textsearch-controls.html#TEXTSEARCH-HEADLINE")
  );
  f!(
    "plainto_tsquery",
    "plainto_tsquery([config], text) -> tsquery",
    "Convert text to a tsquery with AND semantics.",
    "SELECT plainto_tsquery('eng', 'hello world');",
    pg("textsearch-controls.html")
  );
  f!(
    "similarity",
    "similarity(text, text) -> real",
    "pg_trgm similarity score (0..1).",
    "SELECT similarity('foo', 'foobar');",
    pg("pgtrgm.html")
  );
  f!(
    "word_similarity",
    "word_similarity(text, text) -> real",
    "pg_trgm word-level similarity.",
    "SELECT word_similarity('rust', 'postgres rust');",
    pg("pgtrgm.html")
  );
  f!(
    "strict_word_similarity",
    "strict_word_similarity(text, text) -> real",
    "Strict word similarity (pg_trgm).",
    "SELECT strict_word_similarity('foo', 'foobar');",
    pg("pgtrgm.html")
  );
  f!(
    "show_trgm",
    "show_trgm(text) -> text[]",
    "Return trigrams of a string.",
    "SELECT show_trgm('foobar');",
    pg("pgtrgm.html")
  );

  // Additional jsonb fns.
  f!(
    "jsonb_build_array",
    "jsonb_build_array(VARIADIC \"any\") -> jsonb",
    "Build a jsonb array from variadic args.",
    "SELECT jsonb_build_array('a', 1, true);",
    pg("functions-json.html")
  );
  f!(
    "jsonb_object_agg",
    "jsonb_object_agg(key, value) -> jsonb",
    "Aggregate key/value pairs into a jsonb object.",
    "SELECT jsonb_object_agg(k, v) FROM kv;",
    pg("functions-aggregate.html")
  );
  f!(
    "jsonb_array_elements_text",
    "jsonb_array_elements_text(jsonb) -> setof text",
    "Expand jsonb array into rows of text.",
    "SELECT * FROM jsonb_array_elements_text('[\"a\",\"b\"]');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_each_text",
    "jsonb_each_text(jsonb) -> setof (text, text)",
    "Expand jsonb object into key/value text rows.",
    "SELECT * FROM jsonb_each_text('{\"a\":1}');",
    pg("functions-json.html")
  );
  f!(
    "json_build_array",
    "json_build_array(VARIADIC \"any\") -> json",
    "Build a json array.",
    "SELECT json_build_array(1, 2, 3);",
    pg("functions-json.html")
  );
  f!(
    "json_object_agg",
    "json_object_agg(key, value) -> json",
    "Aggregate key/value pairs into a json object.",
    "SELECT json_object_agg(k, v) FROM kv;",
    pg("functions-aggregate.html")
  );
  f!(
    "json_array_elements_text",
    "json_array_elements_text(json) -> setof text",
    "Expand json array as text rows.",
    "SELECT * FROM json_array_elements_text('[\"a\"]');",
    pg("functions-json.html")
  );
  f!(
    "json_each_text",
    "json_each_text(json) -> setof (text, text)",
    "Expand json object as text key/value rows.",
    "SELECT * FROM json_each_text('{\"a\":\"b\"}');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_insert",
    "jsonb_insert(target, path, new_value [, insert_after]) -> jsonb",
    "Insert into a jsonb structure.",
    "SELECT jsonb_insert(data, '{0}', '\"x\"');",
    pg("functions-json.html")
  );
  f!(
    "jsonb_object",
    "jsonb_object(text[] [, text[]]) -> jsonb",
    "Build a jsonb object from a key array (+ optional value array).",
    "SELECT jsonb_object('{a,b}', '{1,2}');",
    pg("functions-json.html")
  );
  f!(
    "json_object",
    "json_object(text[] [, text[]]) -> json",
    "Build a json object from key/value arrays.",
    "SELECT json_object('{a,b}', '{1,2}');",
    pg("functions-json.html")
  );

  // Geometric type constructors.
  f!(
    "point",
    "point(x float8, y float8) -> point",
    "Construct a geometric point.",
    "SELECT point(40.7, -74.0);",
    pg("functions-geometry.html")
  );
  f!(
    "box",
    "box(point, point) -> box",
    "Construct a rectangular box from two points.",
    "SELECT box(point(0,0), point(1,1));",
    pg("functions-geometry.html")
  );
  f!(
    "circle",
    "circle(point, float8) -> circle",
    "Construct a circle from center + radius.",
    "SELECT circle(point(0,0), 5);",
    pg("functions-geometry.html")
  );
  f!(
    "line",
    "line(point, point) -> line",
    "Construct a line from two points.",
    "SELECT line(point(0,0), point(1,1));",
    pg("functions-geometry.html")
  );
  f!(
    "lseg",
    "lseg(point, point) -> lseg",
    "Construct a line segment.",
    "SELECT lseg(point(0,0), point(1,1));",
    pg("functions-geometry.html")
  );
  f!(
    "path",
    "path(polygon) -> path",
    "Construct a path from a polygon.",
    "SELECT path('((0,0),(1,1),(2,2))'::polygon);",
    pg("functions-geometry.html")
  );
  f!(
    "polygon",
    "polygon(box) -> polygon",
    "Construct a polygon from a box.",
    "SELECT polygon(box(point(0,0), point(1,1)));",
    pg("functions-geometry.html")
  );

  // uuid-ossp extension fns (heavily used in real schemas).
  f!(
    "uuid_generate_v4",
    "uuid_generate_v4() -> uuid",
    "Random UUID (v4). Requires the uuid-ossp extension.",
    "SELECT uuid_generate_v4();",
    pg("uuid-ossp.html")
  );
  f!(
    "uuid_generate_v1",
    "uuid_generate_v1() -> uuid",
    "MAC-address + timestamp UUID (v1).",
    "SELECT uuid_generate_v1();",
    pg("uuid-ossp.html")
  );
  f!(
    "uuid_generate_v3",
    "uuid_generate_v3(namespace uuid, name text) -> uuid",
    "Name-based UUID (v3) using MD5.",
    "SELECT uuid_generate_v3(uuid_ns_dns(), 'example.com');",
    pg("uuid-ossp.html")
  );
  f!(
    "uuid_generate_v5",
    "uuid_generate_v5(namespace uuid, name text) -> uuid",
    "Name-based UUID (v5) using SHA-1.",
    "SELECT uuid_generate_v5(uuid_ns_dns(), 'example.com');",
    pg("uuid-ossp.html")
  );
  f!("uuid_nil", "uuid_nil() -> uuid", "All-zero UUID.", "SELECT uuid_nil();", pg("uuid-ossp.html"));
  f!("uuid_ns_dns", "uuid_ns_dns() -> uuid", "DNS namespace UUID.", "SELECT uuid_ns_dns();", pg("uuid-ossp.html"));
  f!("uuid_ns_url", "uuid_ns_url() -> uuid", "URL namespace UUID.", "SELECT uuid_ns_url();", pg("uuid-ossp.html"));
  f!(
    "gen_random_uuid",
    "gen_random_uuid() -> uuid",
    "Random UUID (built-in on PG13+, no extension).",
    "SELECT gen_random_uuid();",
    pg("functions-uuid.html")
  );

  // Common math + date helpers that were missing.
  f!("ceil", "ceil(numeric) -> numeric", "Round up to integer.", "SELECT ceil(3.2);", pg("functions-math.html"));
  f!(
    "div",
    "div(y numeric, x numeric) -> numeric",
    "Integer quotient of y/x.",
    "SELECT div(10, 3);",
    pg("functions-math.html")
  );
  f!(
    "justify_days",
    "justify_days(interval) -> interval",
    "Adjust 30-day periods into months.",
    "SELECT justify_days(interval '60 days');",
    pg("functions-datetime.html")
  );
  f!(
    "justify_hours",
    "justify_hours(interval) -> interval",
    "Adjust 24-hour periods into days.",
    "SELECT justify_hours(interval '50 hours');",
    pg("functions-datetime.html")
  );
  f!(
    "justify_interval",
    "justify_interval(interval) -> interval",
    "Adjust both days and hours.",
    "SELECT justify_interval(interval '60 days 50 hours');",
    pg("functions-datetime.html")
  );
  f!(
    "sha256",
    "sha256(bytea) -> bytea",
    "SHA-256 digest.",
    "SELECT encode(sha256('hello'::bytea), 'hex');",
    pg("functions-binarystring.html")
  );
  f!(
    "sha224",
    "sha224(bytea) -> bytea",
    "SHA-224 digest.",
    "SELECT sha224('hello'::bytea);",
    pg("functions-binarystring.html")
  );
  f!(
    "sha384",
    "sha384(bytea) -> bytea",
    "SHA-384 digest.",
    "SELECT sha384('hello'::bytea);",
    pg("functions-binarystring.html")
  );
  f!(
    "sha512",
    "sha512(bytea) -> bytea",
    "SHA-512 digest.",
    "SELECT sha512('hello'::bytea);",
    pg("functions-binarystring.html")
  );

  // Window-only rank functions (siblings of row_number / rank already
  // listed above).
  f!(
    "percent_rank",
    "percent_rank() -> double precision",
    "Relative rank of the current row (0..1), excluding the current row's peers.",
    "SELECT percent_rank() OVER (ORDER BY salary) FROM employees;",
    pg("functions-window.html")
  );
  f!(
    "cume_dist",
    "cume_dist() -> double precision",
    "Cumulative distribution of the current row -- fraction of partition rows with values <= current.",
    "SELECT cume_dist() OVER (ORDER BY salary) FROM employees;",
    pg("functions-window.html")
  );

  f!(
    "num_nonnulls",
    "num_nonnulls(VARIADIC \"any\") -> int",
    "Count of non-NULL arguments. Useful in CHECK constraints to require exactly one of N columns.",
    "CHECK (num_nonnulls(promo_id, voucher_id) = 1)",
    pg("functions-comparison.html#FUNCTIONS-COMPARISON-FUNC-TABLE")
  );
  f!(
    "num_nulls",
    "num_nulls(VARIADIC \"any\") -> int",
    "Count of NULL arguments. Inverse of num_nonnulls.",
    "SELECT num_nulls(a, b, c);",
    pg("functions-comparison.html#FUNCTIONS-COMPARISON-FUNC-TABLE")
  );

  // ---- Sequence helpers (commonly missing) ----
  f!(
    "nextval",
    "nextval(regclass) -> bigint",
    "Advance sequence and return next value.",
    "SELECT nextval('users_id_seq');",
    pg("functions-sequence.html")
  );
  f!(
    "currval",
    "currval(regclass) -> bigint",
    "Last value returned by nextval in this session.",
    "SELECT currval('users_id_seq');",
    pg("functions-sequence.html")
  );
  f!(
    "setval",
    "setval(regclass, bigint [, boolean]) -> bigint",
    "Set the sequence's current value.",
    "SELECT setval('users_id_seq', 1000);",
    pg("functions-sequence.html")
  );
  f!(
    "lastval",
    "lastval() -> bigint",
    "Last value returned by nextval anywhere in the session, any sequence.",
    "SELECT lastval();",
    pg("functions-sequence.html")
  );

  // ---- Range constructors ----
  f!(
    "int4range",
    "int4range(lower int, upper int [, bounds text]) -> int4range",
    "Construct an int4 range.",
    "SELECT int4range(1, 10);",
    pg("rangetypes.html")
  );
  f!(
    "int8range",
    "int8range(lower bigint, upper bigint [, bounds text]) -> int8range",
    "Construct an int8 range.",
    "SELECT int8range(1, 10);",
    pg("rangetypes.html")
  );
  f!(
    "numrange",
    "numrange(lower numeric, upper numeric [, bounds text]) -> numrange",
    "Construct a numeric range.",
    "SELECT numrange(0.5, 1.5);",
    pg("rangetypes.html")
  );
  f!(
    "tsrange",
    "tsrange(lower timestamp, upper timestamp [, bounds text]) -> tsrange",
    "Construct a timestamp range.",
    "SELECT tsrange(now(), now() + interval '1 day');",
    pg("rangetypes.html")
  );
  f!(
    "tstzrange",
    "tstzrange(lower timestamptz, upper timestamptz [, bounds text]) -> tstzrange",
    "Construct a timestamptz range.",
    "SELECT tstzrange(now(), now() + interval '1 day');",
    pg("rangetypes.html")
  );
  f!(
    "daterange",
    "daterange(lower date, upper date [, bounds text]) -> daterange",
    "Construct a date range.",
    "SELECT daterange('2024-01-01', '2024-12-31');",
    pg("rangetypes.html")
  );

  // ---- Missing-essentials batch ---------------------------------
  f!("cast", "CAST(expr AS type) -- explicit type conversion. Operator alias: `expr::type`.", "Convert between types.", "SELECT CAST('42' AS int);", pg("sql-expressions.html#SQL-SYNTAX-TYPE-CASTS"));
  f!("user", "user -> name", "Current SQL role (alias for current_user).", "SELECT user;", pg("functions-info.html"));
  f!("pg_version", "version() -> text", "Server version string (PG canonical name is `version`).", "SELECT version();", pg("functions-info.html"));
  f!("localtimestamp", "localtimestamp -> timestamp", "Current timestamp WITHOUT time zone, fixed for the transaction.", "SELECT localtimestamp;", pg("functions-datetime.html"));
  f!("localtime", "localtime -> time", "Current time of day WITHOUT time zone, fixed for the transaction.", "SELECT localtime;", pg("functions-datetime.html"));
  f!("jsonb_extract_path", "jsonb_extract_path(jsonb, VARIADIC text) -> jsonb", "Extract sub-object at the given key path (alias for `#>`).", "SELECT jsonb_extract_path(data, 'addr','city') FROM t;", pg("functions-json.html"));
  f!("jsonb_extract_path_text", "jsonb_extract_path_text(jsonb, VARIADIC text) -> text", "Same as jsonb_extract_path, but coerce final value to text (alias for `#>>`).", "SELECT jsonb_extract_path_text(data,'addr','city');", pg("functions-json.html"));
  f!("json_array_elements", "json_array_elements(json) -> setof json", "Expand a JSON array into a set of JSON elements.", "SELECT * FROM json_array_elements('[1,2,3]'::json);", pg("functions-json.html"));
  f!("pow", "pow(a numeric, b numeric) -> numeric", "Alias for `power(a, b)`.", "SELECT pow(2, 10);", pg("functions-math.html"));
  f!("log10", "log10(numeric) -> numeric", "Base-10 logarithm. PG canonical: `log(x)` (which is base-10).", "SELECT log10(1000);", pg("functions-math.html"));

  // ---- function sweep r93 -----------------------------------
  f!("abbrev", "abbrev(...) -> ...", "Network address function (inet / cidr).", "SELECT abbrev(...);", pg("functions-net.html"));
  f!("acosd", "acosd(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT acosd(...);", pg("functions-math.html"));
  f!("acosh", "acosh(double precision) -> double precision", "Hyperbolic / inverse-hyperbolic trig.", "SELECT acosh(...);", pg("functions-math.html"));
  f!("asind", "asind(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT asind(...);", pg("functions-math.html"));
  f!("asinh", "asinh(double precision) -> double precision", "Hyperbolic / inverse-hyperbolic trig.", "SELECT asinh(...);", pg("functions-math.html"));
  f!("atan2d", "atan2d(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT atan2d(...);", pg("functions-math.html"));
  f!("atand", "atand(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT atand(...);", pg("functions-math.html"));
  f!("atanh", "atanh(double precision) -> double precision", "Hyperbolic / inverse-hyperbolic trig.", "SELECT atanh(...);", pg("functions-math.html"));
  f!("broadcast", "broadcast(...) -> ...", "Network address function (inet / cidr).", "SELECT broadcast(...);", pg("functions-net.html"));
  f!("cosd", "cosd(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT cosd(...);", pg("functions-math.html"));
  f!("cosh", "cosh(double precision) -> double precision", "Hyperbolic / inverse-hyperbolic trig.", "SELECT cosh(...);", pg("functions-math.html"));
  f!("factorial", "factorial(bigint) -> numeric", "Factorial. Deprecated -- use `numeric_fac()` or `n!` operator.", "SELECT factorial(10);", pg("functions-math.html"));
  f!("family", "family(...) -> ...", "Network address function (inet / cidr).", "SELECT family(...);", pg("functions-net.html"));
  f!("hostmask", "hostmask(...) -> ...", "Network address function (inet / cidr).", "SELECT hostmask(...);", pg("functions-net.html"));
  f!("inet_client_addr", "inet_client_addr(...) -> ...", "Network address function (inet / cidr).", "SELECT inet_client_addr(...);", pg("functions-net.html"));
  f!("inet_merge", "inet_merge(...) -> ...", "Network address function (inet / cidr).", "SELECT inet_merge(...);", pg("functions-net.html"));
  f!("inet_same_family", "inet_same_family(...) -> ...", "Network address function (inet / cidr).", "SELECT inet_same_family(...);", pg("functions-net.html"));
  f!("inet_server_addr", "inet_server_addr(...) -> ...", "Network address function (inet / cidr).", "SELECT inet_server_addr(...);", pg("functions-net.html"));
  f!("masklen", "masklen(...) -> ...", "Network address function (inet / cidr).", "SELECT masklen(...);", pg("functions-net.html"));
  f!("overlaps", "(t1, t2) OVERLAPS (t3, t4) -> boolean", "(start1, end1) OVERLAPS (start2, end2) -- range overlap predicate.", "SELECT (DATE 2024-01-01, DATE 2024-12-31) OVERLAPS (DATE 2024-06-01, DATE 2025-06-01);", pg("functions-datetime.html"));
  f!("pgp_pub_decrypt", "pgp_pub_decrypt(...) -> ...", "pgcrypto PGP cipher function -- requires pgcrypto extension.", "SELECT pgp_pub_decrypt(...);", pg("pgcrypto.html"));
  f!("pgp_pub_encrypt", "pgp_pub_encrypt(...) -> ...", "pgcrypto PGP cipher function -- requires pgcrypto extension.", "SELECT pgp_pub_encrypt(...);", pg("pgcrypto.html"));
  f!("pgp_sym_decrypt", "pgp_sym_decrypt(...) -> ...", "pgcrypto PGP cipher function -- requires pgcrypto extension.", "SELECT pgp_sym_decrypt(...);", pg("pgcrypto.html"));
  f!("pgp_sym_encrypt", "pgp_sym_encrypt(...) -> ...", "pgcrypto PGP cipher function -- requires pgcrypto extension.", "SELECT pgp_sym_encrypt(...);", pg("pgcrypto.html"));
  f!("sind", "sind(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT sind(...);", pg("functions-math.html"));
  f!("sinh", "sinh(double precision) -> double precision", "Hyperbolic / inverse-hyperbolic trig.", "SELECT sinh(...);", pg("functions-math.html"));
  f!("tand", "tand(numeric) -> double precision", "Trigonometric function operating in degrees.", "SELECT tand(...);", pg("functions-math.html"));
  f!("tanh", "tanh(double precision) -> double precision", "Hyperbolic / inverse-hyperbolic trig.", "SELECT tanh(...);", pg("functions-math.html"));
  f!("text", "text(inet) -> text", "Convert internet address to text representation.", "SELECT text(...);", pg("functions-formatting.html"));
  f!("to_ascii", "to_ascii(text [, encoding]) -> text", "Transliterate to 7-bit ASCII.", "SELECT to_ascii(Karel);", pg("functions-string.html"));
  f!("xmlagg", "xmlagg(xml) -> xml", "Aggregate: concatenate per-row XML values into one XML forest (use `ORDER BY` for deterministic order).", "SELECT xmlagg(XMLELEMENT(NAME item, name) ORDER BY name) FROM products;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));

  // ---- round 148 missing fns ----
  f!("range_intersect", "range_intersect(anyrange, anyrange) -> anyrange", "Intersection of two ranges.", "SELECT int4range(1,10) * int4range(5,15);", pg("rangetypes.html"));
  f!("range_union", "range_union(anyrange, anyrange) -> anyrange", "Union of two ranges (must overlap or abut).", "SELECT int4range(1,5) + int4range(4,10);", pg("rangetypes.html"));
  f!("range_minus", "range_minus(anyrange, anyrange) -> anyrange", "Difference of two ranges.", "SELECT int4range(1,10) - int4range(5,8);", pg("rangetypes.html"));
  f!("lower_inf", "lower_inf(anyrange) -> boolean", "True when the range has no lower bound.", "SELECT lower_inf('(,5)'::int4range);", pg("rangetypes.html"));
  f!("upper_inf", "upper_inf(anyrange) -> boolean", "True when the range has no upper bound.", "SELECT upper_inf('[1,)'::int4range);", pg("rangetypes.html"));
  f!("pg_notify", "pg_notify(channel text, payload text) -> void", "Send a NOTIFY without requiring a literal channel name.", "SELECT pg_notify('chan', 'payload');", pg("functions-admin.html"));
  f!("pg_listening_channels", "pg_listening_channels() -> setof text", "Names of all channels the session is listening on.", "SELECT pg_listening_channels();", pg("functions-info.html"));
  f!("pg_current_xact_id_if_assigned", "pg_current_xact_id_if_assigned() -> xid8", "Current xid or NULL (does not assign one).", "SELECT pg_current_xact_id_if_assigned();", pg("functions-info.html"));
  f!("txid_current", "txid_current() -> bigint", "Deprecated -- use pg_current_xact_id() instead.", "SELECT txid_current();", pg("functions-info.html"));
  f!("pg_export_snapshot", "pg_export_snapshot() -> text", "Export the current snapshot id for parallel transactions.", "SELECT pg_export_snapshot();", pg("functions-admin.html"));
  f!("pg_tablespace_size", "pg_tablespace_size(oid|text) -> bigint", "Disk space used by a tablespace.", "SELECT pg_size_pretty(pg_tablespace_size('pg_default'));", pg("functions-admin.html"));
  f!("pg_size_bytes", "pg_size_bytes(text) -> bigint", "Parse a human-readable size string back into bytes.", "SELECT pg_size_bytes('64 MB');", pg("functions-admin.html"));
  f!("pg_column_size", "pg_column_size(any) -> int", "Bytes used to store a particular value (incl. TOAST overhead).", "SELECT pg_column_size(payload) FROM t;", pg("functions-admin.html"));
  f!("pg_relation_filenode", "pg_relation_filenode(regclass) -> oid", "On-disk filenode for a relation.", "SELECT pg_relation_filenode('users');", pg("functions-admin.html"));

  // ---- round 149 missing fns ----
  f!("pg_blocking_pids", "pg_blocking_pids(pid int) -> int[]", "PIDs of sessions whose locks block the given PID.", "SELECT pid, pg_blocking_pids(pid) FROM pg_stat_activity;", pg("functions-info.html"));
  f!("pg_backend_pid", "pg_backend_pid() -> int", "PID of the server process attached to the current session.", "SELECT pg_backend_pid();", pg("functions-info.html"));
  f!("pg_my_temp_schema", "pg_my_temp_schema() -> oid", "OID of the session's TEMP schema, or 0 if none.", "SELECT pg_my_temp_schema();", pg("functions-info.html"));
  f!("pg_xact_commit_timestamp", "pg_xact_commit_timestamp(xid) -> timestamptz", "Commit time of a transaction (requires track_commit_timestamp).", "SELECT pg_xact_commit_timestamp(xmin) FROM t;", pg("functions-info.html"));
  f!("pg_last_committed_xact", "pg_last_committed_xact() -> (xid, timestamptz, repl_origin_oid)", "Most recently committed transaction.", "SELECT * FROM pg_last_committed_xact();", pg("functions-info.html"));
  f!("pg_last_xact_replay_timestamp", "pg_last_xact_replay_timestamp() -> timestamptz", "Standby: when the last replayed transaction committed on primary.", "SELECT pg_last_xact_replay_timestamp();", pg("functions-admin.html"));
  f!("pg_is_in_recovery", "pg_is_in_recovery() -> boolean", "True when the server is in recovery / standby mode.", "SELECT pg_is_in_recovery();", pg("functions-admin.html"));
  f!("pg_promote", "pg_promote(wait boolean, wait_seconds int) -> boolean", "Promote a standby to primary.", "SELECT pg_promote();", pg("functions-admin.html"));
  f!("pg_postmaster_start_time", "pg_postmaster_start_time() -> timestamptz", "When the server started.", "SELECT pg_postmaster_start_time();", pg("functions-info.html"));
  f!("pg_conf_load_time", "pg_conf_load_time() -> timestamptz", "When the server last reloaded postgresql.conf.", "SELECT pg_conf_load_time();", pg("functions-info.html"));
  f!("pg_reload_conf", "pg_reload_conf() -> boolean", "Send SIGHUP so the server reloads its config.", "SELECT pg_reload_conf();", pg("functions-admin.html"));
  f!("pg_rotate_logfile", "pg_rotate_logfile() -> boolean", "Rotate the server's log file.", "SELECT pg_rotate_logfile();", pg("functions-admin.html"));
  f!("pg_get_keywords", "pg_get_keywords() -> setof (word, catcode, catdesc)", "All known PG keywords + their category.", "SELECT * FROM pg_get_keywords();", pg("functions-info.html"));

  // ---- round 150 replication + WAL ----
  f!("pg_create_logical_replication_slot", "pg_create_logical_replication_slot(slot_name name, plugin name [, temporary boolean] [, twophase boolean]) -> (slot_name, lsn)", "Create a logical replication slot bound to a plugin.", "SELECT pg_create_logical_replication_slot('s1', 'pgoutput');", pg("functions-admin.html"));
  f!("pg_create_physical_replication_slot", "pg_create_physical_replication_slot(slot_name name [, immediately_reserve boolean] [, temporary boolean]) -> (slot_name, lsn)", "Create a physical replication slot.", "SELECT pg_create_physical_replication_slot('standby1');", pg("functions-admin.html"));
  f!("pg_drop_replication_slot", "pg_drop_replication_slot(slot_name name) -> void", "Drop a replication slot.", "SELECT pg_drop_replication_slot('s1');", pg("functions-admin.html"));
  f!("pg_replication_slot_advance", "pg_replication_slot_advance(slot_name name, upto_lsn pg_lsn) -> (slot_name, end_lsn)", "Advance a slot's confirmed_flush_lsn.", "SELECT pg_replication_slot_advance('s1', '0/0');", pg("functions-admin.html"));
  f!("pg_logical_slot_get_changes", "pg_logical_slot_get_changes(slot_name name, upto_lsn pg_lsn, upto_nchanges int, VARIADIC options text[]) -> setof (lsn, xid, data)", "Stream decoded changes from a slot.", "SELECT * FROM pg_logical_slot_get_changes('s1', NULL, NULL);", pg("functions-admin.html"));
  f!("pg_logical_emit_message", "pg_logical_emit_message(transactional boolean, prefix text, content text) -> pg_lsn", "Inject a custom message into the WAL stream.", "SELECT pg_logical_emit_message(true, 'app', 'payload');", pg("functions-admin.html"));
  f!("pg_wal_lsn_diff", "pg_wal_lsn_diff(lsn1 pg_lsn, lsn2 pg_lsn) -> numeric", "Bytes between two WAL positions.", "SELECT pg_wal_lsn_diff('0/2', '0/0');", pg("functions-admin.html"));
  f!("pg_current_wal_lsn", "pg_current_wal_lsn() -> pg_lsn", "Current WAL write position (primary only).", "SELECT pg_current_wal_lsn();", pg("functions-admin.html"));
  f!("pg_current_wal_insert_lsn", "pg_current_wal_insert_lsn() -> pg_lsn", "Current WAL insertion position.", "SELECT pg_current_wal_insert_lsn();", pg("functions-admin.html"));
  f!("pg_current_wal_flush_lsn", "pg_current_wal_flush_lsn() -> pg_lsn", "Current WAL flush position.", "SELECT pg_current_wal_flush_lsn();", pg("functions-admin.html"));
  f!("pg_last_wal_receive_lsn", "pg_last_wal_receive_lsn() -> pg_lsn", "Standby: last WAL position received.", "SELECT pg_last_wal_receive_lsn();", pg("functions-admin.html"));
  f!("pg_last_wal_replay_lsn", "pg_last_wal_replay_lsn() -> pg_lsn", "Standby: last WAL position replayed.", "SELECT pg_last_wal_replay_lsn();", pg("functions-admin.html"));
  f!("pg_switch_wal", "pg_switch_wal() -> pg_lsn", "Force a WAL switch -- start a new segment.", "SELECT pg_switch_wal();", pg("functions-admin.html"));
  f!("pg_backup_start", "pg_backup_start(label text, fast boolean) -> pg_lsn", "PG15+: start a non-exclusive base backup.", "SELECT pg_backup_start('weekly', false);", pg("functions-admin.html"));
  f!("pg_backup_stop", "pg_backup_stop(wait_for_archive boolean) -> (lsn, labelfile, spcmapfile)", "PG15+: stop a base backup, return backup info.", "SELECT * FROM pg_backup_stop(true);", pg("functions-admin.html"));
  f!("pg_walfile_name", "pg_walfile_name(lsn pg_lsn) -> text", "WAL segment file name containing this LSN.", "SELECT pg_walfile_name(pg_current_wal_lsn());", pg("functions-admin.html"));
  f!("pg_walfile_name_offset", "pg_walfile_name_offset(lsn pg_lsn) -> (file_name, file_offset)", "WAL segment file name + byte offset for an LSN.", "SELECT * FROM pg_walfile_name_offset(pg_current_wal_lsn());", pg("functions-admin.html"));

  // ---- round 151 misc gaps ----
  f!("string_to_table", "string_to_table(text, delim text [, null_string text]) -> setof text", "Like string_to_array but returns a row set.", "SELECT * FROM string_to_table('a,b,c', ',');", pg("functions-string.html"));
  f!("jsonb_set_lax", "jsonb_set_lax(jsonb, path text[], new_value jsonb [, create_missing boolean] [, null_value_treatment text]) -> jsonb", "Like jsonb_set but adds 'null_value_treatment' option (PG13+).", "SELECT jsonb_set_lax(data, '{x}', 'null'::jsonb, true, 'delete_key');", pg("functions-json.html"));

  // ---- round 152 missing system/privilege fns ----
  f!("current_role", "current_role -> name", "Same as current_user.", "SELECT current_role;", pg("functions-info.html"));
  f!("current_query", "current_query() -> text", "Text of the currently executing query.", "SELECT current_query();", pg("functions-info.html"));
  f!("current_catalog", "current_catalog -> name", "SQL standard alias for current_database.", "SELECT current_catalog;", pg("functions-info.html"));
  f!("pg_current_logfile", "pg_current_logfile([format text]) -> text", "Path of the current server log file.", "SELECT pg_current_logfile();", pg("functions-info.html"));
  f!("inet_client_port", "inet_client_port() -> int", "Client port; NULL when over Unix socket.", "SELECT inet_client_port();", pg("functions-info.html"));
  f!("inet_server_port", "inet_server_port() -> int", "Server port the connection landed on.", "SELECT inet_server_port();", pg("functions-info.html"));
  f!("has_table_privilege", "has_table_privilege([user,] table, privilege text) -> boolean", "Does the role have the privilege on the table?", "SELECT has_table_privilege('alice', 'users', 'SELECT');", pg("functions-info.html"));
  f!("has_schema_privilege", "has_schema_privilege([user,] schema, privilege text) -> boolean", "Schema-level privilege check.", "SELECT has_schema_privilege('public', 'USAGE');", pg("functions-info.html"));
  f!("has_database_privilege", "has_database_privilege([user,] database, privilege text) -> boolean", "Database-level privilege check.", "SELECT has_database_privilege(current_user, current_database(), 'CONNECT');", pg("functions-info.html"));
  f!("has_column_privilege", "has_column_privilege([user,] table, column, privilege text) -> boolean", "Column-level privilege check.", "SELECT has_column_privilege('alice', 'users', 'email', 'UPDATE');", pg("functions-info.html"));
  f!("has_function_privilege", "has_function_privilege([user,] function, privilege text) -> boolean", "Function execution privilege.", "SELECT has_function_privilege('alice', 'now()', 'EXECUTE');", pg("functions-info.html"));
  f!("has_any_column_privilege", "has_any_column_privilege([user,] table, privilege text) -> boolean", "Does the role have privilege on ANY column?", "SELECT has_any_column_privilege('alice', 'users', 'SELECT');", pg("functions-info.html"));
  f!("has_sequence_privilege", "has_sequence_privilege([user,] sequence, privilege text) -> boolean", "Sequence privilege check.", "SELECT has_sequence_privilege('s', 'USAGE');", pg("functions-info.html"));
  f!("has_tablespace_privilege", "has_tablespace_privilege([user,] tablespace, privilege text) -> boolean", "Tablespace privilege check.", "SELECT has_tablespace_privilege('pg_default', 'CREATE');", pg("functions-info.html"));
  f!("has_foreign_data_wrapper_privilege", "has_foreign_data_wrapper_privilege([user,] fdw, privilege text) -> boolean", "FDW privilege check.", "SELECT has_foreign_data_wrapper_privilege('postgres_fdw', 'USAGE');", pg("functions-info.html"));
  f!("has_language_privilege", "has_language_privilege([user,] lang, privilege text) -> boolean", "Procedural language privilege.", "SELECT has_language_privilege('plpgsql', 'USAGE');", pg("functions-info.html"));
  f!("has_server_privilege", "has_server_privilege([user,] server, privilege text) -> boolean", "Foreign server privilege check.", "SELECT has_server_privilege('srv1', 'USAGE');", pg("functions-info.html"));
  f!("has_type_privilege", "has_type_privilege([user,] type, privilege text) -> boolean", "Type privilege check.", "SELECT has_type_privilege('point', 'USAGE');", pg("functions-info.html"));
  f!("pg_has_role", "pg_has_role([user,] role, privilege text) -> boolean", "Is `user` a member of `role` with the requested membership?", "SELECT pg_has_role('alice', 'admins', 'MEMBER');", pg("functions-info.html"));
  f!("row_security_active", "row_security_active(table) -> boolean", "True when RLS would be enforced on the table for the current role.", "SELECT row_security_active('users');", pg("functions-info.html"));

  // ---- round 153 pg_catalog fns ----
  f!("pg_get_partition_constraintdef", "pg_get_partition_constraintdef(regclass) -> text", "Reconstructs the implicit CHECK constraint that defines a partition.", "SELECT pg_get_partition_constraintdef('orders_2024');", pg("functions-info.html"));
  f!("pg_get_partkeydef", "pg_get_partkeydef(regclass) -> text", "Definition of the PARTITION BY key for a partitioned table.", "SELECT pg_get_partkeydef('orders');", pg("functions-info.html"));
  f!("pg_get_publication_tables", "pg_get_publication_tables(pubname text) -> setof oid", "OIDs of tables included in a publication.", "SELECT * FROM pg_get_publication_tables('pub1');", pg("functions-info.html"));
  f!("pg_partition_root", "pg_partition_root(regclass) -> regclass", "Top-level partitioned table for a (possibly nested) partition.", "SELECT pg_partition_root('orders_2024');", pg("functions-info.html"));
  f!("pg_partition_ancestors", "pg_partition_ancestors(regclass) -> setof regclass", "Ancestor chain of a partition up to the root.", "SELECT * FROM pg_partition_ancestors('orders_2024');", pg("functions-info.html"));
  f!("pg_partition_tree", "pg_partition_tree(regclass) -> setof (relid, parentrelid, isleaf, level)", "Walk the entire partition tree rooted at a relation.", "SELECT * FROM pg_partition_tree('orders');", pg("functions-info.html"));
  f!("pg_relation_is_publishable", "pg_relation_is_publishable(regclass) -> boolean", "True if the relation can be added to a publication.", "SELECT pg_relation_is_publishable('users');", pg("functions-info.html"));
  f!("pg_get_replica_identity_index", "pg_get_replica_identity_index(regclass) -> regclass", "Index used as REPLICA IDENTITY for the table, or NULL.", "SELECT pg_get_replica_identity_index('users');", pg("functions-info.html"));
  f!("pg_get_object_address", "pg_get_object_address(type text, name text[], args text[]) -> (classid, objid, objsubid)", "Canonical address record for an object.", "SELECT * FROM pg_get_object_address('table', '{public,users}', '{}');", pg("functions-info.html"));
  f!("pg_identify_object", "pg_identify_object(classid oid, objid oid, objsubid int) -> (type, schema, name, identity)", "Identify an object by its catalog address.", "SELECT * FROM pg_identify_object('pg_class'::regclass, 'users'::regclass, 0);", pg("functions-info.html"));
  f!("pg_identify_object_as_address", "pg_identify_object_as_address(classid oid, objid oid, objsubid int) -> (type text, object_names text[], object_args text[])", "Identify an object as a usable address.", "SELECT * FROM pg_identify_object_as_address('pg_class'::regclass, 'users'::regclass, 0);", pg("functions-info.html"));
  f!("pg_describe_object", "pg_describe_object(classid oid, objid oid, objsubid int) -> text", "Human-readable description of an object.", "SELECT pg_describe_object('pg_class'::regclass, 'users'::regclass, 0);", pg("functions-info.html"));
  f!("pg_locks", "pg_locks -> view", "View exposing every active lock; useful for blocking diagnostics.", "SELECT * FROM pg_locks WHERE granted = false;", pg("view-pg-locks.html"));
  f!("pg_stat_get_backend_idset", "pg_stat_get_backend_idset() -> setof int", "Internal: enumerate backend ids for pg_stat_get_backend_*.", "SELECT pg_stat_get_backend_idset();", pg("monitoring-stats.html"));
  f!("pg_get_ruledef", "pg_get_ruledef(rule_oid oid [, pretty boolean]) -> text", "Reconstruct a CREATE RULE statement.", "SELECT pg_get_ruledef(oid) FROM pg_rewrite;", pg("functions-info.html"));
  f!("pg_get_function_identity_arguments", "pg_get_function_identity_arguments(funcid oid) -> text", "Function arg signature used in DROP/COMMENT/etc statements.", "SELECT pg_get_function_identity_arguments(oid) FROM pg_proc;", pg("functions-info.html"));

  // ---- round 160 string fns ----
  f!("regexp_instr", "regexp_instr(string text, pattern text [, position int] [, occurrence int] [, return_opt int] [, flags text]) -> int", "Position of a regexp match (PG15+).", "SELECT regexp_instr('foo123bar', '[0-9]+');", pg("functions-matching.html"));
  f!("regexp_like", "regexp_like(string text, pattern text [, flags text]) -> boolean", "Boolean regexp match (PG15+).", "WHERE regexp_like(email, '^.+@.+\\..+$', 'i')", pg("functions-matching.html"));
  f!("similar_to_escape", "similar_to_escape(pattern text [, escape text]) -> text", "Convert a SIMILAR TO pattern to a regular expression.", "SELECT similar_to_escape('he%lo', '#');", pg("functions-matching.html"));
  f!("unistr", "unistr(text [, escape text]) -> text", "Decode Unicode escape sequences in a string (PG16+).", "SELECT unistr('\\u0041\\u0042');", pg("functions-string.html"));

  // ---- round 161 datetime fns ----
  f!("date_add", "date_add(timestamptz, interval [, timezone text]) -> timestamptz", "Add an interval to a timestamptz with zone-aware semantics (PG16+).", "SELECT date_add(now(), interval '1 month', 'UTC');", pg("functions-datetime.html"));
  f!("date_subtract", "date_subtract(timestamptz, interval [, timezone text]) -> timestamptz", "Subtract an interval from a timestamptz with zone-aware semantics (PG16+).", "SELECT date_subtract(now(), interval '1 month', 'UTC');", pg("functions-datetime.html"));
  f!("interval_eq", "interval_eq(a interval, b interval) -> boolean", "Internal comparison: a = b. Use the `=` operator instead in user code.", "SELECT interval_eq('1 day', '24 hours');", pg("functions-datetime.html"));

  // ---- round 162 json record fns ----
  f!("jsonb_to_record", "jsonb_to_record(jsonb) -> record", "Cast a jsonb object into a record. Used in `AS (col type, ...)` syntax.", "SELECT * FROM jsonb_to_record(payload) AS x(id int, name text);", pg("functions-json.html"));
  f!("json_to_record", "json_to_record(json) -> record", "Cast a json object into a record. Used in `AS (col type, ...)` syntax.", "SELECT * FROM json_to_record(my_json) AS x(id int);", pg("functions-json.html"));
  f!("jsonb_to_recordset", "jsonb_to_recordset(jsonb) -> setof record", "Cast a jsonb array of objects into a row set.", "SELECT * FROM jsonb_to_recordset(payload) AS x(id int, name text);", pg("functions-json.html"));
  f!("json_to_recordset", "json_to_recordset(json) -> setof record", "Cast a json array of objects into a row set.", "SELECT * FROM json_to_recordset(my_json) AS x(id int);", pg("functions-json.html"));
  f!("jsonb_populate_record", "jsonb_populate_record(base anyelement, jsonb) -> anyelement", "Cast jsonb into a row type; base is a row prototype.", "SELECT (jsonb_populate_record(NULL::users, payload)).*;", pg("functions-json.html"));
  f!("json_populate_record", "json_populate_record(base anyelement, json) -> anyelement", "Cast json into a row type; base is a row prototype.", "SELECT (json_populate_record(NULL::users, my_json)).*;", pg("functions-json.html"));
  f!("jsonb_populate_recordset", "jsonb_populate_recordset(base anyelement, jsonb) -> setof anyelement", "Cast jsonb array of objects into a row set of `base`.", "SELECT * FROM jsonb_populate_recordset(NULL::users, payload);", pg("functions-json.html"));
  f!("jsonb_concat", "jsonb_concat(jsonb, jsonb) -> jsonb", "Internal name for the `||` operator on jsonb. Prefer the operator.", "SELECT a || b;", pg("functions-json.html"));
  f!("jsonb_delete_path", "jsonb_delete_path(jsonb, path text[]) -> jsonb", "Internal name for the `#-` operator. Prefer the operator.", "SELECT data #- '{addr,city}'::text[];", pg("functions-json.html"));

  // ---- round 163 json operator fns + xml ----
  f!("json_array_element", "json_array_element(json, int) -> json", "Internal name for `json -> int`. Use the operator.", "SELECT my_json -> 0;", pg("functions-json.html"));
  f!("json_array_element_text", "json_array_element_text(json, int) -> text", "Internal name for `json ->> int`. Use the operator.", "SELECT my_json ->> 0;", pg("functions-json.html"));
  f!("jsonb_array_element", "jsonb_array_element(jsonb, int) -> jsonb", "Internal name for `jsonb -> int`. Use the operator.", "SELECT data -> 0;", pg("functions-json.html"));
  f!("jsonb_array_element_text", "jsonb_array_element_text(jsonb, int) -> text", "Internal name for `jsonb ->> int`. Use the operator.", "SELECT data ->> 0;", pg("functions-json.html"));
  f!("json_object_field", "json_object_field(json, text) -> json", "Internal name for `json -> text`. Use the operator.", "SELECT my_json -> 'addr';", pg("functions-json.html"));
  f!("json_object_field_text", "json_object_field_text(json, text) -> text", "Internal name for `json ->> text`. Use the operator.", "SELECT my_json ->> 'name';", pg("functions-json.html"));
  f!("jsonb_object_field", "jsonb_object_field(jsonb, text) -> jsonb", "Internal name for `jsonb -> text`. Use the operator.", "SELECT data -> 'addr';", pg("functions-json.html"));
  f!("jsonb_object_field_text", "jsonb_object_field_text(jsonb, text) -> text", "Internal name for `jsonb ->> text`. Use the operator.", "SELECT data ->> 'name';", pg("functions-json.html"));
  f!("query_to_xml", "query_to_xml(query text, nulls boolean, tableforest boolean, targetns text) -> xml", "Map a query result into XML.", "SELECT query_to_xml('SELECT 1 AS a', true, false, '');", pg("functions-xml.html"));
  f!("schema_to_xml", "schema_to_xml(schema name, nulls boolean, tableforest boolean, targetns text) -> xml", "Map every table in a schema into XML.", "SELECT schema_to_xml('public', true, false, '');", pg("functions-xml.html"));
  f!("table_to_xml", "table_to_xml(tbl regclass, nulls boolean, tableforest boolean, targetns text) -> xml", "Map a table into XML.", "SELECT table_to_xml('users', true, false, '');", pg("functions-xml.html"));
  f!("cursor_to_xml", "cursor_to_xml(cursor refcursor, count int, nulls boolean, tableforest boolean, targetns text) -> xml", "Drain a cursor into XML.", "SELECT cursor_to_xml('c1', 100, true, false, '');", pg("functions-xml.html"));
  f!("database_to_xml", "database_to_xml(nulls boolean, tableforest boolean, targetns text) -> xml", "Whole-database XML dump.", "SELECT database_to_xml(true, false, '');", pg("functions-xml.html"));
  f!("xpath", "xpath(xpath text, xml xml [, nsarray text[]]) -> xml[]", "Apply an XPath expression to an XML document.", "SELECT xpath('//book/title', payload);", pg("functions-xml.html"));
  f!("xpath_exists", "xpath_exists(xpath text, xml xml [, nsarray text[]]) -> boolean", "True when XPath matches anything in the document.", "WHERE xpath_exists('//flag', payload)", pg("functions-xml.html"));
  f!("xmltable", "XMLTABLE(<xpath> PASSING <doc> COLUMNS (...) ) -> setof record", "Project XML into rows (SQL/XML).", "SELECT * FROM XMLTABLE('/r/it' PASSING xmldoc COLUMNS id int PATH '@id');", pg("functions-xml.html"));
  f!("xmlcomment", "xmlcomment(text) -> xml", "Wrap text in <!-- ... -->.", "SELECT xmlcomment('section');", pg("functions-xml.html"));
  f!("xmlconcat", "xmlconcat(xml[]) -> xml", "Concatenate XML fragments.", "SELECT xmlconcat(a, b);", pg("functions-xml.html"));
  f!("xmlelement", "XMLELEMENT(NAME name [, XMLATTRIBUTES (...)], <content>) -> xml", "Build an XML element from SQL.", "SELECT xmlelement(name 'r', xmlattributes('1' as id), 'body');", pg("functions-xml.html"));
  f!("xmlexists", "xmlexists(xpath text PASSING xml [, ns]) -> boolean", "XPath existence test.", "WHERE xmlexists('/x' PASSING xmldoc)", pg("functions-xml.html"));
  f!("xmlforest", "xmlforest(<expr> AS <name>[, ...]) -> xml", "Build a sequence of XML elements.", "SELECT xmlforest(id AS pk, name);", pg("functions-xml.html"));
  f!("xmlroot", "xmlroot(xml, VERSION ver text [, STANDALONE yn]) -> xml", "Replace or set the XML root declaration.", "SELECT xmlroot(payload, VERSION '1.0', STANDALONE YES);", pg("functions-xml.html"));
  f!("xmlserialize", "XMLSERIALIZE({DOCUMENT|CONTENT} <xml> AS <text_type>) -> text", "Serialise XML into TEXT/VARCHAR/CHARACTER.", "SELECT xmlserialize(document payload as text);", pg("functions-xml.html"));
  f!("xmlpi", "xmlpi(NAME name [, content text]) -> xml", "Build a processing instruction.", "SELECT xmlpi(name 'xml-stylesheet', 'href=...');", pg("functions-xml.html"));
  f!("xmltext", "xmltext(text) -> xml", "Wrap a text value as XML text node.", "SELECT xmltext('hello');", pg("functions-xml.html"));

  // ---- round 164 range comparison + fts fns ----
  f!("range_contains", "range_contains(anyrange, anyrange) -> boolean", "Internal name for the `@>` operator on ranges.", "SELECT '[1,10)'::int4range @> '[3,5)'::int4range;", pg("rangetypes.html"));
  f!("range_overlaps", "range_overlaps(anyrange, anyrange) -> boolean", "Internal name for the `&&` operator on ranges.", "SELECT '[1,10)'::int4range && '[5,15)'::int4range;", pg("rangetypes.html"));
  f!("range_eq", "range_eq(anyrange, anyrange) -> boolean", "Internal name for `=` on ranges.", "SELECT a = b FROM t;", pg("rangetypes.html"));
  f!("range_lt", "range_lt(anyrange, anyrange) -> boolean", "Internal name for `<` on ranges.", "SELECT a < b FROM t;", pg("rangetypes.html"));
  f!("range_le", "range_le(anyrange, anyrange) -> boolean", "Internal name for `<=` on ranges.", "SELECT a <= b FROM t;", pg("rangetypes.html"));
  f!("range_gt", "range_gt(anyrange, anyrange) -> boolean", "Internal name for `>` on ranges.", "SELECT a > b FROM t;", pg("rangetypes.html"));
  f!("range_ge", "range_ge(anyrange, anyrange) -> boolean", "Internal name for `>=` on ranges.", "SELECT a >= b FROM t;", pg("rangetypes.html"));
  f!("range_after", "range_after(anyrange, anyrange) -> boolean", "Internal name for the `>>` operator -- strictly after.", "SELECT a >> b FROM t;", pg("rangetypes.html"));
  f!("range_before", "range_before(anyrange, anyrange) -> boolean", "Internal name for the `<<` operator -- strictly before.", "SELECT a << b FROM t;", pg("rangetypes.html"));
  f!("range_adjacent", "range_adjacent(anyrange, anyrange) -> boolean", "Internal name for the `-|-` operator -- adjacent ranges.", "SELECT a -|- b FROM t;", pg("rangetypes.html"));
  f!("unnest_multirange", "unnest(anymultirange) -> setof anyrange", "Expand a multirange into its constituent ranges. Alias of `unnest`.", "SELECT * FROM unnest('{[1,5), [10,15)}'::int4multirange);", pg("rangetypes.html"));
  f!("ts_filter", "ts_filter(tsvector, char[]) -> tsvector", "Keep only lexemes with the given weights.", "SELECT ts_filter(to_tsvector('a b c'), '{A,B}');", pg("textsearch-functions.html"));
  f!("ts_delete", "ts_delete(tsvector, text|text[]) -> tsvector", "Remove lexemes from a tsvector.", "SELECT ts_delete(vec, 'noise');", pg("textsearch-functions.html"));
  f!("ts_rewrite", "ts_rewrite(query tsquery, target tsquery, substitute tsquery) -> tsquery", "Rewrite occurrences of `target` in `query` with `substitute`.", "SELECT ts_rewrite('cat'::tsquery, 'cat'::tsquery, 'feline'::tsquery);", pg("textsearch-functions.html"));

  // ---- round 165 geometric fns ----
  f!("area", "area(box|polygon|circle) -> double precision", "Area of a geometric object.", "SELECT area(box '(0,0),(1,1)');", pg("functions-geometry.html"));
  f!("diameter", "diameter(circle) -> double precision", "Diameter of a circle.", "SELECT diameter(circle '<(0,0),5>');", pg("functions-geometry.html"));
  f!("height", "height(box) -> double precision", "Vertical span of a box.", "SELECT height(box '(0,0),(3,4)');", pg("functions-geometry.html"));
  f!("width", "width(box) -> double precision", "Horizontal span of a box.", "SELECT width(box '(0,0),(3,4)');", pg("functions-geometry.html"));
  f!("slope", "slope(point, point) -> double precision", "Slope of the line through two points.", "SELECT slope(point '(0,0)', point '(1,1)');", pg("functions-geometry.html"));
  f!("center", "center(box|circle) -> point", "Centroid of a geometric object.", "SELECT center(box '(0,0),(2,2)');", pg("functions-geometry.html"));
  f!("distance", "<obj> <-> <obj> -- distance operator alias.", "Distance between two geometric objects.", "SELECT point '(0,0)' <-> point '(3,4)';", pg("functions-geometry.html"));
  f!("isclosed", "isclosed(path) -> boolean", "True when the path is closed.", "SELECT isclosed(path '((0,0),(1,1))');", pg("functions-geometry.html"));
  f!("isopen", "isopen(path) -> boolean", "True when the path is open.", "SELECT isopen(path '[(0,0),(1,1)]');", pg("functions-geometry.html"));
  f!("npoints", "npoints(path|polygon) -> int", "Number of vertices in a path or polygon.", "SELECT npoints(polygon '((0,0),(1,0),(1,1))');", pg("functions-geometry.html"));
  f!("pclose", "pclose(path) -> path", "Convert a path to a closed path.", "SELECT pclose(path '[(0,0),(1,1)]');", pg("functions-geometry.html"));
  f!("popen", "popen(path) -> path", "Convert a path to an open path.", "SELECT popen(path '((0,0),(1,1))');", pg("functions-geometry.html"));
  f!("radius", "radius(circle) -> double precision", "Radius of a circle.", "SELECT radius(circle '<(0,0),5>');", pg("functions-geometry.html"));

  // ---- cycle2 round 4: missing core temporal + formatting + reg* + jsonb_set + size fns ----
  f!("age", "age(timestamp [, timestamp]) -> interval", "Time elapsed between two timestamps (defaults to now()::date if second arg omitted).", "SELECT age(now(), birth) FROM users;", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  f!("date_part", "date_part(text, timestamp|interval) -> double precision", "Extract sub-field. SQL-standard form: `EXTRACT(<field> FROM ...)`.", "SELECT date_part('year', now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  f!("date_trunc", "date_trunc(text, timestamp [, text]) -> timestamp", "Round timestamp down to a unit (hour/day/week/month/year/...). Optional third arg = time zone.", "SELECT date_trunc('day', now()), date_trunc('day', now(), 'UTC');", pg("functions-datetime.html#FUNCTIONS-DATETIME-TRUNC"));
  f!("to_char", "to_char(value, format) -> text", "Format a number/date/timestamp/interval to text using a pattern (`YYYY-MM-DD`, `FM999.99`, etc).", "SELECT to_char(now(), 'YYYY-MM-DD HH24:MI:SS');", pg("functions-formatting.html"));
  f!("to_date", "to_date(text, text) -> date", "Parse text to date using a pattern.", "SELECT to_date('2026-05-30', 'YYYY-MM-DD');", pg("functions-formatting.html"));
  f!("to_number", "to_number(text, text) -> numeric", "Parse text to numeric using a pattern.", "SELECT to_number('$1,234.50', 'L9,999.99');", pg("functions-formatting.html"));
  f!("to_timestamp", "to_timestamp(text, text) -> timestamptz | to_timestamp(double) -> timestamptz", "Two forms: pattern-parse text, or convert Unix epoch seconds to timestamptz.", "SELECT to_timestamp('2026-05-30','YYYY-MM-DD'), to_timestamp(1717000000);", pg("functions-formatting.html"));
  f!("justify_days", "justify_days(interval) -> interval", "Roll 30-day blocks into months.", "SELECT justify_days(interval '90 days');", pg("functions-datetime.html"));
  f!("justify_hours", "justify_hours(interval) -> interval", "Roll 24-hour blocks into days.", "SELECT justify_hours(interval '49 hours');", pg("functions-datetime.html"));
  f!("justify_interval", "justify_interval(interval) -> interval", "Apply both day and hour justifications.", "SELECT justify_interval(interval '1 month 100 days 2 hours');", pg("functions-datetime.html"));
  f!("make_date", "make_date(year int, month int, day int) -> date", "Build a date from components.", "SELECT make_date(2026, 5, 30);", pg("functions-datetime.html"));
  f!("make_time", "make_time(hour int, min int, sec double precision) -> time", "Build a time-of-day from components.", "SELECT make_time(14, 30, 0);", pg("functions-datetime.html"));
  f!("make_timestamp", "make_timestamp(year, month, day, hour, min, sec) -> timestamp", "Build a timestamp (no zone) from components.", "SELECT make_timestamp(2026, 5, 30, 14, 30, 0);", pg("functions-datetime.html"));
  f!("make_timestamptz", "make_timestamptz(year, month, day, hour, min, sec [, tz]) -> timestamptz", "Build a timestamptz from components; optional time zone.", "SELECT make_timestamptz(2026, 5, 30, 14, 30, 0, 'UTC');", pg("functions-datetime.html"));
  f!("make_interval", "make_interval([years=][, months=][, weeks=][, days=][, hours=][, mins=][, secs=]) -> interval", "Build an interval from named components.", "SELECT make_interval(days => 7, hours => 12);", pg("functions-datetime.html"));
  f!("to_regclass", "to_regclass(text) -> regclass", "Look up table OID by name. Returns NULL when not found (vs `::regclass` which errors).", "SELECT to_regclass('public.users') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("to_regtype", "to_regtype(text) -> regtype", "Look up type OID by name; NULL when missing.", "SELECT to_regtype('int4') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("to_regproc", "to_regproc(text) -> regproc", "Look up function OID by simple name; NULL when missing.", "SELECT to_regproc('now') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("to_regprocedure", "to_regprocedure(text) -> regprocedure", "Look up function OID by signature `name(arg_types)`; NULL when missing.", "SELECT to_regprocedure('pg_size_pretty(bigint)') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("jsonb_set", "jsonb_set(target jsonb, path text[], new_value jsonb [, create_missing boolean]) -> jsonb", "Set or replace a nested value at `path`. `create_missing` (default true) inserts missing keys.", "SELECT jsonb_set('{\"a\":1}'::jsonb, '{b,c}', '42'::jsonb, true);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("jsonb_strip_nulls", "jsonb_strip_nulls(jsonb) -> jsonb", "Recursively remove all keys with NULL values.", "SELECT jsonb_strip_nulls('{\"a\":1,\"b\":null}'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("jsonb_pretty", "jsonb_pretty(jsonb) -> text", "Pretty-print jsonb with indentation.", "SELECT jsonb_pretty('{\"a\":[1,2]}'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("pg_size_pretty", "pg_size_pretty(bigint) -> text", "Format a byte count as human-readable string (kB / MB / GB / TB).", "SELECT pg_size_pretty(pg_database_size(current_database()));", pg("functions-admin.html#FUNCTIONS-ADMIN-DBSIZE"));
  f!("pg_database_size", "pg_database_size(name|oid) -> bigint", "Total disk usage of a database in bytes.", "SELECT pg_database_size(current_database());", pg("functions-admin.html#FUNCTIONS-ADMIN-DBSIZE"));
  f!("pg_table_size", "pg_table_size(regclass) -> bigint", "Bytes used by table's main fork + FSM/VM (excludes indexes/TOAST).", "SELECT pg_table_size('users');", pg("functions-admin.html#FUNCTIONS-ADMIN-DBSIZE"));
  f!("pg_indexes_size", "pg_indexes_size(regclass) -> bigint", "Total bytes used by all indexes on a table.", "SELECT pg_indexes_size('users');", pg("functions-admin.html#FUNCTIONS-ADMIN-DBSIZE"));
  f!("pg_relation_size", "pg_relation_size(regclass [, text]) -> bigint", "Bytes of a specific fork (main/fsm/vm/init); default main.", "SELECT pg_relation_size('users', 'main');", pg("functions-admin.html#FUNCTIONS-ADMIN-DBSIZE"));
  f!("pg_total_relation_size", "pg_total_relation_size(regclass) -> bigint", "Table + TOAST + every index, in bytes.", "SELECT pg_total_relation_size('users');", pg("functions-admin.html#FUNCTIONS-ADMIN-DBSIZE"));

  // ---- cycle2 round 5: basic string fns ----
  f!("lower", "lower(text) -> text", "Lowercase text using collation rules.", "SELECT lower('HELLO');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("upper", "upper(text) -> text", "Uppercase text using collation rules.", "SELECT upper('hello');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("initcap", "initcap(text) -> text", "Capitalize the first letter of each word.", "SELECT initcap('hello world');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("left", "left(text, int) -> text", "First n characters; negative n drops the last |n| chars.", "SELECT left('postgres', 4);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("right", "right(text, int) -> text", "Last n characters; negative n drops the first |n| chars.", "SELECT right('postgres', 3);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("substr", "substr(text, from int [, count int]) -> text", "PG alias of `SUBSTRING`. 1-based.", "SELECT substr('postgres', 5, 3);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("trim", "trim([LEADING|TRAILING|BOTH] [chars] FROM text) -> text", "Strip leading/trailing chars (default whitespace).", "SELECT trim(BOTH '\"' FROM '\"hi\"');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("translate", "translate(text, from text, to text) -> text", "Per-char map: replace each char in `from` with the same-index char in `to`; chars without a mapping are deleted.", "SELECT translate('12345', '143', 'ax');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("replace", "replace(text, from text, to text) -> text", "Replace every literal occurrence of `from` with `to`.", "SELECT replace('foo bar foo', 'foo', 'baz');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("overlay", "overlay(text PLACING text FROM int [FOR int]) -> text", "Substring replacement starting at 1-based position.", "SELECT overlay('Postgres' PLACING 'SQL' FROM 5);", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("position", "position(needle IN haystack) -> int", "1-based index of needle in haystack; 0 when not found.", "SELECT position('@' IN 'a@b.com');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("octet_length", "octet_length(text|bytea) -> int", "Length in bytes (not characters).", "SELECT octet_length('café');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("bit_length", "bit_length(text|bytea) -> int", "Length in bits.", "SELECT bit_length('a');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("char_length", "char_length(text) -> int", "Length in characters. Alias `character_length`, `length`.", "SELECT char_length('café');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("character_length", "character_length(text) -> int", "SQL-standard alias of `char_length`.", "SELECT character_length('café');", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("length", "length(text|bytea) -> int", "Characters for text; bytes for bytea.", "SELECT length('café'), length('café'::bytea);", pg("functions-string.html#FUNCTIONS-STRING-SQL"));
  f!("encode", "encode(bytea, format text) -> text", "Encode bytes as text: 'base64', 'hex', 'escape'.", "SELECT encode(sha256('x'::bytea), 'hex');", pg("functions-binarystring.html"));
  f!("decode", "decode(text, format text) -> bytea", "Decode text into bytes using a format ('base64', 'hex', 'escape').", "SELECT decode('48656c6c6f', 'hex');", pg("functions-binarystring.html"));
  f!("ascii", "ascii(text) -> int", "Unicode codepoint of the first character.", "SELECT ascii('A');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("lpad", "lpad(text, length int [, fill text]) -> text", "Left-pad to length with `fill` (default space); truncates when text longer than length.", "SELECT lpad('42', 5, '0');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("rpad", "rpad(text, length int [, fill text]) -> text", "Right-pad to length with `fill` (default space).", "SELECT rpad('x', 5, '-');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("btrim", "btrim(text [, chars text]) -> text", "Trim chars from BOTH sides (default whitespace).", "SELECT btrim(' hello ');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("ltrim", "ltrim(text [, chars text]) -> text", "Trim chars from LEADING side.", "SELECT ltrim('00042', '0');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("rtrim", "rtrim(text [, chars text]) -> text", "Trim chars from TRAILING side.", "SELECT rtrim('hello   ');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("repeat", "repeat(text, int) -> text", "Concatenate text n times.", "SELECT repeat('-', 30);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("reverse", "reverse(text) -> text", "Reverse characters (UTF-8 safe).", "SELECT reverse('abc');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("split_part", "split_part(text, delim text, field int) -> text", "Return the nth field after splitting by delim (1-based; negative counts from end since PG14).", "SELECT split_part('a,b,c', ',', 2);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("starts_with", "starts_with(text, prefix text) -> boolean", "True when text begins with prefix. PG14+; SQL-portable equivalent: `text LIKE prefix || '%'`.", "SELECT starts_with('postgres', 'post');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("concat", "concat(any [, ...]) -> text", "Concatenate args as text; NULL args become empty (unlike `||`).", "SELECT concat('a', NULL, 'b');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("concat_ws", "concat_ws(sep text, any [, ...]) -> text", "Concatenate with separator; NULLs skipped.", "SELECT concat_ws(',', 1, NULL, 'x');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("format", "format(fmt text, args any ...) -> text", "printf-style: %s, %I (identifier-quote), %L (literal-quote). Safer than ad-hoc concat for dynamic SQL.", "SELECT format('SELECT * FROM %I WHERE id = %L', 'users', 42);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("quote_ident", "quote_ident(text) -> text", "Double-quote text if needed to use as an SQL identifier. Use for safe dynamic SQL.", "SELECT quote_ident('users with spaces');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("quote_literal", "quote_literal(text) -> text", "Single-quote text into an SQL string literal. NULL in -> empty string; use `quote_nullable` to preserve NULL.", "SELECT quote_literal('O''Reilly');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("quote_nullable", "quote_nullable(any) -> text", "Like `quote_literal` but returns the unquoted text `NULL` for NULL input.", "SELECT quote_nullable(NULL::text);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  // ---- cycle2 round 5: array + array<->string fns ----
  f!("string_to_array", "string_to_array(text, delim text [, null_string text]) -> text[]", "Split text by delim into an array. Optional `null_string` makes matching tokens NULL.", "SELECT string_to_array('a,,c', ',', '');", pg("functions-array.html"));
  f!("array_to_string", "array_to_string(anyarray, delim text [, null_string text]) -> text", "Join array elements with delim; NULL elements use `null_string` (or are skipped if omitted).", "SELECT array_to_string(ARRAY[1,2,3], ',');", pg("functions-array.html"));
  f!("string_agg", "string_agg(text, delim text [ORDER BY ...]) -> text", "Aggregate: concat per-group strings with delim. Use ORDER BY for deterministic output.", "SELECT string_agg(name, ', ' ORDER BY name) FROM users GROUP BY org_id;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("array_agg", "array_agg(any [ORDER BY ...]) -> anyarray", "Aggregate: collect rows into an array.", "SELECT array_agg(id ORDER BY created_at) FROM events;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("array_append", "array_append(anyarray, any) -> anyarray", "Append an element. Same as `arr || elem`.", "SELECT array_append(ARRAY[1,2], 3);", pg("functions-array.html"));
  f!("array_prepend", "array_prepend(any, anyarray) -> anyarray", "Prepend an element. Same as `elem || arr`.", "SELECT array_prepend(0, ARRAY[1,2]);", pg("functions-array.html"));
  f!("array_cat", "array_cat(anyarray, anyarray) -> anyarray", "Concatenate two arrays. Same as `a || b`.", "SELECT array_cat(ARRAY[1,2], ARRAY[3,4]);", pg("functions-array.html"));
  f!("array_remove", "array_remove(anyarray, any) -> anyarray", "Remove every occurrence of a value from a 1-D array.", "SELECT array_remove(ARRAY[1,2,3,2], 2);", pg("functions-array.html"));
  f!("array_replace", "array_replace(anyarray, from any, to any) -> anyarray", "Replace every occurrence of `from` with `to`.", "SELECT array_replace(ARRAY[1,2,3], 2, 20);", pg("functions-array.html"));
  f!("array_position", "array_position(anyarray, any [, start int]) -> int", "1-based index of first occurrence; NULL when absent.", "SELECT array_position(ARRAY['a','b','c'], 'b');", pg("functions-array.html"));
  f!("array_positions", "array_positions(anyarray, any) -> int[]", "All 1-based indexes of a value.", "SELECT array_positions(ARRAY[1,2,3,2,1], 2);", pg("functions-array.html"));
  f!("array_length", "array_length(anyarray, dim int) -> int", "Length along dimension `dim`.", "SELECT array_length(ARRAY[1,2,3], 1);", pg("functions-array.html"));
  f!("array_lower", "array_lower(anyarray, dim int) -> int", "Lower bound (default 1) of dimension.", "SELECT array_lower(ARRAY[1,2,3], 1);", pg("functions-array.html"));
  f!("array_upper", "array_upper(anyarray, dim int) -> int", "Upper bound of dimension.", "SELECT array_upper(ARRAY[1,2,3], 1);", pg("functions-array.html"));
  f!("array_ndims", "array_ndims(anyarray) -> int", "Number of dimensions.", "SELECT array_ndims(ARRAY[[1,2],[3,4]]);", pg("functions-array.html"));
  f!("array_dims", "array_dims(anyarray) -> text", "Text representation of dimensions, e.g. `[1:3]`.", "SELECT array_dims(ARRAY[[1,2],[3,4]]);", pg("functions-array.html"));
  f!("cardinality", "cardinality(anyarray) -> int", "Total element count across all dimensions.", "SELECT cardinality(ARRAY[[1,2],[3,4]]);", pg("functions-array.html"));
  f!("unnest", "unnest(anyarray [, anyarray ...]) -> setof element", "Expand array(s) into rows. Multi-arg form pads shorter arrays with NULL.", "SELECT * FROM unnest(ARRAY[1,2,3]) AS t(v);", pg("functions-array.html"));
  f!("array_fill", "array_fill(elem any, dims int[] [, lowers int[]]) -> anyarray", "Fill an array of given dimensions with `elem`.", "SELECT array_fill(0, ARRAY[3,3]);", pg("functions-array.html"));
  f!("generate_subscripts", "generate_subscripts(anyarray, dim int [, reverse boolean]) -> setof int", "Set of valid subscripts along a dimension.", "SELECT i, a[i] FROM (SELECT ARRAY['a','b','c'] AS a) s, generate_subscripts(a, 1) AS i;", pg("functions-array.html"));
  // ---- cycle2 round 5: json/jsonb fns ----
  f!("jsonb_path_query", "jsonb_path_query(jsonb, jsonpath [, vars jsonb [, silent boolean]]) -> setof jsonb", "JSONPath query returning each match as a row.", "SELECT jsonb_path_query('{\"a\":[1,2,3]}'::jsonb, '$.a[*]');", pg("functions-json.html#FUNCTIONS-SQLJSON-PATH"));
  f!("jsonb_path_query_array", "jsonb_path_query_array(jsonb, jsonpath [, ...]) -> jsonb", "JSONPath query returning all matches as a JSON array.", "SELECT jsonb_path_query_array(j, '$.tags[*]');", pg("functions-json.html#FUNCTIONS-SQLJSON-PATH"));
  f!("jsonb_path_query_first", "jsonb_path_query_first(jsonb, jsonpath [, ...]) -> jsonb", "Return only the first JSONPath match.", "SELECT jsonb_path_query_first(j, '$.name');", pg("functions-json.html#FUNCTIONS-SQLJSON-PATH"));
  f!("jsonb_path_exists", "jsonb_path_exists(jsonb, jsonpath [, ...]) -> boolean", "True when JSONPath matches at least one value.", "SELECT jsonb_path_exists(j, '$.id');", pg("functions-json.html#FUNCTIONS-SQLJSON-PATH"));
  f!("jsonb_path_match", "jsonb_path_match(jsonb, jsonpath [, ...]) -> boolean", "Evaluate a JSONPath that yields a single boolean.", "SELECT jsonb_path_match(j, '$.age > 18');", pg("functions-json.html#FUNCTIONS-SQLJSON-PATH"));
  f!("jsonb_array_length", "jsonb_array_length(jsonb) -> int", "Length of a JSON array; errors on non-array.", "SELECT jsonb_array_length('[1,2,3]'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("jsonb_object_keys", "jsonb_object_keys(jsonb) -> setof text", "Top-level keys of a JSON object.", "SELECT jsonb_object_keys('{\"a\":1,\"b\":2}'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("jsonb_each", "jsonb_each(jsonb) -> setof (key text, value jsonb)", "Iterate object's (key, value) pairs.", "SELECT * FROM jsonb_each('{\"a\":1,\"b\":2}'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("jsonb_each_text", "jsonb_each_text(jsonb) -> setof (key text, value text)", "Same as `jsonb_each` but values cast to text.", "SELECT * FROM jsonb_each_text('{\"a\":1}'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("jsonb_typeof", "jsonb_typeof(jsonb) -> text", "JSON type: 'object', 'array', 'string', 'number', 'boolean', 'null'.", "SELECT jsonb_typeof('[1,2,3]'::jsonb);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_object_agg", "json_object_agg(key, value) -> json", "Aggregate: build a JSON object from key/value pairs.", "SELECT json_object_agg(name, value) FROM kv;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("jsonb_object_agg", "jsonb_object_agg(key, value) -> jsonb", "Aggregate: build a jsonb object from key/value pairs.", "SELECT jsonb_object_agg(name, value) FROM kv;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("json_agg", "json_agg(any [ORDER BY ...]) -> json", "Aggregate: collect rows as a JSON array.", "SELECT json_agg(row(id, name) ORDER BY id) FROM users;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("jsonb_agg", "jsonb_agg(any [ORDER BY ...]) -> jsonb", "Aggregate: collect rows as a jsonb array.", "SELECT jsonb_agg(t ORDER BY id) FROM (SELECT id, name FROM users) t;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("row_to_json", "row_to_json(record [, pretty boolean]) -> json", "Serialize a row/composite to JSON.", "SELECT row_to_json(u) FROM users u LIMIT 1;", pg("functions-json.html#FUNCTIONS-JSON-CREATION-TABLE"));
  f!("json_build_object", "json_build_object(any ...) -> json", "Alternating key/value args, JSON object out.", "SELECT json_build_object('id', 1, 'name', 'a');", pg("functions-json.html#FUNCTIONS-JSON-CREATION-TABLE"));
  f!("jsonb_build_object", "jsonb_build_object(any ...) -> jsonb", "Same as `json_build_object` but jsonb.", "SELECT jsonb_build_object('id', 1, 'name', 'a');", pg("functions-json.html#FUNCTIONS-JSON-CREATION-TABLE"));
  f!("json_build_array", "json_build_array(any ...) -> json", "Variadic args to a JSON array.", "SELECT json_build_array(1, 'x', true);", pg("functions-json.html#FUNCTIONS-JSON-CREATION-TABLE"));
  f!("jsonb_build_array", "jsonb_build_array(any ...) -> jsonb", "Variadic args to a jsonb array.", "SELECT jsonb_build_array(1, 'x', true);", pg("functions-json.html#FUNCTIONS-JSON-CREATION-TABLE"));

  // ---- cycle2 round 6: math fns ----
  f!("abs", "abs(numeric|int|...) -> same", "Absolute value.", "SELECT abs(-12);", pg("functions-math.html"));
  f!("ceiling", "ceiling(numeric) -> numeric", "Smallest integer >= argument (alias of `ceil`).", "SELECT ceiling(1.2);", pg("functions-math.html"));
  f!("floor", "floor(numeric) -> numeric", "Largest integer <= argument.", "SELECT floor(1.9);", pg("functions-math.html"));
  f!("round", "round(numeric [, int]) -> numeric", "Round half-away-from-zero to N decimals (default 0).", "SELECT round(2.567, 2);", pg("functions-math.html"));
  f!("trunc", "trunc(numeric [, int]) -> numeric", "Truncate toward zero to N decimals.", "SELECT trunc(2.567, 1);", pg("functions-math.html"));
  f!("div", "div(numerator, denominator) -> numeric", "Integer division (truncated toward zero).", "SELECT div(7, 2);", pg("functions-math.html"));
  f!("mod", "mod(a, b) -> same", "Remainder of `a / b`. Sign follows the dividend.", "SELECT mod(10, 3);", pg("functions-math.html"));
  f!("power", "power(base, exp) -> double precision|numeric", "Exponentiation; alias of `^`.", "SELECT power(2, 10);", pg("functions-math.html"));
  f!("log", "log(b numeric, x numeric) -> numeric | log(numeric) -> numeric", "Two-arg: log of x in base b. One-arg: base 10.", "SELECT log(2.0, 8.0), log(100);", pg("functions-math.html"));
  f!("sqrt", "sqrt(numeric|double precision) -> same", "Square root.", "SELECT sqrt(144);", pg("functions-math.html"));
  f!("sign", "sign(numeric) -> numeric", "-1, 0, or 1.", "SELECT sign(-3), sign(0), sign(5);", pg("functions-math.html"));
  f!("cos", "cos(double precision) -> double precision", "Cosine (radians).", "SELECT cos(0);", pg("functions-math.html"));
  f!("tan", "tan(double precision) -> double precision", "Tangent (radians).", "SELECT tan(pi()/4);", pg("functions-math.html"));
  f!("asin", "asin(double precision) -> double precision", "Inverse sine; result in radians.", "SELECT asin(1);", pg("functions-math.html"));
  f!("acos", "acos(double precision) -> double precision", "Inverse cosine; result in radians.", "SELECT acos(0);", pg("functions-math.html"));
  f!("atan", "atan(double precision) -> double precision", "Inverse tangent; result in radians.", "SELECT atan(1);", pg("functions-math.html"));
  f!("atan2", "atan2(y double precision, x double precision) -> double precision", "Two-arg arctan; quadrant-aware.", "SELECT atan2(1, 1);", pg("functions-math.html"));
  f!("degrees", "degrees(double precision) -> double precision", "Radians -> degrees.", "SELECT degrees(pi());", pg("functions-math.html"));
  f!("radians", "radians(double precision) -> double precision", "Degrees -> radians.", "SELECT radians(180);", pg("functions-math.html"));
  f!("random", "random() -> double precision", "Uniform [0,1). NOT cryptographically secure -- use `gen_random_uuid()`/`pgcrypto` for tokens.", "SELECT random();", pg("functions-math.html"));
  f!("setseed", "setseed(double precision) -> void", "Seed `random()` with a value in [-1,1] for reproducibility within the session.", "SELECT setseed(0.42); SELECT random();", pg("functions-math.html"));
  f!("scale", "scale(numeric) -> int", "Display scale (digits after decimal point).", "SELECT scale(1.23000);", pg("functions-math.html"));
  f!("min_scale", "min_scale(numeric) -> int", "Minimum scale needed to represent the value exactly (no trailing zeros).", "SELECT min_scale(1.23000);", pg("functions-math.html"));
  f!("trim_scale", "trim_scale(numeric) -> numeric", "Strip trailing zeros from the fractional part.", "SELECT trim_scale(1.23000);", pg("functions-math.html"));

  // ---- cycle2 round 6: regex fns ----
  f!("regexp_match", "regexp_match(text, pattern [, flags]) -> text[]", "Return the first match's capture groups as an array; NULL when no match.", "SELECT regexp_match('foo123bar', '([a-z]+)(\\d+)');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));
  f!("regexp_matches", "regexp_matches(text, pattern [, flags]) -> setof text[]", "All matches as rows. Flag `g` returns every match (else just the first).", "SELECT regexp_matches('aabbcc', '([a-z])\\1', 'g');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));
  f!("regexp_replace", "regexp_replace(text, pattern, replacement [, flags]) -> text", "Replace matches. Use `g` flag for global, `i` for case-insensitive.", "SELECT regexp_replace('Hello world', 'world', 'PG', 'gi');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));
  f!("regexp_split_to_array", "regexp_split_to_array(text, pattern [, flags]) -> text[]", "Split text on regex matches into an array.", "SELECT regexp_split_to_array('a, b ,c', '\\s*,\\s*');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));
  f!("regexp_split_to_table", "regexp_split_to_table(text, pattern [, flags]) -> setof text", "Split text on regex matches into rows.", "SELECT regexp_split_to_table('a;b;c', ';');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));
  f!("regexp_count", "regexp_count(text, pattern [, start int [, flags text]]) -> int", "Count matches (PG15+).", "SELECT regexp_count('abcabc', 'b');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));
  f!("regexp_substr", "regexp_substr(text, pattern [, start int [, n int [, flags text [, subexpr int]]]]) -> text", "Return the nth match (PG15+).", "SELECT regexp_substr('a1 b2 c3', '[a-z]\\d');", pg("functions-matching.html#FUNCTIONS-POSIX-REGEXP"));

  // ---- cycle2 round 6: fts fns ----
  f!("to_tsvector", "to_tsvector([config regconfig,] text) -> tsvector", "Tokenize + normalize text into a tsvector (default config: `default_text_search_config`).", "SELECT to_tsvector('english', 'The quick brown fox');", pg("functions-textsearch.html"));
  f!("to_tsquery", "to_tsquery([config,] text) -> tsquery", "Parse `& | ! <-> ()`-style query into tsquery. Errors on operator chars in raw user input -- prefer `websearch_to_tsquery` for that.", "SELECT to_tsquery('english', 'fox & jump:*');", pg("functions-textsearch.html"));
  f!("plainto_tsquery", "plainto_tsquery([config,] text) -> tsquery", "Treat input as plain words AND-joined; ignores operators.", "SELECT plainto_tsquery('english', 'quick fox');", pg("functions-textsearch.html"));
  f!("phraseto_tsquery", "phraseto_tsquery([config,] text) -> tsquery", "Like `plainto_tsquery` but words joined with phrase distance operator `<->`.", "SELECT phraseto_tsquery('english', 'quick brown fox');", pg("functions-textsearch.html"));
  f!("websearch_to_tsquery", "websearch_to_tsquery([config,] text) -> tsquery", "Google-style: \"quoted phrases\", -minus, OR. Safe for raw user input.", "SELECT websearch_to_tsquery('english', '\"quick fox\" OR -dog');", pg("functions-textsearch.html"));
  f!("ts_rank", "ts_rank([weights,] tsvector, tsquery [, normalization int]) -> real", "Rank by term frequency. Default weights {0.1, 0.2, 0.4, 1.0} for D,C,B,A.", "SELECT ts_rank(doc, q) FROM articles, websearch_to_tsquery('postgres') q WHERE doc @@ q ORDER BY 1 DESC;", pg("functions-textsearch.html"));
  f!("ts_rank_cd", "ts_rank_cd([weights,] tsvector, tsquery [, normalization int]) -> real", "Cover-density rank (rewards lexeme proximity).", "SELECT ts_rank_cd(doc, q) FROM articles, q;", pg("functions-textsearch.html"));
  f!("ts_headline", "ts_headline([config,] text|tsvector, tsquery [, options text]) -> text", "Highlight matching lexemes in text snippet.", "SELECT ts_headline('english', body, q, 'MaxFragments=3') FROM articles, websearch_to_tsquery('pg') q;", pg("functions-textsearch.html"));
  f!("numnode", "numnode(tsquery) -> int", "Number of nodes in a tsquery (for query-complexity checks).", "SELECT numnode('a & b'::tsquery);", pg("functions-textsearch.html"));
  f!("queries_to_xml", "queries_to_xml(tableset_query text [, ...]) -> xml", "Run query and return result-set as XML. Companion of `query_to_xml`.", "SELECT queries_to_xml('SELECT * FROM t;', true, false, '');", pg("functions-xml.html#FUNCTIONS-XML-MAPPING"));

  // ---- cycle2 round 6: range/multirange fns ----
  f!("isempty", "isempty(anyrange|anymultirange) -> boolean", "True when range/multirange covers zero elements.", "SELECT isempty('[5,5)'::int4range);", pg("functions-range.html"));
  f!("lower_inc", "lower_inc(anyrange|anymultirange) -> boolean", "True when the lower bound is inclusive (e.g. `[1,5)` -> true).", "SELECT lower_inc('[1,5)'::int4range);", pg("functions-range.html"));
  f!("upper_inc", "upper_inc(anyrange|anymultirange) -> boolean", "True when the upper bound is inclusive.", "SELECT upper_inc('[1,5]'::int4range);", pg("functions-range.html"));
  f!("range_merge", "range_merge(anyrange, anyrange) -> anyrange | range_merge(anymultirange) -> anyrange", "Smallest range covering all inputs.", "SELECT range_merge('[1,5)'::int4range, '[10,20)'::int4range);", pg("functions-range.html"));
  f!("int4range", "int4range(lower int [, upper int [, bounds text]]) -> int4range", "Construct int4range; `bounds` like '[)' (default) / '(]' / '[]' / '()'.", "SELECT int4range(1, 10, '[]');", pg("rangetypes.html"));
  f!("int8range", "int8range(lower bigint, upper bigint [, bounds text]) -> int8range", "Construct int8range.", "SELECT int8range(1, 10);", pg("rangetypes.html"));
  f!("numrange", "numrange(lower numeric, upper numeric [, bounds text]) -> numrange", "Construct numeric range.", "SELECT numrange(0.0, 1.0);", pg("rangetypes.html"));
  f!("tsrange", "tsrange(lower timestamp, upper timestamp [, bounds]) -> tsrange", "Construct timestamp range.", "SELECT tsrange('2026-01-01', '2027-01-01');", pg("rangetypes.html"));
  f!("tstzrange", "tstzrange(lower timestamptz, upper timestamptz [, bounds]) -> tstzrange", "Construct timestamptz range.", "SELECT tstzrange(now(), now() + interval '1 day');", pg("rangetypes.html"));
  f!("daterange", "daterange(lower date, upper date [, bounds]) -> daterange", "Construct date range.", "SELECT daterange('2026-01-01', '2027-01-01');", pg("rangetypes.html"));
  f!("multirange", "multirange(VARIADIC anyrange[]) -> anymultirange", "Construct multirange from ranges (PG14+).", "SELECT multirange('[1,5)'::int4range, '[10,20)'::int4range);", pg("rangetypes.html"));
  f!("range_agg", "range_agg(anyrange) -> anymultirange", "Aggregate ranges into a multirange (PG14+).", "SELECT range_agg(period) FROM bookings WHERE room_id = 1;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));

  // ---- cycle2 round 7: aggregate + window fns ----
  f!("sum", "sum(numeric|interval|...) -> same", "Aggregate sum; NULL ignored. Result type widens (int->bigint, bigint->numeric).", "SELECT sum(amount) FROM orders;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("avg", "avg(numeric|...) -> numeric|double precision|interval", "Arithmetic mean.", "SELECT avg(price) FROM products;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("count", "count(* | expr) -> bigint", "Row count (`count(*)`) or non-NULL value count (`count(expr)`).", "SELECT count(*), count(email) FROM users;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("min", "min(any) -> same", "Smallest non-NULL value.", "SELECT min(created_at) FROM events;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("max", "max(any) -> same", "Largest non-NULL value.", "SELECT max(price) FROM products;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("bool_and", "bool_and(boolean) -> boolean", "True when every non-NULL row is true.", "SELECT bool_and(active) FROM users;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("bool_or", "bool_or(boolean) -> boolean", "True when any row is true.", "SELECT bool_or(error) FROM logs;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("every", "every(boolean) -> boolean", "SQL-standard alias of `bool_and`.", "SELECT every(active) FROM users;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("bit_and", "bit_and(int|bit) -> same", "Bitwise AND across rows.", "SELECT bit_and(flags) FROM perms;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("bit_or", "bit_or(int|bit) -> same", "Bitwise OR across rows.", "SELECT bit_or(flags) FROM perms;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("stddev", "stddev(numeric) -> numeric|double precision", "Sample standard deviation (alias of `stddev_samp`).", "SELECT stddev(latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("stddev_pop", "stddev_pop(numeric) -> numeric|double precision", "Population standard deviation.", "SELECT stddev_pop(latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("stddev_samp", "stddev_samp(numeric) -> numeric|double precision", "Sample standard deviation (Bessel-corrected).", "SELECT stddev_samp(latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("variance", "variance(numeric) -> numeric|double precision", "Sample variance (alias of `var_samp`).", "SELECT variance(latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("var_pop", "var_pop(numeric) -> numeric|double precision", "Population variance.", "SELECT var_pop(latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("var_samp", "var_samp(numeric) -> numeric|double precision", "Sample variance.", "SELECT var_samp(latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("covar_pop", "covar_pop(y, x) -> double precision", "Population covariance.", "SELECT covar_pop(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("covar_samp", "covar_samp(y, x) -> double precision", "Sample covariance.", "SELECT covar_samp(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("corr", "corr(y, x) -> double precision", "Pearson correlation coefficient.", "SELECT corr(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_slope", "regr_slope(y, x) -> double precision", "Slope of least-squares-fit linear regression.", "SELECT regr_slope(rev, day) FROM daily;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_intercept", "regr_intercept(y, x) -> double precision", "Intercept of linear regression.", "SELECT regr_intercept(rev, day) FROM daily;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_r2", "regr_r2(y, x) -> double precision", "Coefficient of determination.", "SELECT regr_r2(rev, day) FROM daily;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_count", "regr_count(y, x) -> bigint", "Rows with non-NULL y AND x.", "SELECT regr_count(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_avgx", "regr_avgx(y, x) -> double precision", "Average of x for rows where both y and x are non-NULL.", "SELECT regr_avgx(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_avgy", "regr_avgy(y, x) -> double precision", "Average of y for rows where both y and x are non-NULL.", "SELECT regr_avgy(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_sxx", "regr_sxx(y, x) -> double precision", "Sum of squares of x.", "SELECT regr_sxx(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_syy", "regr_syy(y, x) -> double precision", "Sum of squares of y.", "SELECT regr_syy(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("regr_sxy", "regr_sxy(y, x) -> double precision", "Sum of products (x_i * y_i).", "SELECT regr_sxy(y, x) FROM xy;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-STATISTICS-TABLE"));
  f!("mode", "mode() WITHIN GROUP (ORDER BY <expr>) -> same", "Ordered-set aggregate: most common value.", "SELECT mode() WITHIN GROUP (ORDER BY status) FROM tickets;", pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE"));
  f!("percentile_cont", "percentile_cont(fraction) WITHIN GROUP (ORDER BY expr) -> double precision|interval", "Continuous percentile (linear interp). Common: median = percentile_cont(0.5).", "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY salary) FROM employees;", pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE"));
  f!("percentile_disc", "percentile_disc(fraction) WITHIN GROUP (ORDER BY expr) -> same", "Discrete percentile (picks an actual row value).", "SELECT percentile_disc(0.95) WITHIN GROUP (ORDER BY latency) FROM probes;", pg("functions-aggregate.html#FUNCTIONS-ORDEREDSET-TABLE"));
  f!("rank", "rank() OVER (...) -> bigint", "Window: 1-based rank with gaps for ties.", "SELECT rank() OVER (PARTITION BY org ORDER BY score DESC) FROM scores;", pg("functions-window.html"));
  f!("dense_rank", "dense_rank() OVER (...) -> bigint", "Window: rank without gaps.", "SELECT dense_rank() OVER (ORDER BY score DESC) FROM scores;", pg("functions-window.html"));
  f!("percent_rank", "percent_rank() OVER (...) -> double precision", "Window: (rank - 1) / (n - 1).", "SELECT percent_rank() OVER (ORDER BY score) FROM scores;", pg("functions-window.html"));
  f!("cume_dist", "cume_dist() OVER (...) -> double precision", "Window: cumulative distribution = rows preceding-or-peer / total rows.", "SELECT cume_dist() OVER (ORDER BY score) FROM scores;", pg("functions-window.html"));
  f!("row_number", "row_number() OVER (...) -> bigint", "Window: monotonically increasing row counter per partition.", "SELECT row_number() OVER (PARTITION BY user_id ORDER BY ts) FROM events;", pg("functions-window.html"));
  f!("ntile", "ntile(n int) OVER (...) -> int", "Window: assign rows to N equal-sized buckets.", "SELECT ntile(4) OVER (ORDER BY score) FROM scores;", pg("functions-window.html"));
  f!("lag", "lag(expr [, offset int [, default any]]) OVER (...) -> same", "Window: value from row offset rows BEFORE the current one (default 1).", "SELECT ts, lag(ts) OVER (PARTITION BY uid ORDER BY ts) FROM events;", pg("functions-window.html"));
  f!("lead", "lead(expr [, offset int [, default any]]) OVER (...) -> same", "Window: value from row offset rows AFTER the current one.", "SELECT ts, lead(ts) OVER (PARTITION BY uid ORDER BY ts) FROM events;", pg("functions-window.html"));
  f!("first_value", "first_value(expr) OVER (...) -> same", "Window: value at the first row of the window frame.", "SELECT first_value(price) OVER (PARTITION BY sku ORDER BY ts) FROM prices;", pg("functions-window.html"));
  f!("last_value", "last_value(expr) OVER (...) -> same", "Window: value at the last row of the window frame. NB: default frame ends at CURRENT ROW -- use `ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING` for partition-wide.", "SELECT last_value(price) OVER (PARTITION BY sku ORDER BY ts ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) FROM prices;", pg("functions-window.html"));
  f!("nth_value", "nth_value(expr, n int) OVER (...) -> same", "Window: value at nth row of the frame (1-based).", "SELECT nth_value(price, 2) OVER (PARTITION BY sku ORDER BY ts) FROM prices;", pg("functions-window.html"));

  // ---- cycle2 round 7: conversion fns ----
  f!("int4", "int4(any) -> int4", "Cast to int4 (alias `int`/`integer`).", "SELECT int4(1.7);", pg("datatype-numeric.html"));
  f!("int8", "int8(any) -> int8", "Cast to int8 (alias `bigint`).", "SELECT int8('12345');", pg("datatype-numeric.html"));
  f!("int2", "int2(any) -> int2", "Cast to int2 (alias `smallint`).", "SELECT int2(42);", pg("datatype-numeric.html"));
  f!("float4", "float4(any) -> float4", "Cast to float4 (alias `real`).", "SELECT float4('3.14');", pg("datatype-numeric.html"));
  f!("float8", "float8(any) -> float8", "Cast to float8 (alias `double precision`).", "SELECT float8('3.14');", pg("datatype-numeric.html"));

  // ---- cycle2 round 7: bit fns ----
  f!("get_bit", "get_bit(bytea|bit, int) -> int", "Read bit at 0-based offset.", "SELECT get_bit('\\xff'::bytea, 0);", pg("functions-binarystring.html"));
  f!("set_bit", "set_bit(bytea|bit, int, int) -> same", "Write bit at offset.", "SELECT set_bit('\\x00'::bytea, 7, 1);", pg("functions-binarystring.html"));
  f!("get_byte", "get_byte(bytea, int) -> int", "Read byte at 0-based offset (0..255).", "SELECT get_byte('\\xdeadbeef'::bytea, 1);", pg("functions-binarystring.html"));
  f!("set_byte", "set_byte(bytea, int, int) -> bytea", "Write byte at offset.", "SELECT set_byte('\\x00000000'::bytea, 2, 255);", pg("functions-binarystring.html"));

  // ---- cycle2 round 7: network fns ----
  f!("host", "host(inet|cidr) -> text", "Address only, no netmask.", "SELECT host('192.168.1.5/24'::inet);", pg("functions-net.html"));
  f!("netmask", "netmask(inet) -> inet", "Netmask of an inet value.", "SELECT netmask('192.168.1.0/24');", pg("functions-net.html"));
  f!("network", "network(inet) -> cidr", "Network portion as cidr.", "SELECT network('192.168.1.5/24');", pg("functions-net.html"));
  f!("set_masklen", "set_masklen(inet|cidr, int) -> same", "Set the netmask length without changing the address.", "SELECT set_masklen('192.168.1.5'::inet, 16);", pg("functions-net.html"));
  f!("inet", "inet(text) -> inet", "Cast text to inet (also via `::inet`).", "SELECT inet('10.0.0.1');", pg("functions-net.html"));

  // ---- cycle2 round 7: uuid fns ----
  f!("gen_random_uuid", "gen_random_uuid() -> uuid", "Cryptographically-strong random UUID (v4). Built-in since PG13 (no extension needed).", "SELECT gen_random_uuid();", pg("functions-uuid.html"));
  f!("uuid_generate_v1", "uuid_generate_v1() -> uuid", "Time-based UUID v1. Requires `uuid-ossp` extension.", "CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"; SELECT uuid_generate_v1();", pg("uuid-ossp.html"));
  f!("uuid_generate_v4", "uuid_generate_v4() -> uuid", "Random UUID v4 (uuid-ossp). PG13+ has built-in `gen_random_uuid()` -- prefer that.", "SELECT uuid_generate_v4();", pg("uuid-ossp.html"));
  f!("uuid_generate_v5", "uuid_generate_v5(namespace uuid, name text) -> uuid", "Name-based UUID using SHA-1.", "SELECT uuid_generate_v5(uuid_ns_url(), 'https://example.com');", pg("uuid-ossp.html"));

  // ---- cycle2 round 7: hash/crypto fns ----
  f!("sha224", "sha224(bytea) -> bytea", "SHA-224 hash.", "SELECT sha224('abc'::bytea);", pg("functions-binarystring.html"));
  f!("sha256", "sha256(bytea) -> bytea", "SHA-256 hash.", "SELECT encode(sha256('abc'::bytea), 'hex');", pg("functions-binarystring.html"));
  f!("sha384", "sha384(bytea) -> bytea", "SHA-384 hash.", "SELECT sha384('abc'::bytea);", pg("functions-binarystring.html"));
  f!("sha512", "sha512(bytea) -> bytea", "SHA-512 hash.", "SELECT sha512('abc'::bytea);", pg("functions-binarystring.html"));
  f!("hashtext", "hashtext(text) -> int", "Internal fast hash of text (not cryptographic).", "SELECT hashtext('abc');", pg("functions-binarystring.html"));
  f!("crypt", "crypt(password text, salt text) -> text", "pgcrypto: hash password using crypt() schemes (bf, md5, ...). Use `gen_salt('bf')` for salt.", "SELECT crypt('secret', gen_salt('bf'));", pg("pgcrypto.html"));
  f!("gen_salt", "gen_salt(algorithm text [, iter_count int]) -> text", "pgcrypto: generate a salt for `crypt()`. Algos: 'bf', 'md5', 'des', 'xdes'.", "SELECT gen_salt('bf', 12);", pg("pgcrypto.html"));
  f!("encrypt", "encrypt(data bytea, key bytea, type text) -> bytea", "pgcrypto: symmetric encrypt. `type` like 'aes' or 'aes-cbc/pad:pkcs'.", "SELECT encrypt('secret'::bytea, 'key'::bytea, 'aes');", pg("pgcrypto.html"));
  f!("decrypt", "decrypt(data bytea, key bytea, type text) -> bytea", "pgcrypto: symmetric decrypt counterpart of `encrypt`.", "SELECT decrypt(ciphertext, 'key', 'aes');", pg("pgcrypto.html"));
  f!("hmac", "hmac(data bytea|text, key bytea|text, type text) -> bytea", "pgcrypto: HMAC. `type` like 'sha256'.", "SELECT encode(hmac('msg', 'key', 'sha256'), 'hex');", pg("pgcrypto.html"));
  f!("digest", "digest(data bytea|text, type text) -> bytea", "pgcrypto: arbitrary digest. `type` like 'sha256', 'sha512', 'md5'.", "SELECT encode(digest('abc', 'sha256'), 'hex');", pg("pgcrypto.html"));

  // ---- cycle2 round 8: sequence fns ----
  f!("nextval", "nextval(regclass) -> bigint", "Advance sequence and return the new value. Caches per session per `CACHE` setting.", "SELECT nextval('users_id_seq');", pg("functions-sequence.html"));
  f!("currval", "currval(regclass) -> bigint", "Value most recently returned by `nextval` in this session. Errors if no `nextval` has run yet.", "SELECT currval('users_id_seq');", pg("functions-sequence.html"));
  f!("lastval", "lastval() -> bigint", "Value most recently returned by `nextval` for ANY sequence in this session.", "SELECT lastval();", pg("functions-sequence.html"));
  f!("setval", "setval(regclass, bigint [, is_called boolean]) -> bigint", "Reset sequence. With `is_called`=false the next `nextval` returns the same value.", "SELECT setval('users_id_seq', 1000, true);", pg("functions-sequence.html"));

  // ---- cycle2 round 8: advisory lock fns ----
  f!("pg_advisory_lock", "pg_advisory_lock(key bigint) -> void | pg_advisory_lock(key1 int, key2 int)", "Session-level exclusive advisory lock. Blocks until acquired; must `pg_advisory_unlock`.", "SELECT pg_advisory_lock(hashtext('user_42'));", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_advisory_lock_shared", "pg_advisory_lock_shared(key bigint) -> void", "Session-level SHARED advisory lock -- multiple holders OK.", "SELECT pg_advisory_lock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_advisory_unlock", "pg_advisory_unlock(key bigint) -> boolean", "Release one stack-frame of a session advisory lock. True if a frame was released.", "SELECT pg_advisory_unlock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_advisory_unlock_shared", "pg_advisory_unlock_shared(key bigint) -> boolean", "Release a shared advisory lock.", "SELECT pg_advisory_unlock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_advisory_unlock_all", "pg_advisory_unlock_all() -> void", "Release every session-level advisory lock held by this session.", "SELECT pg_advisory_unlock_all();", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_advisory_xact_lock", "pg_advisory_xact_lock(key bigint) -> void", "Transaction-scoped exclusive advisory lock -- auto-released at COMMIT/ROLLBACK.", "BEGIN; SELECT pg_advisory_xact_lock(42); ...; COMMIT;", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_advisory_xact_lock_shared", "pg_advisory_xact_lock_shared(key bigint) -> void", "Transaction-scoped SHARED advisory lock.", "SELECT pg_advisory_xact_lock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_try_advisory_lock", "pg_try_advisory_lock(key bigint) -> boolean", "Non-blocking acquire of session advisory lock. False when busy.", "SELECT pg_try_advisory_lock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_try_advisory_lock_shared", "pg_try_advisory_lock_shared(key bigint) -> boolean", "Non-blocking shared advisory lock acquire.", "SELECT pg_try_advisory_lock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_try_advisory_xact_lock", "pg_try_advisory_xact_lock(key bigint) -> boolean", "Non-blocking transaction-scoped exclusive advisory lock.", "SELECT pg_try_advisory_xact_lock(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));
  f!("pg_try_advisory_xact_lock_shared", "pg_try_advisory_xact_lock_shared(key bigint) -> boolean", "Non-blocking transaction-scoped shared advisory lock.", "SELECT pg_try_advisory_xact_lock_shared(42);", pg("functions-admin.html#FUNCTIONS-ADVISORY-LOCKS"));

  // ---- cycle2 round 8: misc set-returning + sysadmin fns ----
  f!("generate_series", "generate_series(start, stop [, step]) -> setof int|bigint|numeric|timestamp", "Inclusive series. Step defaults to 1 (or `1 day` for timestamps). Negative step iterates downward.", "SELECT * FROM generate_series(1, 10);\nSELECT * FROM generate_series(date '2026-01-01', date '2026-12-31', interval '1 month');", pg("functions-srf.html"));
  f!("pg_sleep", "pg_sleep(seconds double precision) -> void", "Sleep server-side for N seconds.", "SELECT pg_sleep(0.5);", pg("functions-datetime.html#FUNCTIONS-DATETIME-DELAY"));
  f!("pg_sleep_for", "pg_sleep_for(interval) -> void", "Sleep server-side for an interval.", "SELECT pg_sleep_for('2 seconds');", pg("functions-datetime.html#FUNCTIONS-DATETIME-DELAY"));
  f!("pg_sleep_until", "pg_sleep_until(timestamptz) -> void", "Sleep until a timestamp.", "SELECT pg_sleep_until(now() + interval '1 minute');", pg("functions-datetime.html#FUNCTIONS-DATETIME-DELAY"));
  f!("pg_cancel_backend", "pg_cancel_backend(pid int) -> boolean", "Signal cancel (like Ctrl-C) to a backend pid. Caller must be superuser or the same role.", "SELECT pg_cancel_backend(12345);", pg("functions-admin.html#FUNCTIONS-ADMIN-SIGNAL"));
  f!("pg_terminate_backend", "pg_terminate_backend(pid int [, timeout int]) -> boolean", "Send SIGTERM to a backend pid. Stronger than cancel; closes the connection. Optional timeout (PG14+) waits for confirmation.", "SELECT pg_terminate_backend(12345);", pg("functions-admin.html#FUNCTIONS-ADMIN-SIGNAL"));
  f!("pg_stat_reset", "pg_stat_reset() -> void", "Reset every cumulative stats counter in the current DB.", "SELECT pg_stat_reset();", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_stat_reset_shared", "pg_stat_reset_shared(target text) -> void", "Reset a specific cluster-wide stats target ('bgwriter', 'archiver', 'wal', ...).", "SELECT pg_stat_reset_shared('bgwriter');", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_stat_reset_single_table_counters", "pg_stat_reset_single_table_counters(regclass) -> void", "Reset stats for one table only.", "SELECT pg_stat_reset_single_table_counters('users'::regclass);", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_stat_reset_single_function_counters", "pg_stat_reset_single_function_counters(regprocedure) -> void", "Reset stats for one function only.", "SELECT pg_stat_reset_single_function_counters('now()'::regprocedure);", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_is_other_temp_schema", "pg_is_other_temp_schema(oid) -> boolean", "True when the schema is a temp schema belonging to another session.", "SELECT pg_is_other_temp_schema(nspid) FROM pg_namespace;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));

  // ---- cycle2 round 9: pg_get_* introspection fns ----
  f!("pg_get_viewdef", "pg_get_viewdef(view_oid|view_name regclass [, pretty boolean | wrap_column int]) -> text", "Return the underlying SELECT for a view.", "SELECT pg_get_viewdef('public.v_user_summary', true);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_indexdef", "pg_get_indexdef(index_oid regclass [, column int, pretty boolean]) -> text", "Reconstruct `CREATE INDEX` DDL.", "SELECT pg_get_indexdef('users_email_idx'::regclass);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_constraintdef", "pg_get_constraintdef(constraint_oid [, pretty boolean]) -> text", "Reconstruct the constraint definition (USING / CHECK / FK clauses).", "SELECT conname, pg_get_constraintdef(oid) FROM pg_constraint WHERE conrelid = 'users'::regclass;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_functiondef", "pg_get_functiondef(function_oid regprocedure) -> text", "Reconstruct full `CREATE OR REPLACE FUNCTION` DDL.", "SELECT pg_get_functiondef('my_fn(int)'::regprocedure);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_triggerdef", "pg_get_triggerdef(trigger_oid [, pretty boolean]) -> text", "Reconstruct `CREATE TRIGGER` DDL.", "SELECT pg_get_triggerdef(oid) FROM pg_trigger WHERE NOT tgisinternal;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_userbyid", "pg_get_userbyid(role_oid) -> name", "Resolve a role OID to its name. Useful when joining catalog tables exposing OIDs.", "SELECT pg_get_userbyid(relowner) FROM pg_class WHERE relname = 'users';", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_serial_sequence", "pg_get_serial_sequence(table_name text, column_name text) -> text", "Sequence backing a `serial`/`identity` column, fully qualified.", "SELECT pg_get_serial_sequence('users', 'id');", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_expr", "pg_get_expr(node_tree pg_node_tree, relation regclass [, pretty boolean]) -> text", "Render a stored expression tree (e.g. column default, partition bound).", "SELECT attname, pg_get_expr(adbin, adrelid) FROM pg_attrdef JOIN pg_attribute USING (adrelid, adnum);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_statisticsobjdef", "pg_get_statisticsobjdef(statistic_oid) -> text", "Reconstruct `CREATE STATISTICS` DDL.", "SELECT pg_get_statisticsobjdef(oid) FROM pg_statistic_ext;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_relation_filepath", "pg_relation_filepath(regclass) -> text", "Filesystem path (relative to data dir) of a relation's main fork.", "SELECT pg_relation_filepath('users');", pg("functions-admin.html#FUNCTIONS-ADMIN-DBLOCATION"));
  f!("pg_filenode_relation", "pg_filenode_relation(tablespace_oid oid, filenode oid) -> regclass", "Inverse of `pg_relation_filenode`.", "SELECT pg_filenode_relation(1663, 16384);", pg("functions-admin.html#FUNCTIONS-ADMIN-DBLOCATION"));
  f!("pg_log_backend_memory_contexts", "pg_log_backend_memory_contexts(pid int) -> boolean", "Write target backend's memory contexts to the server log (PG14+).", "SELECT pg_log_backend_memory_contexts(12345);", pg("functions-admin.html#FUNCTIONS-ADMIN-SERVER-SIGNALING"));
  f!("pg_typeof", "pg_typeof(\"any\") -> regtype", "Return the type of the expression as a regtype.", "SELECT pg_typeof(NULL::int);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_column_compression", "pg_column_compression(\"any\") -> text", "Compression method used on a TOASTed value ('pglz', 'lz4', etc).", "SELECT pg_column_compression(body) FROM articles LIMIT 1;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_tablespace_databases", "pg_tablespace_databases(oid) -> setof oid", "Databases that have at least one object in the tablespace.", "SELECT * FROM pg_tablespace_databases(1663);", pg("functions-admin.html"));
  f!("pg_tablespace_location", "pg_tablespace_location(oid) -> text", "Filesystem path of a tablespace (empty for pg_default / pg_global).", "SELECT pg_tablespace_location(oid) FROM pg_tablespace;", pg("functions-admin.html"));
  f!("pg_options_to_table", "pg_options_to_table(options_array text[]) -> setof (option_name text, option_value text)", "Expand a `name=value` array (as stored in pg_foreign_server etc) to rows.", "SELECT * FROM pg_options_to_table(srvoptions) FROM pg_foreign_server;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));

  // ---- cycle2 round 9: monitoring + encoding + visibility fns ----
  f!("pg_safe_snapshot_blocking_pids", "pg_safe_snapshot_blocking_pids(pid int) -> int[]", "Pids holding the SERIALIZABLE snapshot a given pid is waiting for.", "SELECT pg_safe_snapshot_blocking_pids(pg_backend_pid());", pg("functions-info.html#FUNCTIONS-INFO-SESSION"));
  f!("pg_isolation_test_session_is_blocked", "pg_isolation_test_session_is_blocked(pid int, interesting_pids int[]) -> boolean", "Internal helper used by `isolationtester`. Mostly for PG dev/test.", "SELECT pg_isolation_test_session_is_blocked(pid, ARRAY[other_pid]);", pg("functions-info.html"));
  f!("pg_collation_actual_version", "pg_collation_actual_version(collation_oid) -> text", "OS-reported version of an ICU/libc collation -- compare with `pg_collation.collversion` to detect upgrades that break sort order.", "SELECT collname, pg_collation_actual_version(oid) FROM pg_collation WHERE collprovider IN ('i','d');", pg("collation.html"));
  f!("pg_collation_for", "pg_collation_for(\"any\") -> text", "Collation of an expression as a quoted identifier.", "SELECT pg_collation_for('hello'::text);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_encoding_to_char", "pg_encoding_to_char(encoding int) -> name", "Encoding id (pg_database.encoding) to its name.", "SELECT pg_encoding_to_char(encoding) FROM pg_database;", pg("functions-info.html"));
  f!("pg_char_to_encoding", "pg_char_to_encoding(text) -> int", "Encoding name to id.", "SELECT pg_char_to_encoding('UTF8');", pg("functions-info.html"));
  f!("pg_client_encoding", "pg_client_encoding() -> name", "Client encoding for the current session.", "SELECT pg_client_encoding();", pg("functions-info.html"));
  f!("format_type", "format_type(type_oid oid, typmod int) -> text", "Pretty-print a type with its typmod (e.g. `numeric(12,2)`).", "SELECT format_type(atttypid, atttypmod) FROM pg_attribute WHERE attrelid = 'users'::regclass;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_type_is_visible", "pg_type_is_visible(type_oid) -> boolean", "True when the type is reachable without schema-qualification under the current search_path.", "SELECT pg_type_is_visible(oid) FROM pg_type WHERE typname = 'myenum';", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_table_is_visible", "pg_table_is_visible(table_oid) -> boolean", "True when the table/view/sequence is reachable without schema-qualification.", "SELECT pg_table_is_visible('users'::regclass);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_function_is_visible", "pg_function_is_visible(function_oid) -> boolean", "True when the function is reachable without schema-qualification.", "SELECT pg_function_is_visible('now()'::regprocedure);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_event_trigger_ddl_commands", "pg_event_trigger_ddl_commands() -> setof record", "From inside an event trigger: rows describing the DDL command(s) running.", "-- inside event trigger body\nFOR r IN SELECT * FROM pg_event_trigger_ddl_commands() LOOP ... END LOOP;", pg("functions-event-triggers.html"));
  f!("pg_event_trigger_dropped_objects", "pg_event_trigger_dropped_objects() -> setof record", "From inside an `sql_drop` event trigger: rows for each dropped object.", "FOR r IN SELECT * FROM pg_event_trigger_dropped_objects() LOOP ... END LOOP;", pg("functions-event-triggers.html"));
  f!("pg_event_trigger_table_rewrite_oid", "pg_event_trigger_table_rewrite_oid() -> oid", "Inside a `table_rewrite` event trigger: OID of the table being rewritten.", "SELECT pg_event_trigger_table_rewrite_oid();", pg("functions-event-triggers.html"));
  f!("pg_event_trigger_table_rewrite_reason", "pg_event_trigger_table_rewrite_reason() -> int", "Bitmask describing why the table is being rewritten.", "SELECT pg_event_trigger_table_rewrite_reason();", pg("functions-event-triggers.html"));

  // ---- cycle2 round 9: SQL/JSON + soft-input + FTS internals ----
  f!("json_table", "JSON_TABLE(<json> '<jsonpath>' [PASSING ...] COLUMNS (...)) -- shred JSON to a relational result set (SQL:2023 / PG17+).", "Like XMLTABLE for JSON.", "SELECT * FROM JSON_TABLE(j, '$.items[*]' COLUMNS (id INT PATH '$.id', name TEXT PATH '$.name'));", pg("functions-json.html#FUNCTIONS-SQLJSON-TABLE"));
  f!("jsonb_to_tsvector", "jsonb_to_tsvector([config,] jsonb, filter jsonb) -> tsvector", "Build a tsvector from JSON, filtered by element-type list ('[\"string\",\"numeric\"]' etc).", "SELECT jsonb_to_tsvector('english', j, '[\"string\"]');", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("json_to_tsvector", "json_to_tsvector([config,] json, filter jsonb) -> tsvector", "Same as `jsonb_to_tsvector` for `json`.", "SELECT json_to_tsvector('english', j, '[\"all\"]');", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("json_query", "JSON_QUERY(<json>, '<jsonpath>' [RETURNING <type>] [WRAPPER ...] [{ERROR|NULL|EMPTY ARRAY|EMPTY OBJECT|DEFAULT <e>} ON EMPTY|ON ERROR])", "SQL:2023 JSON_QUERY (PG17+) -- return JSON value(s) matching jsonpath; configurable behavior on empty/error.", "SELECT JSON_QUERY(j, '$.tags' RETURNING jsonb NULL ON EMPTY);", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  f!("json_value", "JSON_VALUE(<json>, '<jsonpath>' [RETURNING <type>] [{ERROR|NULL|DEFAULT <e>} ON EMPTY|ON ERROR])", "SQL:2023 JSON_VALUE (PG17+) -- extract a single SQL scalar value.", "SELECT JSON_VALUE(j, '$.user.id' RETURNING bigint DEFAULT 0 ON ERROR);", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  f!("json_exists", "JSON_EXISTS(<json>, '<jsonpath>' [{TRUE|FALSE|UNKNOWN|ERROR} ON ERROR])", "SQL:2023 JSON_EXISTS (PG17+) -- boolean does-it-match.", "SELECT JSON_EXISTS(j, '$.user.email');", pg("functions-json.html#FUNCTIONS-SQLJSON-QUERY"));
  f!("pg_input_is_valid", "pg_input_is_valid(input text, type text) -> boolean", "PG16+: would casting `input` to `type` succeed (without throwing)?", "SELECT pg_input_is_valid('42x', 'integer');", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_input_error_info", "pg_input_error_info(input text, type text) -> setof (message text, detail text, hint text, sql_error_code text)", "PG16+: detail rows for why a cast would fail; empty when valid.", "SELECT * FROM pg_input_error_info('42x', 'integer');", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("ts_lexize", "ts_lexize(dictionary regdictionary, token text) -> text[]", "Run a single token through an FTS dictionary; useful for debugging.", "SELECT ts_lexize('english_stem', 'running');", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("ts_token_type", "ts_token_type([parser regconfig|name]) -> setof (tokid int, alias text, description text)", "Token types a parser can emit.", "SELECT * FROM ts_token_type('default');", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("ts_parse", "ts_parse(parser_name|oid, text) -> setof (tokid int, token text)", "Run a parser over text; useful for FTS debugging.", "SELECT * FROM ts_parse('default', 'The quick brown fox');", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("ts_debug", "ts_debug([config,] text) -> setof record", "Shows each token, its dictionaries, and lexemes -- the FTS debugging Swiss knife.", "SELECT * FROM ts_debug('english', 'The quick brown fox');", pg("textsearch-debugging.html"));

  // ---- cycle2 round 10: replication slot / origin / WAL fns ----
  f!("pg_replication_origin_create", "pg_replication_origin_create(node_name text) -> oid", "Create a replication origin identifier (logical replication / pglogical-style apply tracking).", "SELECT pg_replication_origin_create('upstream_1');", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_replication_origin_drop", "pg_replication_origin_drop(node_name text) -> void", "Delete a replication origin.", "SELECT pg_replication_origin_drop('upstream_1');", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_replication_origin_session_setup", "pg_replication_origin_session_setup(node_name text) -> void", "Mark current session as applying changes from `node_name`. Pair with `_session_reset` at exit.", "SELECT pg_replication_origin_session_setup('upstream_1');", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_replication_origin_session_reset", "pg_replication_origin_session_reset() -> void", "Clear the origin association on the current session.", "SELECT pg_replication_origin_session_reset();", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_logical_slot_peek_changes", "pg_logical_slot_peek_changes(slot_name name, upto_lsn pg_lsn, upto_nchanges int, VARIADIC options text[]) -> setof (lsn pg_lsn, xid xid, data text)", "Like `pg_logical_slot_get_changes` but does NOT advance the slot -- repeatable reads.", "SELECT * FROM pg_logical_slot_peek_changes('my_slot', NULL, NULL);", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_logical_slot_get_binary_changes", "pg_logical_slot_get_binary_changes(slot_name name, upto_lsn pg_lsn, upto_nchanges int, VARIADIC options text[]) -> setof (lsn pg_lsn, xid xid, data bytea)", "Drain a binary-output logical replication slot; consumes changes.", "SELECT * FROM pg_logical_slot_get_binary_changes('my_slot', NULL, NULL);", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_logical_slot_peek_binary_changes", "pg_logical_slot_peek_binary_changes(slot_name name, upto_lsn pg_lsn, upto_nchanges int, VARIADIC options text[]) -> setof (lsn pg_lsn, xid xid, data bytea)", "Peek at binary changes without advancing the slot.", "SELECT * FROM pg_logical_slot_peek_binary_changes('my_slot', NULL, NULL);", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_replication_origin_oid", "pg_replication_origin_oid(node_name text) -> oid", "OID of a named replication origin.", "SELECT pg_replication_origin_oid('upstream_1');", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_replication_origin_session_progress", "pg_replication_origin_session_progress(flush boolean) -> pg_lsn", "Last applied LSN tracked for the session's origin.", "SELECT pg_replication_origin_session_progress(true);", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_show_replication_origin_status", "pg_show_replication_origin_status() -> setof (local_id oid, external_id text, remote_lsn pg_lsn, local_lsn pg_lsn)", "Snapshot of every replication origin's progress.", "SELECT * FROM pg_show_replication_origin_status();", pg("functions-admin.html#FUNCTIONS-REPLICATION"));
  f!("pg_get_wal_resource_managers", "pg_get_wal_resource_managers() -> setof (rm_id int, rm_name text, rm_builtin boolean)", "PG15+: WAL resource managers known to the server (Heap, Btree, custom RMGRs).", "SELECT * FROM pg_get_wal_resource_managers();", pg("functions-admin.html#FUNCTIONS-CONTROLDATA"));
  f!("pg_get_wal_replay_pause_state", "pg_get_wal_replay_pause_state() -> text", "Standby-only: 'not paused' / 'pause requested' / 'paused'.", "SELECT pg_get_wal_replay_pause_state();", pg("functions-admin.html#FUNCTIONS-RECOVERY-CONTROL"));
  f!("pg_is_wal_replay_paused", "pg_is_wal_replay_paused() -> boolean", "Standby-only: WAL replay currently paused?", "SELECT pg_is_wal_replay_paused();", pg("functions-admin.html#FUNCTIONS-RECOVERY-CONTROL"));
  f!("pg_wal_replay_pause", "pg_wal_replay_pause() -> void", "Standby-only: pause WAL replay.", "SELECT pg_wal_replay_pause();", pg("functions-admin.html#FUNCTIONS-RECOVERY-CONTROL"));
  f!("pg_wal_replay_resume", "pg_wal_replay_resume() -> void", "Standby-only: resume WAL replay.", "SELECT pg_wal_replay_resume();", pg("functions-admin.html#FUNCTIONS-RECOVERY-CONTROL"));

  // ---- cycle2 round 10: listen/notify queue + lo_* (large object) ----
  f!("pg_notification_queue_usage", "pg_notification_queue_usage() -> double precision", "Fraction (0..1) of the global async-notify queue currently used. >0.5 = pageable.", "SELECT pg_notification_queue_usage();", pg("functions-info.html#FUNCTIONS-INFO-SESSION"));
  f!("lo_create", "lo_create(loid oid) -> oid", "Create a large object with the given OID (0 = pick free OID).", "SELECT lo_create(0);", pg("lo-funcs.html"));
  f!("lo_unlink", "lo_unlink(loid oid) -> int", "Delete a large object.", "SELECT lo_unlink(16385);", pg("lo-funcs.html"));
  f!("lo_open", "lo_open(loid oid, mode int) -> int", "Open a large object; modes: INV_READ=0x40000, INV_WRITE=0x20000.", "SELECT lo_open(16385, 0x40000);", pg("lo-funcs.html"));
  f!("lo_close", "lo_close(fd int) -> int", "Close a large-object file descriptor.", "SELECT lo_close(fd);", pg("lo-funcs.html"));
  f!("lo_read", "lo_read(fd int, len int) -> bytea", "Read up to len bytes from a large-object fd.", "SELECT lo_read(fd, 8192);", pg("lo-funcs.html"));
  f!("lo_write", "lo_write(fd int, data bytea) -> int", "Write bytes to a large-object fd.", "SELECT lo_write(fd, data);", pg("lo-funcs.html"));
  f!("lo_lseek", "lo_lseek(fd int, offset int, whence int) -> int", "32-bit seek inside a large object. Whence: 0=SET, 1=CUR, 2=END.", "SELECT lo_lseek(fd, 0, 0);", pg("lo-funcs.html"));
  f!("lo_lseek64", "lo_lseek64(fd int, offset bigint, whence int) -> bigint", "64-bit seek for large objects > 2 GB.", "SELECT lo_lseek64(fd, 3000000000, 0);", pg("lo-funcs.html"));
  f!("lo_tell", "lo_tell(fd int) -> int", "Current offset (32-bit).", "SELECT lo_tell(fd);", pg("lo-funcs.html"));
  f!("lo_tell64", "lo_tell64(fd int) -> bigint", "Current offset (64-bit).", "SELECT lo_tell64(fd);", pg("lo-funcs.html"));
  f!("lo_truncate", "lo_truncate(fd int, len int) -> int", "Truncate (32-bit).", "SELECT lo_truncate(fd, 0);", pg("lo-funcs.html"));
  f!("lo_truncate64", "lo_truncate64(fd int, len bigint) -> int", "Truncate (64-bit).", "SELECT lo_truncate64(fd, 0);", pg("lo-funcs.html"));
  f!("lo_put", "lo_put(loid oid, offset bigint, data bytea) -> void", "Overwrite bytes at offset -- whole-object form, no fd dance.", "SELECT lo_put(16385, 0, '\\xdeadbeef');", pg("lo-funcs.html"));
  f!("lo_get", "lo_get(loid oid [, offset bigint, length int]) -> bytea", "Read part (or all) of a large object as bytea.", "SELECT lo_get(16385, 0, 1024);", pg("lo-funcs.html"));
  f!("lo_from_bytea", "lo_from_bytea(loid oid, data bytea) -> oid", "Create a large object from bytea.", "SELECT lo_from_bytea(0, '\\xdeadbeef');", pg("lo-funcs.html"));
  f!("lo_export", "lo_export(loid oid, file text) -> int", "Write a large object to a server-side file (superuser).", "SELECT lo_export(16385, '/tmp/blob.bin');", pg("lo-funcs.html"));
  f!("lo_import", "lo_import(file text [, loid oid]) -> oid", "Load a server-side file into a large object (superuser).", "SELECT lo_import('/tmp/blob.bin');", pg("lo-funcs.html"));

  // ---- cycle2 round 10: server-side fs fns ----
  f!("pg_ls_dir", "pg_ls_dir(dir text [, missing_ok boolean, include_dot_dirs boolean]) -> setof text", "List entries in a server-side directory (superuser or pg_read_server_files).", "SELECT pg_ls_dir('base');", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_ls_logdir", "pg_ls_logdir() -> setof (name text, size bigint, modification timestamptz)", "List the server log directory contents.", "SELECT * FROM pg_ls_logdir();", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_ls_waldir", "pg_ls_waldir() -> setof (name text, size bigint, modification timestamptz)", "List pg_wal/.", "SELECT * FROM pg_ls_waldir();", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_ls_tmpdir", "pg_ls_tmpdir([tablespace oid]) -> setof (name text, size bigint, modification timestamptz)", "List the temp directory of a tablespace.", "SELECT * FROM pg_ls_tmpdir();", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_ls_archive_statusdir", "pg_ls_archive_statusdir() -> setof (name text, size bigint, modification timestamptz)", "List pg_wal/archive_status/.", "SELECT * FROM pg_ls_archive_statusdir();", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_read_file", "pg_read_file(filename text [, offset bigint, length bigint [, missing_ok boolean]]) -> text", "Read a server-side file as text (superuser or pg_read_server_files).", "SELECT pg_read_file('postgresql.conf', 0, 4096);", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_read_binary_file", "pg_read_binary_file(filename text [, offset bigint, length bigint [, missing_ok boolean]]) -> bytea", "Binary counterpart of `pg_read_file`.", "SELECT length(pg_read_binary_file('PG_VERSION'));", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));
  f!("pg_stat_file", "pg_stat_file(filename text [, missing_ok boolean]) -> record", "Server-side `stat()` info: size, modification, change, access, creation, isdir.", "SELECT * FROM pg_stat_file('postgresql.conf');", pg("functions-admin.html#FUNCTIONS-ADMIN-GENFILE"));

  // ---- cycle2 round 10: dblink (contrib) ----
  f!("dblink", "dblink(connstr text, sql text) -> setof record", "Execute a query on a remote PG via dblink and return rows. Caller supplies the AS (col types) list.", "SELECT * FROM dblink('host=remote dbname=app', 'SELECT id, name FROM t') AS r(id int, name text);", pg("dblink.html"));
  f!("dblink_exec", "dblink_exec(connstr text, sql text) -> text", "Execute a non-returning SQL on a remote server.", "SELECT dblink_exec('myconn', 'INSERT INTO t VALUES (1)');", pg("dblink.html"));
  f!("dblink_connect", "dblink_connect(name text, connstr text) -> text", "Open a named persistent dblink connection.", "SELECT dblink_connect('myconn', 'host=remote dbname=app');", pg("dblink.html"));
  f!("dblink_disconnect", "dblink_disconnect(name text) -> text", "Close a named dblink connection.", "SELECT dblink_disconnect('myconn');", pg("dblink.html"));
  f!("dblink_get_connections", "dblink_get_connections() -> text[]", "Names of all open dblink connections in this session.", "SELECT dblink_get_connections();", pg("dblink.html"));

  // ---- cycle2 round 10: misc helpers ----
  f!("num_nonnulls", "num_nonnulls(VARIADIC \"any\") -> int", "Count non-NULL args (PG10+).", "SELECT num_nonnulls(a, b, c) FROM t;", pg("functions-comparison.html#FUNCTIONS-COMPARISON-FUNC-TABLE"));
  f!("num_nulls", "num_nulls(VARIADIC \"any\") -> int", "Count NULL args.", "SELECT num_nulls(a, b, c) FROM t;", pg("functions-comparison.html#FUNCTIONS-COMPARISON-FUNC-TABLE"));
  f!("suppress_redundant_updates_trigger", "suppress_redundant_updates_trigger() -> trigger", "BEFORE UPDATE trigger that skips the row update when nothing changed -- cheaper than recomputing in app code.", "CREATE TRIGGER skip_noop BEFORE UPDATE ON t FOR EACH ROW EXECUTE FUNCTION suppress_redundant_updates_trigger();", pg("functions-trigger.html"));
  f!("tsmatchsel", "tsmatchsel(internal, oid, internal, int) -> double precision", "Selectivity-estimator for the `@@` FTS match operator. Internal -- referenced by `CREATE OPERATOR ... RESTRICT = tsmatchsel`.", "-- internal", pg("internals/index-functions.html"));
  f!("tsmatchjoinsel", "tsmatchjoinsel(internal, oid, internal, smallint, internal) -> double precision", "Join-selectivity estimator counterpart of `tsmatchsel`.", "-- internal", pg("internals/index-functions.html"));

  // ---- cycle2 round 69: misc + GUC + encoding helpers ----
  f!("width_bucket", "width_bucket(value, low, high, count int) -> int | width_bucket(value, array) -> int", "Map value into one of N equiwidth buckets between low/high, or into a sorted-array bucket. Useful for histograms.", "SELECT width_bucket(score, 0, 100, 10) AS bucket, count(*) FROM scores GROUP BY 1 ORDER BY 1;", pg("functions-math.html"));
  f!("bound_box", "bound_box(box, box) -> box", "Smallest box containing both input boxes.", "SELECT bound_box(box '(0,0),(1,1)', box '(2,2),(3,3)');", pg("functions-geometry.html"));
  f!("convert", "convert(bytea, src_encoding text, dst_encoding text) -> bytea", "Convert bytes between encodings. Companion of `convert_from` / `convert_to`.", "SELECT convert('\\xc3a9'::bytea, 'UTF8', 'LATIN1');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("convert_from", "convert_from(bytea, src_encoding text) -> text", "Decode bytes from a given encoding into a text value (using the server encoding).", "SELECT convert_from('\\xc3a9'::bytea, 'UTF8');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("convert_to", "convert_to(text, dst_encoding text) -> bytea", "Encode text into a target encoding's byte representation.", "SELECT convert_to('café', 'LATIN1');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("set_config", "set_config(setting_name text, new_value text, is_local boolean) -> text", "Equivalent of `SET`/`SET LOCAL`. Returns the new value as text.", "SELECT set_config('statement_timeout', '5s', false);", pg("functions-admin.html#FUNCTIONS-ADMIN-SET"));
  f!("current_setting", "current_setting(setting_name text [, missing_ok boolean]) -> text", "Equivalent of `SHOW`. Returns the current GUC value as text. `missing_ok` true returns NULL when unknown.", "SELECT current_setting('search_path');", pg("functions-admin.html#FUNCTIONS-ADMIN-SET"));
  f!("cluster_name", "cluster_name -> text", "Server-wide cluster_name GUC (set in postgresql.conf or via ALTER SYSTEM).", "SELECT cluster_name;", pg("functions-info.html"));
  f!("pg_jit_available", "pg_jit_available() -> boolean", "True when the server has a JIT provider compiled in and `jit` GUC enabled.", "SELECT pg_jit_available();", pg("functions-info.html"));
  f!("pg_index_column_has_property", "pg_index_column_has_property(index_oid regclass, column_no int, property text) -> boolean", "Test per-column index capabilities (e.g. 'asc', 'desc', 'orderable', 'nulls_first').", "SELECT pg_index_column_has_property('users_email_idx'::regclass, 1, 'orderable');", pg("functions-info.html#FUNCTIONS-INFO-INDEX-COLUMN-PROPS"));
  f!("pg_index_has_property", "pg_index_has_property(index_oid regclass, property text) -> boolean", "Test whole-index capabilities (e.g. 'returnable', 'distance_orderable').", "SELECT pg_index_has_property('users_email_idx'::regclass, 'distance_orderable');", pg("functions-info.html"));
  f!("pg_indexam_has_property", "pg_indexam_has_property(am_oid oid, property text) -> boolean", "Test access-method-level capabilities (e.g. 'can_order', 'can_unique', 'can_multi_col').", "SELECT pg_indexam_has_property('btree'::regam, 'can_unique');", pg("functions-info.html"));
  // ---- cycle2 round 69: more JSON fns ----
  f!("to_jsonb", "to_jsonb(any) -> jsonb", "Convert any value to its JSONB representation.", "SELECT to_jsonb(ARRAY[1,2,3]);", pg("functions-json.html#FUNCTIONS-JSON-CREATION-TABLE"));
  f!("json_strip_nulls", "json_strip_nulls(json) -> json", "Recursively drop keys with NULL values from a JSON object.", "SELECT json_strip_nulls('{\"a\":1,\"b\":null}'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_array_length", "json_array_length(json) -> int", "Length of a JSON array.", "SELECT json_array_length('[1,2,3]'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_typeof", "json_typeof(json) -> text", "JSON type label: 'object'/'array'/'string'/'number'/'boolean'/'null'.", "SELECT json_typeof('[1,2,3]'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_extract_path", "json_extract_path(json, variadic text[]) -> json", "Walk a JSON path (key-by-key) and return the matching JSON.", "SELECT json_extract_path('{\"a\":{\"b\":1}}'::json, 'a', 'b');", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_extract_path_text", "json_extract_path_text(json, variadic text[]) -> text", "Same as `json_extract_path` but coerces the result to text.", "SELECT json_extract_path_text('{\"a\":1}'::json, 'a');", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_each", "json_each(json) -> setof (key text, value json)", "Iterate object's (key, value) pairs.", "SELECT * FROM json_each('{\"a\":1,\"b\":2}'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_each_text", "json_each_text(json) -> setof (key text, value text)", "Same as `json_each` but values cast to text.", "SELECT * FROM json_each_text('{\"a\":1}'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_object_keys", "json_object_keys(json) -> setof text", "Top-level keys of a JSON object.", "SELECT json_object_keys('{\"a\":1,\"b\":2}'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  f!("json_populate_recordset", "json_populate_recordset(base anyelement, json) -> setof anyelement", "Materialise an array of JSON objects as rows of a target composite type.", "SELECT * FROM json_populate_recordset(null::users, '[{\"id\":1},{\"id\":2}]'::json);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));
  // ---- cycle2 round 69: more reg / hash fns ----
  f!("to_regnamespace", "to_regnamespace(text) -> regnamespace", "Look up schema OID by name; NULL when missing.", "SELECT to_regnamespace('public') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("to_regrole", "to_regrole(text) -> regrole", "Look up role OID by name; NULL when missing.", "SELECT to_regrole('postgres') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("to_regoperator", "to_regoperator(text) -> regoperator", "Look up operator OID by signature 'op(lt,rt)'; NULL when missing.", "SELECT to_regoperator('+(int,int)') IS NOT NULL;", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("hashtextextended", "hashtextextended(text, seed bigint) -> bigint", "64-bit seeded hash of text. PG13+; used by hash partitioning and parallel ops.", "SELECT hashtextextended('abc', 0);", pg("functions-binarystring.html"));
  f!("hashbpchar", "hashbpchar(bpchar) -> int", "Internal hash of a fixed-length character. Referenced by hash index/AM definitions.", "-- internal", pg("functions-binarystring.html"));

  // ---- cycle2 round 70: FTS + pg_trgm helpers ----
  f!("tsvector_to_array", "tsvector_to_array(tsvector) -> text[]", "Project lexemes of a tsvector into a text[] (positions/weights dropped).", "SELECT tsvector_to_array(to_tsvector('a b c'));", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("array_to_tsvector", "array_to_tsvector(text[]) -> tsvector", "Build a tsvector from an array of lexemes (no normalization).", "SELECT array_to_tsvector(ARRAY['a','b','c']);", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("ts_strip", "ts_strip(tsvector) -> tsvector", "Strip positions and weights from a tsvector.", "SELECT ts_strip(to_tsvector('a:1A b:2'));", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("ts_setweight", "setweight(tsvector, weight char [, lexemes text[]]) -> tsvector", "Re-weight an entire tsvector (or only given lexemes) with A/B/C/D.", "SELECT setweight(to_tsvector('quick fox'), 'A');", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("ts_length", "length(tsvector) -> int", "Number of distinct lexemes in a tsvector.", "SELECT length(to_tsvector('a a b'));", pg("functions-textsearch.html#TEXTSEARCH-FUNCTIONS-TABLE"));
  f!("similarity", "similarity(text, text) -> real", "pg_trgm: trigram similarity in [0, 1]. 0 = no overlap, 1 = identical.", "SELECT similarity('postgres', 'postgresql');", pg("pgtrgm.html"));
  f!("word_similarity", "word_similarity(text, text) -> real", "pg_trgm: best similarity of left-side phrase within right text.", "SELECT word_similarity('post', 'postgres rocks');", pg("pgtrgm.html"));
  f!("strict_word_similarity", "strict_word_similarity(text, text) -> real", "Stricter `word_similarity` that requires word boundaries on the right side.", "SELECT strict_word_similarity('post', 'postgres rocks');", pg("pgtrgm.html"));
  f!("show_trgm", "show_trgm(text) -> text[]", "pg_trgm: trigrams the text decomposes into.", "SELECT show_trgm('Postgres');", pg("pgtrgm.html"));
  f!("show_limit", "show_limit() -> real", "pg_trgm: current similarity threshold for `%` operator.", "SELECT show_limit();", pg("pgtrgm.html"));
  f!("set_limit", "set_limit(real) -> real", "pg_trgm: set the similarity threshold for `%`.", "SELECT set_limit(0.4);", pg("pgtrgm.html"));

  // ---- cycle2 round 70: geometric constructors / helpers ----
  f!("box", "box(text|point,point|polygon|circle) -> box", "Construct or convert to a box.", "SELECT box(point '(0,0)', point '(1,1)');", pg("functions-geometry.html"));
  f!("circle", "circle(text|point, radius|box|polygon) -> circle", "Construct or convert to a circle.", "SELECT circle(point '(0,0)', 5);", pg("functions-geometry.html"));
  f!("line", "line(point, point) -> line", "Construct an infinite line through two points.", "SELECT line(point '(0,0)', point '(1,1)');", pg("functions-geometry.html"));
  f!("lseg", "lseg(point, point) -> lseg", "Construct a line segment between two points.", "SELECT lseg(point '(0,0)', point '(1,1)');", pg("functions-geometry.html"));
  f!("path", "path(text|polygon) -> path", "Convert text/polygon into a path.", "SELECT path(polygon '((0,0),(1,0),(1,1))');", pg("functions-geometry.html"));
  f!("point", "point(double precision, double precision) -> point", "Construct a point. Alias `(x, y)::point`.", "SELECT point(1.0, 2.0);", pg("functions-geometry.html"));
  f!("polygon", "polygon(text|box|path|circle) -> polygon", "Construct or convert to a polygon.", "SELECT polygon(box '(0,0),(1,1)');", pg("functions-geometry.html"));
  f!("circle_in", "circle_in(cstring) -> circle", "Type-input function for `circle`. Called by the parser, rarely by users.", "-- internal", pg("functions-geometry.html"));
  f!("box_in", "box_in(cstring) -> box", "Type-input function for `box`. Internal.", "-- internal", pg("functions-geometry.html"));

  // ---- cycle2 round 70: date/time fns ----
  f!("extract", "EXTRACT(<field> FROM <timestamp|interval>) -> double precision", "Pull a sub-field (year, month, day, dow, doy, epoch, ...) from a timestamp/interval.", "SELECT EXTRACT(EPOCH FROM (now() - birth)) FROM users;", pg("functions-datetime.html#FUNCTIONS-DATETIME-EXTRACT"));
  f!("isfinite", "isfinite(date|timestamp|interval) -> boolean", "True when the value is not `-infinity`/`infinity`.", "SELECT isfinite(now());", pg("functions-datetime.html"));
  f!("timezone", "timezone(text, timestamp|timestamptz|time|timetz) -> ...", "Equivalent of `AT TIME ZONE`. Convert between zones.", "SELECT timezone('UTC', now());", pg("functions-datetime.html#FUNCTIONS-DATETIME-ZONECONVERT"));
  f!("timeofday", "timeofday() -> text", "Current wall-clock time as a human-readable string. Cheap debug helper.", "SELECT timeofday();", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  f!("clock_timestamp", "clock_timestamp() -> timestamptz", "Real-time clock (changes within a statement). Compare with `now()` (transaction-time).", "SELECT clock_timestamp();", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  f!("transaction_timestamp", "transaction_timestamp() -> timestamptz", "Alias of `now()` -- transaction start time.", "SELECT transaction_timestamp();", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));
  f!("statement_timestamp", "statement_timestamp() -> timestamptz", "Current statement start time (constant within one statement).", "SELECT statement_timestamp();", pg("functions-datetime.html#FUNCTIONS-DATETIME-CURRENT"));

  // ---- cycle2 round 71: more interval + introspection ----
  f!("date_bin", "date_bin(stride interval, source timestamp, origin timestamp) -> timestamp", "Snap a timestamp to the nearest stride-sized bin starting at `origin`. PG14+. Useful for time-series bucketing.", "SELECT date_bin('15 minutes', now(), TIMESTAMP '2000-01-01');", pg("functions-datetime.html#FUNCTIONS-DATETIME-BIN"));
  f!("date_diff", "date_diff(field text, a timestamp, b timestamp) -> bigint", "PG17+ helper: signed difference between two timestamps in whole units of `field` ('year','month','day','hour','minute','second',...).", "SELECT date_diff('day', '2026-01-01'::timestamp, now()::timestamp);", pg("functions-datetime.html"));
  f!("pg_get_function_arguments", "pg_get_function_arguments(function_oid regprocedure) -> text", "Re-render a function's argument signature, including modes / defaults.", "SELECT pg_get_function_arguments('my_fn(int)'::regprocedure);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_function_result", "pg_get_function_result(function_oid regprocedure) -> text", "Re-render a function's RETURNS clause.", "SELECT pg_get_function_result('my_fn(int)'::regprocedure);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));

  // ---- cycle2 round 72: misc string / array / enum / json ----
  f!("ord", "ord(text) -> int", "Unicode codepoint of the first character. PG17+; for older PGs use `ascii(left(s,1))`.", "SELECT ord('A');", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("to_hex", "to_hex(int|bigint) -> text", "Lowercase hex representation (no `0x` prefix).", "SELECT to_hex(255);", pg("functions-string.html#FUNCTIONS-STRING-OTHER"));
  f!("array_subscript", "array_subscript_handler -- internal type-specific subscript routing", "Internal type-support function plugged into custom-type subscript handlers (PG14+ subscriptable types).", "-- internal", pg("functions-array.html"));
  f!("hash_array", "hash_array(anyarray) -> int", "Internal hash of an array. Referenced by hash index/joins.", "-- internal", pg("functions-array.html"));
  f!("bit_count", "bit_count(bytea|bit) -> bigint", "Number of set bits (population count). PG14+.", "SELECT bit_count('\\xff'::bytea);", pg("functions-binarystring.html"));
  f!("array_sample", "array_sample(array anyarray, n int) -> anyarray", "Return n elements sampled without replacement (PG16+).", "SELECT array_sample(ARRAY[1,2,3,4,5], 2);", pg("functions-array.html#FUNCTIONS-ARRAY-TABLE"));
  f!("array_shuffle", "array_shuffle(anyarray) -> anyarray", "Return the array elements in random order (PG16+).", "SELECT array_shuffle(ARRAY[1,2,3,4,5]);", pg("functions-array.html#FUNCTIONS-ARRAY-TABLE"));
  f!("range_intersect_agg", "range_intersect_agg(anyrange) -> anyrange", "Aggregate the intersection of a set of ranges. PG14+.", "SELECT range_intersect_agg(period) FROM availability WHERE resource_id = 1;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("pg_get_catalog_foreign_keys", "pg_get_catalog_foreign_keys() -> setof record(fktable, fkcols, pktable, pkcols, is_array, is_opt)", "Hard-coded FK relationships between pg_catalog tables (PG14+).", "SELECT * FROM pg_get_catalog_foreign_keys();", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_partition_constraintdef", "pg_get_partition_constraintdef(oid) -> text", "Reconstruct the CHECK constraint implied by a partition's bounds.", "SELECT pg_get_partition_constraintdef('events_2025'::regclass::oid);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_partition_root", "pg_partition_root(regclass) -> regclass", "Return the topmost parent of a partitioned-table tree.", "SELECT pg_partition_root('events_2025'::regclass);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_partition_ancestors", "pg_partition_ancestors(regclass) -> setof regclass", "Each ancestor in the partitioning tree, root last.", "SELECT * FROM pg_partition_ancestors('events_2025'::regclass);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_partition_tree", "pg_partition_tree(regclass) -> setof record(relid, parentid, isleaf, level)", "Walk a partitioned table top-down.", "SELECT * FROM pg_partition_tree('events'::regclass);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  // ---- fuzzystrmatch (CREATE EXTENSION fuzzystrmatch) ----
  f!("levenshtein", "levenshtein(s1 text, s2 text [, ins_cost int, del_cost int, sub_cost int]) -> int", "Edit distance with optional per-op costs. Requires `CREATE EXTENSION fuzzystrmatch`.", "SELECT levenshtein('kitten', 'sitting');", pg("fuzzystrmatch.html#FUZZYSTRMATCH-LEVENSHTEIN"));
  f!("levenshtein_less_equal", "levenshtein_less_equal(s1, s2, max int) -> int", "Like levenshtein() but bails out early when distance would exceed `max`. Faster on similar inputs.", "SELECT levenshtein_less_equal('foo', 'fooz', 2);", pg("fuzzystrmatch.html#FUZZYSTRMATCH-LEVENSHTEIN"));
  f!("metaphone", "metaphone(text, max_output_length int) -> text", "Metaphone phonetic key for English words. Extension fuzzystrmatch.", "SELECT metaphone('Pittsburgh', 6);", pg("fuzzystrmatch.html#FUZZYSTRMATCH-METAPHONE"));
  f!("dmetaphone", "dmetaphone(text) -> text", "Double-Metaphone primary code (improved Metaphone). Extension fuzzystrmatch.", "SELECT dmetaphone('Pittsburgh');", pg("fuzzystrmatch.html#FUZZYSTRMATCH-DOUBLE-METAPHONE"));
  f!("dmetaphone_alt", "dmetaphone_alt(text) -> text", "Double-Metaphone alternate code (for ambiguous pronunciation). Extension fuzzystrmatch.", "SELECT dmetaphone_alt('Pittsburgh');", pg("fuzzystrmatch.html#FUZZYSTRMATCH-DOUBLE-METAPHONE"));
  f!("soundex", "soundex(text) -> text", "Soundex code (legacy phonetic). Extension fuzzystrmatch.", "SELECT soundex('Smith') = soundex('Smyth');", pg("fuzzystrmatch.html#FUZZYSTRMATCH-SOUNDEX"));
  f!("difference", "difference(a text, b text) -> int", "Soundex distance: 0..4. Extension fuzzystrmatch.", "SELECT difference('Smith', 'Smyth');", pg("fuzzystrmatch.html#FUZZYSTRMATCH-SOUNDEX"));
  // ---- unaccent (CREATE EXTENSION unaccent) ----
  f!("unaccent", "unaccent([dict regdictionary,] text) -> text", "Strip accents using a tsearch dictionary (default `unaccent`). Extension unaccent.", "SELECT unaccent('café');", pg("unaccent.html"));
  // ---- earthdistance / cube extension helpers (very common) ----
  f!("ll_to_earth", "ll_to_earth(lat double, lon double) -> earth", "Convert a lat/lon to the cube-based `earth` type. Requires `cube` + `earthdistance`.", "SELECT ll_to_earth(40.7, -74.0);", pg("earthdistance.html"));
  f!("earth_distance", "earth_distance(a earth, b earth) -> double precision", "Great-circle distance in meters. Requires cube + earthdistance.", "SELECT earth_distance(ll_to_earth(40.7,-74.0), ll_to_earth(34.0,-118.0));", pg("earthdistance.html"));
  // ---- pgcrypto helpers worth surfacing ----
  f!("crypt_check", "crypt(password, hash) -> text", "Verify by comparing a freshly-crypted password to a stored hash (constant-time-friendly). Pair with gen_salt.", "SELECT (hash = crypt('plaintext', hash)) AS ok FROM users WHERE id = 1;", pg("pgcrypto.html#PGCRYPTO-PASSWORD-HASHING"));
  // ---- PG18 reservoir/sampling ----
  f!("random_normal", "random_normal([mean double, stddev double]) -> double precision", "Normally-distributed random number (PG17+). Default mean 0, stddev 1.", "SELECT random_normal(0.0, 1.0);", pg("functions-math.html"));
  // ---- pgvector ----
  f!("cosine_distance", "cosine_distance(vector, vector) -> double precision", "1 - cosine similarity. Range [0, 2]. Requires `CREATE EXTENSION vector` (pgvector). Same as `<=>` operator.", "SELECT cosine_distance(embedding, '[0.1, 0.2, 0.3]') FROM docs ORDER BY 1 LIMIT 5;", pg("https://github.com/pgvector/pgvector"));
  f!("l2_distance", "l2_distance(vector, vector) -> double precision", "Euclidean distance. Same as `<->` operator. pgvector.", "SELECT l2_distance(embedding, query) FROM docs ORDER BY 1 LIMIT 5;", pg("https://github.com/pgvector/pgvector"));
  f!("inner_product", "inner_product(vector, vector) -> double precision", "Dot product. Negated form (`<#>`) sorts low->high for KNN. pgvector.", "SELECT inner_product(a, b) FROM vecs;", pg("https://github.com/pgvector/pgvector"));
  f!("l1_distance", "l1_distance(vector, vector) -> double precision", "Taxicab (Manhattan) distance. pgvector 0.5+.", "SELECT l1_distance(a, b) FROM vecs;", pg("https://github.com/pgvector/pgvector"));
  f!("vector_dims", "vector_dims(vector) -> int", "Dimensionality of the vector.", "SELECT vector_dims(embedding) FROM docs LIMIT 1;", pg("https://github.com/pgvector/pgvector"));
  f!("vector_norm", "vector_norm(vector) -> double precision", "L2 norm (magnitude). Use to normalize.", "SELECT embedding / vector_norm(embedding) FROM docs;", pg("https://github.com/pgvector/pgvector"));
  f!("hamming_distance", "hamming_distance(bit, bit) -> int", "Bit-wise hamming distance. pgvector also exposes a vector variant.", "SELECT hamming_distance('1010', '1100');", pg("https://github.com/pgvector/pgvector"));
  f!("jaccard_distance", "jaccard_distance(bit, bit) -> double precision", "1 - Jaccard similarity. pgvector for binary vectors.", "SELECT jaccard_distance('1010', '1100');", pg("https://github.com/pgvector/pgvector"));
  // ---- hstore (CREATE EXTENSION hstore) ----
  f!("akeys", "akeys(hstore) -> text[]", "Return all keys as a text array (sorted by storage order).", "SELECT akeys('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("avals", "avals(hstore) -> text[]", "Return all values as a text array.", "SELECT avals('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("skeys", "skeys(hstore) -> setof text", "Return keys as a row set.", "SELECT * FROM skeys('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("svals", "svals(hstore) -> setof text", "Return values as a row set.", "SELECT * FROM svals('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("hstore_to_json", "hstore_to_json(hstore) -> json", "Convert hstore to JSON.", "SELECT hstore_to_json('a=>1'::hstore);", pg("hstore.html"));
  f!("hstore_to_jsonb", "hstore_to_jsonb(hstore) -> jsonb", "Convert hstore to JSONB (faster for storage/queries).", "SELECT hstore_to_jsonb('a=>1'::hstore);", pg("hstore.html"));
  f!("hstore_to_array", "hstore_to_array(hstore) -> text[]", "Flatten hstore to {k1, v1, k2, v2, ...} text array.", "SELECT hstore_to_array('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("hstore_to_matrix", "hstore_to_matrix(hstore) -> text[][]", "Convert hstore to a 2-dim array {{k,v},{k,v}}.", "SELECT hstore_to_matrix('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("populate_record", "populate_record(base anyelement, hstore) -> anyelement", "Cast hstore into a record/row type, using `base` for shape.", "SELECT populate_record(NULL::users, 'id=>1,name=>x'::hstore);", pg("hstore.html"));
  f!("each_hstore", "each(hstore) -> setof record(key text, value text)", "Decompose hstore into (key, value) rows.", "SELECT * FROM each('a=>1,b=>2'::hstore);", pg("hstore.html"));
  f!("exist", "exist(hstore, key text) -> boolean", "True when the hstore has the given key. Same as `?` operator.", "SELECT exist(meta, 'priority') FROM jobs;", pg("hstore.html"));
  f!("defined", "defined(hstore, key text) -> boolean", "True when key exists AND its value is not NULL.", "SELECT defined(meta, 'tag') FROM jobs;", pg("hstore.html"));
  // ---- pg_stat_statements monitoring helper ----
  f!("pg_stat_statements", "pg_stat_statements [(showtext boolean)] -> setof record(queryid, userid, dbid, calls, total_exec_time, mean_exec_time, rows, ...)", "Per-statement execution stats. Requires `CREATE EXTENSION pg_stat_statements` and `shared_preload_libraries`.", "SELECT queryid, calls, mean_exec_time FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 10;", pg("pgstatstatements.html"));
  f!("pg_stat_statements_reset", "pg_stat_statements_reset([userid oid, dbid oid, queryid bigint]) -> setof bigint", "Reset stats for all entries, or just matching subset (PG14+).", "SELECT pg_stat_statements_reset();", pg("pgstatstatements.html"));
  f!("pg_stat_statements_info", "pg_stat_statements_info() -> record(dealloc bigint, stats_reset timestamptz)", "Sidecar stats for pg_stat_statements itself (PG14+).", "SELECT * FROM pg_stat_statements_info();", pg("pgstatstatements.html"));
  // ---- PostGIS basic geometry constructors ----
  f!("st_geomfromtext", "ST_GeomFromText(wkt text [, srid int]) -> geometry", "Build a geometry from Well-Known Text. PostGIS.", "SELECT ST_GeomFromText('POINT(-71.064544 42.28787)', 4326);", pg("https://postgis.net/docs/ST_GeomFromText.html"));
  f!("st_geomfromewkt", "ST_GeomFromEWKT(text) -> geometry", "EWKT variant that includes the SRID inline. PostGIS.", "SELECT ST_GeomFromEWKT('SRID=4326;POINT(-71 42)');", pg("https://postgis.net/docs/ST_GeomFromEWKT.html"));
  f!("st_geomfromwkb", "ST_GeomFromWKB(bytea [, srid int]) -> geometry", "Build a geometry from Well-Known Binary. PostGIS.", "SELECT ST_GeomFromWKB(decode('...', 'hex'), 4326);", pg("https://postgis.net/docs/ST_GeomFromWKB.html"));
  f!("st_point", "ST_Point(x double, y double [, srid int]) -> geometry", "Build a point. PostGIS 3.2+ adds the srid form; older versions need ST_SetSRID.", "SELECT ST_Point(-71.0, 42.0, 4326);", pg("https://postgis.net/docs/ST_Point.html"));
  f!("st_makepoint", "ST_MakePoint(x double, y double [, z, m]) -> geometry", "Same as ST_Point pre-3.2.", "SELECT ST_MakePoint(-71.0, 42.0);", pg("https://postgis.net/docs/ST_MakePoint.html"));
  f!("st_setsrid", "ST_SetSRID(geom geometry, srid int) -> geometry", "Stamp an SRID onto a geometry (no reprojection). PostGIS.", "SELECT ST_SetSRID(ST_MakePoint(-71, 42), 4326);", pg("https://postgis.net/docs/ST_SetSRID.html"));
  f!("st_transform", "ST_Transform(geom geometry, to_srid int) -> geometry", "Reproject between coordinate systems. PostGIS.", "SELECT ST_Transform(geom, 3857) FROM places;", pg("https://postgis.net/docs/ST_Transform.html"));
  // ---- PostGIS introspection ----
  f!("st_astext", "ST_AsText(geom geometry [, maxdecimals int]) -> text", "Format geometry as WKT. PostGIS.", "SELECT ST_AsText(geom) FROM places LIMIT 1;", pg("https://postgis.net/docs/ST_AsText.html"));
  f!("st_asewkt", "ST_AsEWKT(geom geometry) -> text", "WKT with SRID prefix. PostGIS.", "SELECT ST_AsEWKT(geom) FROM places LIMIT 1;", pg("https://postgis.net/docs/ST_AsEWKT.html"));
  f!("st_asgeojson", "ST_AsGeoJSON(geom geometry [, maxdecimals int, options int]) -> text", "Format as GeoJSON. PostGIS.", "SELECT ST_AsGeoJSON(geom, 6) FROM places;", pg("https://postgis.net/docs/ST_AsGeoJSON.html"));
  f!("st_srid", "ST_SRID(geom geometry) -> int", "Spatial reference system id. 0 means unspecified.", "SELECT ST_SRID(geom) FROM places LIMIT 1;", pg("https://postgis.net/docs/ST_SRID.html"));
  f!("st_geometrytype", "ST_GeometryType(geom geometry) -> text", "'ST_Point', 'ST_LineString', etc. Prefixed.", "SELECT ST_GeometryType(geom) FROM places;", pg("https://postgis.net/docs/ST_GeometryType.html"));
  // ---- PostGIS spatial predicates ----
  f!("st_within", "ST_Within(a geometry, b geometry) -> boolean", "True when a is completely inside b. PostGIS. Indexed by GiST when the args have spatial indexes.", "SELECT * FROM places p WHERE ST_Within(p.geom, ST_GeomFromText('POLYGON(...)', 4326));", pg("https://postgis.net/docs/ST_Within.html"));
  f!("st_contains", "ST_Contains(a, b geometry) -> boolean", "Mirror of ST_Within. PostGIS.", "SELECT ST_Contains(poly, point) FROM ...;", pg("https://postgis.net/docs/ST_Contains.html"));
  f!("st_intersects", "ST_Intersects(a, b geometry) -> boolean", "True when geometries share at least one point. PostGIS.", "SELECT * FROM roads WHERE ST_Intersects(geom, ST_Buffer(town_geom, 100));", pg("https://postgis.net/docs/ST_Intersects.html"));
  f!("st_disjoint", "ST_Disjoint(a, b geometry) -> boolean", "Mirror of ST_Intersects.", "SELECT ST_Disjoint(a, b) FROM ...;", pg("https://postgis.net/docs/ST_Disjoint.html"));
  f!("st_dwithin", "ST_DWithin(a, b geometry, distance double precision [, use_spheroid boolean]) -> boolean", "True when geometries are within `distance` meters. Crucial for KNN/radius queries. PostGIS.", "SELECT * FROM places WHERE ST_DWithin(geom::geography, ST_MakePoint(lon, lat)::geography, 1000);", pg("https://postgis.net/docs/ST_DWithin.html"));
  f!("st_distance", "ST_Distance(a, b geometry) -> double precision | ST_Distance(a, b geography [, use_spheroid]) -> double precision (meters)", "Shortest distance between geometries.", "SELECT ST_Distance(a, b) FROM pairs;", pg("https://postgis.net/docs/ST_Distance.html"));
  f!("st_area", "ST_Area(geom geometry) -> double precision | ST_Area(geog geography) -> double precision (m²)", "Surface area. Casting to geography gives accurate metric results.", "SELECT ST_Area(geom::geography) FROM polygons;", pg("https://postgis.net/docs/ST_Area.html"));
  f!("st_length", "ST_Length(geom geometry) -> double precision | (geography) -> meters", "Total length of linestrings.", "SELECT ST_Length(geom::geography) FROM roads;", pg("https://postgis.net/docs/ST_Length.html"));
  f!("st_buffer", "ST_Buffer(geom geometry, distance double [, quad_segs int | options text]) -> geometry", "Expand geometry by `distance` (in units of SRID, or meters when cast to geography).", "SELECT ST_Buffer(point::geography, 1000) FROM places;", pg("https://postgis.net/docs/ST_Buffer.html"));
  f!("st_centroid", "ST_Centroid(geom geometry) -> geometry", "Geometric centroid (center of mass).", "SELECT ST_Centroid(geom) FROM polygons;", pg("https://postgis.net/docs/ST_Centroid.html"));
  f!("st_makeenvelope", "ST_MakeEnvelope(xmin double, ymin double, xmax double, ymax double [, srid int]) -> geometry", "Build a rectangular polygon (bounding box).", "SELECT ST_MakeEnvelope(-180, -90, 180, 90, 4326);", pg("https://postgis.net/docs/ST_MakeEnvelope.html"));
  // ---- pg_partman ----
  f!("create_parent", "partman.create_parent(p_parent_table text, p_control text, p_type text, p_interval text, ...) -> boolean", "Bootstrap a partition set + its background trigger. pg_partman.", "SELECT partman.create_parent('public.events', 'created_at', 'native', 'monthly');", pg("https://github.com/pgpartman/pg_partman"));
  f!("run_maintenance", "partman.run_maintenance([p_parent_table text]) -> void", "Create new partitions ahead of time, drop old ones. Called by cron. pg_partman.", "SELECT partman.run_maintenance('public.events');", pg("https://github.com/pgpartman/pg_partman"));
  f!("partition_data_time", "partman.partition_data_time(p_parent_table text, p_batch_count int, ...) -> bigint", "Move rows from a default partition into the right child. pg_partman.", "SELECT partman.partition_data_time('public.events');", pg("https://github.com/pgpartman/pg_partman"));
  // ---- pg_cron ----
  f!("cron_schedule", "cron.schedule(job_name text, schedule text, command text) -> bigint", "Schedule a SQL command on a 5-field cron string. Returns the new job id. pg_cron.", "SELECT cron.schedule('clean-old-events', '0 3 * * *', $$ DELETE FROM events WHERE ts < now() - INTERVAL '90 days' $$);", pg("https://github.com/citusdata/pg_cron"));
  f!("cron_unschedule", "cron.unschedule(jobid bigint | job_name text) -> boolean", "Cancel a scheduled job. pg_cron.", "SELECT cron.unschedule('clean-old-events');", pg("https://github.com/citusdata/pg_cron"));
  f!("cron_schedule_in_database", "cron.schedule_in_database(job_name text, schedule text, command text, database text, [user text, active boolean]) -> bigint", "Schedule a job that runs against a specific database. pg_cron.", "SELECT cron.schedule_in_database('analytics-refresh', '0 * * * *', 'REFRESH MATERIALIZED VIEW mv', 'analytics');", pg("https://github.com/citusdata/pg_cron"));
  // ---- TimescaleDB hypertables ----
  f!("create_hypertable", "create_hypertable(relation regclass, time_column_name name [, chunk_time_interval interval, ...]) -> record", "Convert a regular table into a TimescaleDB hypertable; partitions on time. Many optional knobs (space partitioning, migrate_data).", "SELECT create_hypertable('events', 'ts', chunk_time_interval => INTERVAL '1 day');", pg("https://docs.timescale.com/api/latest/hypertable/create_hypertable/"));
  f!("add_dimension", "add_dimension(hypertable regclass, column_name name [, number_partitions int | chunk_time_interval interval]) -> record", "Add an additional partitioning dimension (e.g. space partition).", "SELECT add_dimension('events', 'region_id', number_partitions => 4);", pg("https://docs.timescale.com/api/latest/hypertable/add_dimension/"));
  f!("add_compression_policy", "add_compression_policy(hypertable regclass, compress_after interval, [if_not_exists boolean]) -> int", "Auto-compress chunks older than `compress_after`. TimescaleDB.", "SELECT add_compression_policy('events', INTERVAL '30 days');", pg("https://docs.timescale.com/api/latest/compression/add_compression_policy/"));
  f!("add_retention_policy", "add_retention_policy(hypertable regclass, drop_after interval, [if_not_exists boolean]) -> int", "Auto-drop chunks older than `drop_after`. TimescaleDB.", "SELECT add_retention_policy('events', INTERVAL '1 year');", pg("https://docs.timescale.com/api/latest/data_retention/add_retention_policy/"));
  f!("add_continuous_aggregate_policy", "add_continuous_aggregate_policy(cagg regclass, start_offset interval, end_offset interval, schedule_interval interval) -> int", "Schedule incremental refresh of a continuous aggregate (materialized view of time bucket aggregates).", "SELECT add_continuous_aggregate_policy('events_5m', INTERVAL '1 month', INTERVAL '5 minutes', INTERVAL '5 minutes');", pg("https://docs.timescale.com/api/latest/continuous-aggregates/add_continuous_aggregate_policy/"));
  f!("time_bucket", "time_bucket(bucket_width interval, ts timestamp [, origin timestamp]) -> timestamp", "Snap timestamp to the nearest bucket. TimescaleDB.", "SELECT time_bucket('5 minutes', ts), avg(value) FROM metrics GROUP BY 1;", pg("https://docs.timescale.com/api/latest/hyperfunctions/time_bucket/"));
  f!("time_bucket_gapfill", "time_bucket_gapfill(bucket_width interval, ts timestamp, start timestamp, finish timestamp) -> timestamp", "time_bucket + gap-fill missing buckets in [start, finish] range. TimescaleDB.", "SELECT time_bucket_gapfill('5 minutes', ts, now() - INTERVAL '1 hour', now()), locf(avg(value)) FROM metrics GROUP BY 1;", pg("https://docs.timescale.com/api/latest/hyperfunctions/gapfilling/time_bucket_gapfill/"));
  f!("show_chunks", "show_chunks(hypertable regclass [, older_than interval, newer_than interval]) -> setof regclass", "List chunk OIDs of a hypertable.", "SELECT show_chunks('events', older_than => INTERVAL '7 days');", pg("https://docs.timescale.com/api/latest/hypertable/show_chunks/"));
  f!("drop_chunks", "drop_chunks(hypertable regclass, older_than interval) -> setof text", "Drop chunks older than the cutoff. TimescaleDB.", "SELECT drop_chunks('events', INTERVAL '1 year');", pg("https://docs.timescale.com/api/latest/data_retention/drop_chunks/"));
  f!("locf", "locf(value anyelement) -> anyelement", "Last-Observation-Carried-Forward fill; pair with time_bucket_gapfill. TimescaleDB.", "SELECT time_bucket_gapfill('5m', ts, ...), locf(avg(value)) FROM metrics GROUP BY 1;", pg("https://docs.timescale.com/api/latest/hyperfunctions/gapfilling/locf/"));
  f!("interpolate", "interpolate(value anyelement) -> double precision", "Linear interpolation of gap values. TimescaleDB.", "SELECT time_bucket_gapfill('5m', ts, ...), interpolate(avg(value)) FROM metrics GROUP BY 1;", pg("https://docs.timescale.com/api/latest/hyperfunctions/gapfilling/interpolate/"));
  f!("first", "first(value anyelement, time timestamp) -> anyelement", "Return `value` from the row with the smallest `time`. TimescaleDB.", "SELECT first(metric, ts) FROM events;", pg("https://docs.timescale.com/api/latest/hyperfunctions/first/"));
  f!("last", "last(value anyelement, time timestamp) -> anyelement", "Return `value` from the row with the largest `time`. TimescaleDB.", "SELECT last(metric, ts) FROM events;", pg("https://docs.timescale.com/api/latest/hyperfunctions/last/"));
  // ---- Citus distribution helpers ----
  f!("create_distributed_table", "create_distributed_table(table_name regclass, distribution_column text [, distribution_type text, colocate_with text]) -> void", "Shard a table by the chosen column. Citus.", "SELECT create_distributed_table('events', 'user_id');", pg("https://docs.citusdata.com/en/stable/develop/api_udf.html#create-distributed-table"));
  f!("create_reference_table", "create_reference_table(table_name regclass) -> void", "Replicate a small reference table to every worker. Citus.", "SELECT create_reference_table('countries');", pg("https://docs.citusdata.com/en/stable/develop/api_udf.html#create-reference-table"));
  f!("citus_add_node", "citus_add_node(node_name text, node_port int [, node_group int, ...]) -> int", "Register a new Citus worker.", "SELECT citus_add_node('worker-3', 5432);", pg("https://docs.citusdata.com/en/stable/admin_guide/cluster_management.html"));
  f!("citus_remove_node", "citus_remove_node(node_name text, node_port int) -> void", "Deregister a worker. Drains shards first.", "SELECT citus_remove_node('worker-3', 5432);", pg("https://docs.citusdata.com/en/stable/admin_guide/cluster_management.html"));
  f!("citus_shards", "citus_shards -- view exposing shard placement / size.", "SELECT * FROM citus_shards ORDER BY shard_size DESC LIMIT 5;", "SELECT * FROM citus_shards LIMIT 5;", pg("https://docs.citusdata.com/en/stable/admin_guide/cluster_management.html"));
  f!("citus_rebalance_start", "citus_rebalance_start([rebalance_strategy text, drain_only boolean, shard_transfer_mode text]) -> bigint", "Kick off background rebalance. Citus.", "SELECT citus_rebalance_start();", pg("https://docs.citusdata.com/en/stable/admin_guide/cluster_management.html"));
  // ---- PostgresML inference ----
  f!("pgml_predict", "pgml.predict(project text, features ...) -> double precision | anyarray", "Run inference using a registered model. PostgresML.", "SELECT pgml.predict('churn_model', user_features) FROM users;", pg("https://postgresml.org/docs"));
  f!("pgml_embed", "pgml.embed(transformer text, text text [, kwargs jsonb]) -> real[]", "Compute a text embedding using a HuggingFace model. PostgresML.", "SELECT pgml.embed('intfloat/e5-small', 'hello world');", pg("https://postgresml.org/docs"));
  f!("pgml_chat", "pgml.transform(task jsonb | text, inputs text[]) -> jsonb", "Run a HuggingFace pipeline (chat, summarization, classification). PostgresML.", "SELECT pgml.transform('summarization', ARRAY['<long text>']);", pg("https://postgresml.org/docs"));
  f!("bit_xor", "bit_xor(int|bigint|bit) -> same", "Aggregate: bitwise XOR across rows. PG14+.", "SELECT bit_xor(flags) FROM events;", pg("functions-aggregate.html#FUNCTIONS-AGGREGATE-TABLE"));
  f!("enum_first", "enum_first(anyenum) -> anyenum", "First label of an enum type. Pass `NULL::<enum_t>` to specify type.", "SELECT enum_first(NULL::mood);", pg("functions-enum.html"));
  f!("enum_last", "enum_last(anyenum) -> anyenum", "Last label of an enum type.", "SELECT enum_last(NULL::mood);", pg("functions-enum.html"));
  f!("enum_range", "enum_range([from anyenum,] [to anyenum]) -> anyenum[]", "All labels of an enum (with optional inclusive bounds).", "SELECT enum_range('sad'::mood, 'happy'::mood);", pg("functions-enum.html"));
  f!("json_array", "JSON_ARRAY(<expr>[, ...]) -> json", "SQL:2023 array constructor (PG16+).", "SELECT JSON_ARRAY(1, 'two', true);", pg("functions-json.html#FUNCTIONS-SQLJSON-CREATION"));
  f!("json_object", "JSON_OBJECT(<key>: <val>[, ...] [ABSENT|NULL ON NULL] [UNIQUE]) -> json", "SQL:2023 object constructor (PG16+).", "SELECT JSON_OBJECT('id': 1, 'name': 'a');", pg("functions-json.html#FUNCTIONS-SQLJSON-CREATION"));
  f!("json_scalar", "JSON_SCALAR(<expr>) -> json", "SQL:2023 scalar constructor; promotes value to JSON.", "SELECT JSON_SCALAR(42);", pg("functions-json.html#FUNCTIONS-SQLJSON-CREATION"));
  f!("json_serialize", "JSON_SERIALIZE(<json> [RETURNING <type>] [PRETTY]) -> text|bytea", "SQL:2023 JSON_SERIALIZE -- render a jsonb back to text.", "SELECT JSON_SERIALIZE('{\"a\":1}'::jsonb RETURNING text PRETTY);", pg("functions-json.html#FUNCTIONS-SQLJSON-PROCESSING"));
  f!("jsonb_insert", "jsonb_insert(target jsonb, path text[], new_value jsonb [, insert_after boolean]) -> jsonb", "Insert into nested location at `path`. `insert_after` (default false) controls order around an existing array index.", "SELECT jsonb_insert('[1,2,3]'::jsonb, '{1}', '99'::jsonb, false);", pg("functions-json.html#FUNCTIONS-JSON-PROCESSING-TABLE"));

  // ---- cycle2 round 73: remaining admin / xact / signal helpers ----
  f!("pg_xact_status", "pg_xact_status(xid8) -> text", "Status of a transaction id: 'in progress' / 'committed' / 'aborted'. PG10+.", "SELECT pg_xact_status(pg_current_xact_id());", pg("functions-info.html#FUNCTIONS-INFO-SNAPSHOT"));
  f!("pg_current_xact_id", "pg_current_xact_id() -> xid8", "Current transaction id (8-byte). Assigns one if none yet.", "SELECT pg_current_xact_id();", pg("functions-info.html#FUNCTIONS-INFO-SNAPSHOT"));
  f!("pg_log_standby_snapshot", "pg_log_standby_snapshot() -> pg_lsn", "Trigger a standby snapshot WAL record; used to let logical replication slots advance on a quiet system. PG14+.", "SELECT pg_log_standby_snapshot();", pg("functions-admin.html#FUNCTIONS-ADMIN-SIGNALING"));
  f!("pg_trigger_depth", "pg_trigger_depth() -> int", "Current nesting depth of trigger invocations (0 outside triggers, 1 inside the outermost).", "SELECT pg_trigger_depth();", pg("functions-info.html#FUNCTIONS-INFO-SESSION"));
  f!("pg_log_query_plan", "pg_log_query_plan(pid int) -> boolean", "Ask another backend to log its currently-executing query plan. PG16+.", "SELECT pg_log_query_plan(12345);", pg("functions-admin.html#FUNCTIONS-ADMIN-SIGNALING"));
  f!("pg_stat_have_stats", "pg_stat_have_stats(stat_kind text, objoid oid, subobjoid int) -> boolean", "Ask whether a stats entry exists for a given object. PG15+.", "SELECT pg_stat_have_stats('relation', 'users'::regclass::oid, 0);", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_stat_reset_subscription_stats", "pg_stat_reset_subscription_stats(subid oid) -> void", "Reset cumulative subscription stats counters. PG15+.", "SELECT pg_stat_reset_subscription_stats(NULL); -- all subscriptions", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_stat_reset_slru", "pg_stat_reset_slru(target text) -> void", "Reset SLRU cache stats counters (CommitTs, MultiXactMember, ...). PG13+.", "SELECT pg_stat_reset_slru(NULL); -- all SLRUs", pg("monitoring-stats.html#MONITORING-STATS-FUNCTIONS"));
  f!("pg_settings_get_flags", "pg_settings_get_flags(guc_name text) -> text[]", "Introspect categorical flags on a GUC (NO_SHOW_ALL, NO_RESET_ALL, NOT_IN_SAMPLE, ...). PG15+.", "SELECT pg_settings_get_flags('work_mem');", pg("functions-admin.html#FUNCTIONS-ADMIN-SET"));
  f!("pg_split_walfile_name", "pg_split_walfile_name(name text) -> record", "Decompose a WAL filename into (segment_number, timeline_id). PG17+.", "SELECT * FROM pg_split_walfile_name('000000010000000000000001');", pg("functions-admin.html#FUNCTIONS-ADMIN-BACKUP-TABLE"));
  f!("pg_get_acl", "pg_get_acl(classid oid, objid oid, objsubid int) -> aclitem[]", "Return the ACL of any cataloged object as the raw aclitem array. PG17+.", "SELECT pg_get_acl('pg_class'::regclass::oid, 'users'::regclass::oid, 0);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_get_loaded_modules", "pg_get_loaded_modules() -> setof record", "List of currently loaded shared libraries (name, version, modules). PG17+.", "SELECT * FROM pg_get_loaded_modules();", pg("functions-admin.html#FUNCTIONS-ADMIN-DBOBJECT"));
  f!("pg_basetype", "pg_basetype(regtype) -> regtype", "Strip domain wrappers and return the underlying base type. PG17+.", "SELECT pg_basetype('positive_int'::regtype);", pg("functions-info.html#FUNCTIONS-INFO-CATALOG"));
  f!("pg_input_error_message", "pg_input_error_message(input text, type text) -> text", "Like `pg_input_is_valid` but returns the error text on failure (NULL on success). PG16+.", "SELECT pg_input_error_message('foo', 'int');", pg("functions-info.html#FUNCTIONS-INFO-VALIDITY"));
  f!("pg_signal_backend", "pg_signal_backend(pid int, sig text) -> boolean", "Lower-level cousin of `pg_cancel_backend`/`pg_terminate_backend`. Restricted by pg_signal_backend role.", "SELECT pg_signal_backend(12345, 'SIGINT');", pg("functions-admin.html#FUNCTIONS-ADMIN-SIGNALING"));
  f!("pg_xlog_replay_pause", "pg_xlog_replay_pause() -> void", "Legacy alias of `pg_wal_replay_pause` (PG9.x). Removed in newer versions; use `pg_wal_replay_pause`.", "SELECT pg_wal_replay_pause();", pg("functions-admin.html#FUNCTIONS-RECOVERY-CONTROL"));
  f!("pg_xlogfile_name", "pg_xlogfile_name(lsn pg_lsn) -> text", "Legacy alias of `pg_walfile_name`. Use `pg_walfile_name`.", "SELECT pg_walfile_name(pg_current_wal_lsn());", pg("functions-admin.html#FUNCTIONS-ADMIN-BACKUP-TABLE"));
  f!("inet_send", "inet_send(inet) -> bytea", "Type-output (binary) function for `inet`. Internal -- emitted by binary COPY/wire format.", "-- internal", pg("functions-net.html"));

  m
}
