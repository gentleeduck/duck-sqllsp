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

  // ---- Trigonometric ----
  f!("sin",   "sin(double precision) -> double precision", "Sine, radians.",   "SELECT sin(0);", pg("functions-math.html"));
  f!("cos",   "cos(double precision) -> double precision", "Cosine, radians.", "SELECT cos(0);", pg("functions-math.html"));
  f!("tan",   "tan(double precision) -> double precision", "Tangent, radians.", "SELECT tan(0);", pg("functions-math.html"));
  f!("asin",  "asin(double precision) -> double precision", "Arc sine, radians.",   "SELECT asin(1);", pg("functions-math.html"));
  f!("acos",  "acos(double precision) -> double precision", "Arc cosine, radians.", "SELECT acos(1);", pg("functions-math.html"));
  f!("atan",  "atan(double precision) -> double precision", "Arc tangent, radians.","SELECT atan(1);", pg("functions-math.html"));
  f!("atan2", "atan2(y double, x double) -> double precision", "Arc tangent of y/x, radians.", "SELECT atan2(1, 1);", pg("functions-math.html"));

  // ---- Bit aggregates ----
  f!("bit_and", "bit_and(integer) -> integer", "Bitwise AND of all non-null values.", "SELECT bit_and(flags) FROM perms;", pg("functions-aggregate.html"));
  f!("bit_or",  "bit_or(integer) -> integer",  "Bitwise OR of all non-null values.",  "SELECT bit_or(flags) FROM perms;", pg("functions-aggregate.html"));
  f!("bit_xor", "bit_xor(integer) -> integer", "Bitwise XOR of all non-null values (PG14+).", "SELECT bit_xor(checksum) FROM blocks;", pg("functions-aggregate.html"));

  // ---- Stats aggregates ----
  f!("corr",       "corr(y double, x double) -> double precision", "Correlation coefficient.", "SELECT corr(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("covar_pop",  "covar_pop(y, x) -> double precision",  "Population covariance.", "SELECT covar_pop(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("covar_samp", "covar_samp(y, x) -> double precision", "Sample covariance.",     "SELECT covar_samp(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("stddev_pop", "stddev_pop(numeric) -> numeric", "Population standard deviation.", "SELECT stddev_pop(grade) FROM tests;", pg("functions-aggregate.html"));
  f!("stddev_samp","stddev_samp(numeric) -> numeric", "Sample standard deviation.",     "SELECT stddev_samp(grade) FROM tests;", pg("functions-aggregate.html"));
  f!("var_pop",    "var_pop(numeric) -> numeric", "Population variance.", "SELECT var_pop(grade) FROM tests;", pg("functions-aggregate.html"));
  f!("var_samp",   "var_samp(numeric) -> numeric", "Sample variance.",     "SELECT var_samp(grade) FROM tests;", pg("functions-aggregate.html"));

  // ---- Full-text search ----
  f!("to_tsvector",        "to_tsvector([config regconfig,] text) -> tsvector", "Convert text to a tsvector.", "SELECT to_tsvector('english', 'duck typing');", pg("textsearch-controls.html"));
  f!("to_tsquery",         "to_tsquery([config,] text) -> tsquery", "Convert query text to tsquery.",          "SELECT to_tsquery('english', 'duck & typing');", pg("textsearch-controls.html"));
  f!("plainto_tsquery",    "plainto_tsquery([config,] text) -> tsquery", "Plain phrase -> tsquery (no operators).", "SELECT plainto_tsquery('duck typing');", pg("textsearch-controls.html"));
  f!("phraseto_tsquery",   "phraseto_tsquery([config,] text) -> tsquery", "Phrase tsquery using `<->`.", "SELECT phraseto_tsquery('duck typing');", pg("textsearch-controls.html"));
  f!("websearch_to_tsquery","websearch_to_tsquery([config,] text) -> tsquery", "Web-search-style tsquery (quotes, OR, -term).", "SELECT websearch_to_tsquery('\"duck typing\" OR rust');", pg("textsearch-controls.html"));
  f!("ts_rank",            "ts_rank(tsvector, tsquery) -> real",    "Rank a document against a query.", "SELECT ts_rank(doc, q) FROM ...", pg("textsearch-controls.html"));
  f!("ts_rank_cd",         "ts_rank_cd(tsvector, tsquery) -> real", "Cover density rank.",              "SELECT ts_rank_cd(doc, q) FROM ...", pg("textsearch-controls.html"));
  f!("ts_headline",        "ts_headline([config,] text, tsquery) -> text", "Snippet with matched terms highlighted.", "SELECT ts_headline('the duck quacks', q);", pg("textsearch-controls.html"));

  // ---- JSONB path (SQL/JSON) ----
  f!("jsonb_path_exists",       "jsonb_path_exists(jsonb, jsonpath) -> boolean", "True if any item matches the path.", "SELECT jsonb_path_exists(doc, '$.a.b ? (@ > 0)');", pg("functions-json.html"));
  f!("jsonb_path_match",        "jsonb_path_match(jsonb, jsonpath) -> boolean",  "Match path returning a single boolean.", "SELECT jsonb_path_match(doc, 'exists($.x)');", pg("functions-json.html"));
  f!("jsonb_path_query_first",  "jsonb_path_query_first(jsonb, jsonpath) -> jsonb", "First matching item or NULL.", "SELECT jsonb_path_query_first(doc, '$.items[0]');", pg("functions-json.html"));
  f!("jsonb_path_query_array",  "jsonb_path_query_array(jsonb, jsonpath) -> jsonb", "All matches packed into a jsonb array.", "SELECT jsonb_path_query_array(doc, '$.items[*]');", pg("functions-json.html"));
  f!("jsonb_insert", "jsonb_insert(target jsonb, path text[], new jsonb [, insert_after bool]) -> jsonb", "Insert a value at a jsonb path.", "SELECT jsonb_insert('{\"a\":[1]}'::jsonb, '{a,1}', '2');", pg("functions-json.html"));

  // ---- Object lookups (regclass et al.) ----
  f!("to_regclass",     "to_regclass(text) -> regclass", "OID lookup; NULL when missing (vs `::regclass` which errors).", "SELECT to_regclass('public.users');", pg("functions-info.html#FUNCTIONS-INFO-OBJECT"));
  f!("to_regproc",      "to_regproc(text) -> regproc",   "OID lookup for a function name.", "SELECT to_regproc('lower');", pg("functions-info.html#FUNCTIONS-INFO-OBJECT"));
  f!("to_regtype",      "to_regtype(text) -> regtype",   "OID lookup for a type name.",     "SELECT to_regtype('int4');", pg("functions-info.html#FUNCTIONS-INFO-OBJECT"));
  f!("to_regnamespace", "to_regnamespace(text) -> regnamespace", "OID lookup for a schema.", "SELECT to_regnamespace('public');", pg("functions-info.html#FUNCTIONS-INFO-OBJECT"));
  f!("to_regrole",      "to_regrole(text) -> regrole",   "OID lookup for a role.",          "SELECT to_regrole('postgres');", pg("functions-info.html#FUNCTIONS-INFO-OBJECT"));
  f!("pg_get_userbyid", "pg_get_userbyid(oid) -> name",  "Role name for OID.",              "SELECT pg_get_userbyid(10);", pg("functions-info.html"));
  f!("pg_get_serial_sequence", "pg_get_serial_sequence(table_name, column_name) -> text", "Sequence backing a SERIAL/IDENTITY column.", "SELECT pg_get_serial_sequence('users', 'id');", pg("functions-info.html"));

  // ---- Size & stats ----
  f!("pg_table_size",   "pg_table_size(regclass) -> bigint",  "On-disk size of a table (excluding indexes, TOAST sums separately).", "SELECT pg_size_pretty(pg_table_size('users'));", pg("functions-admin.html"));
  f!("pg_indexes_size", "pg_indexes_size(regclass) -> bigint","Total size of all indexes attached to a relation.", "SELECT pg_size_pretty(pg_indexes_size('users'));", pg("functions-admin.html"));
  f!("pg_relation_size","pg_relation_size(regclass [, fork]) -> bigint", "Size of one fork of a relation.", "SELECT pg_relation_size('users');", pg("functions-admin.html"));
  f!("pg_total_relation_size", "pg_total_relation_size(regclass) -> bigint", "Total disk usage of a relation including indexes + TOAST.", "SELECT pg_size_pretty(pg_total_relation_size('users'));", pg("functions-admin.html"));
  f!("pg_database_size","pg_database_size(name) -> bigint",   "Total disk usage of a database.", "SELECT pg_size_pretty(pg_database_size('app'));", pg("functions-admin.html"));

  // ---- Enums / arrays ----
  f!("enum_first", "enum_first(anyenum) -> anyenum", "First label of an enum.",  "SELECT enum_first(NULL::status);", pg("functions-enum.html"));
  f!("enum_last",  "enum_last(anyenum) -> anyenum",  "Last label of an enum.",   "SELECT enum_last(NULL::status);", pg("functions-enum.html"));
  f!("enum_range", "enum_range([anyenum [, anyenum]]) -> anyarray", "Array of enum labels in declared order.", "SELECT enum_range(NULL::status);", pg("functions-enum.html"));
  f!("array_fill", "array_fill(anyelement, int[] [, int[]]) -> anyarray", "Create an array filled with copies of one value.", "SELECT array_fill(0, ARRAY[3]);", pg("functions-array.html"));

  // ---- Misc heavy traffic ----
  f!("concat_ws", "concat_ws(sep text, args...) -> text", "Concatenate non-null args with separator.", "SELECT concat_ws(', ', first, middle, last);", pg("functions-string.html"));
  f!("to_regoperator", "to_regoperator(text) -> regoperator", "OID lookup for an operator with operand types.", "SELECT to_regoperator('=(int,int)');", pg("functions-info.html#FUNCTIONS-INFO-OBJECT"));
  f!("pg_current_xact_id", "pg_current_xact_id() -> xid8", "Current transaction's xid8 (read-write).", "SELECT pg_current_xact_id();", pg("functions-info.html"));
  f!("pg_xact_status",     "pg_xact_status(xid8) -> text",  "Status of a transaction: committed / in progress / aborted.", "SELECT pg_xact_status(pg_current_xact_id());", pg("functions-info.html"));

  // ---- Comparison / NULL counting ----
  // Window-only rank functions (siblings of row_number / rank already
  // listed above).
  f!("percent_rank", "percent_rank() -> double precision", "Relative rank of the current row (0..1), excluding the current row's peers.", "SELECT percent_rank() OVER (ORDER BY salary) FROM employees;", pg("functions-window.html"));
  f!("cume_dist",    "cume_dist() -> double precision",    "Cumulative distribution of the current row -- fraction of partition rows with values <= current.", "SELECT cume_dist() OVER (ORDER BY salary) FROM employees;", pg("functions-window.html"));

  f!("num_nonnulls", "num_nonnulls(VARIADIC \"any\") -> int", "Count of non-NULL arguments. Useful in CHECK constraints to require exactly one of N columns.", "CHECK (num_nonnulls(promo_id, voucher_id) = 1)", pg("functions-comparison.html#FUNCTIONS-COMPARISON-FUNC-TABLE"));
  f!("num_nulls",    "num_nulls(VARIADIC \"any\") -> int",    "Count of NULL arguments. Inverse of num_nonnulls.",                                                                  "SELECT num_nulls(a, b, c);",                    pg("functions-comparison.html#FUNCTIONS-COMPARISON-FUNC-TABLE"));

  // ---- Sequence helpers (commonly missing) ----
  f!("nextval",  "nextval(regclass) -> bigint",   "Advance sequence and return next value.",   "SELECT nextval('users_id_seq');", pg("functions-sequence.html"));
  f!("currval",  "currval(regclass) -> bigint",   "Last value returned by nextval in this session.", "SELECT currval('users_id_seq');", pg("functions-sequence.html"));
  f!("setval",   "setval(regclass, bigint [, boolean]) -> bigint", "Set the sequence's current value.",        "SELECT setval('users_id_seq', 1000);", pg("functions-sequence.html"));
  f!("lastval",  "lastval() -> bigint",           "Last value returned by nextval anywhere in the session, any sequence.", "SELECT lastval();", pg("functions-sequence.html"));

  // ---- Range constructors ----
  f!("int4range", "int4range(lower int, upper int [, bounds text]) -> int4range", "Construct an int4 range.",   "SELECT int4range(1, 10);", pg("rangetypes.html"));
  f!("int8range", "int8range(lower bigint, upper bigint [, bounds text]) -> int8range", "Construct an int8 range.", "SELECT int8range(1, 10);", pg("rangetypes.html"));
  f!("numrange",  "numrange(lower numeric, upper numeric [, bounds text]) -> numrange", "Construct a numeric range.", "SELECT numrange(0.5, 1.5);", pg("rangetypes.html"));
  f!("tsrange",   "tsrange(lower timestamp, upper timestamp [, bounds text]) -> tsrange", "Construct a timestamp range.", "SELECT tsrange(now(), now() + interval '1 day');", pg("rangetypes.html"));
  f!("tstzrange", "tstzrange(lower timestamptz, upper timestamptz [, bounds text]) -> tstzrange", "Construct a timestamptz range.", "SELECT tstzrange(now(), now() + interval '1 day');", pg("rangetypes.html"));
  f!("daterange", "daterange(lower date, upper date [, bounds text]) -> daterange", "Construct a date range.", "SELECT daterange('2024-01-01', '2024-12-31');", pg("rangetypes.html"));

  m
}
