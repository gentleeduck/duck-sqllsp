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
  f!("quote_literal",  "quote_literal(text) -> text", "Quote a literal so it's a safe SQL string literal.", "SELECT quote_literal($$it's$$);", pg("functions-string.html"));
  f!("quote_ident",    "quote_ident(text) -> text", "Quote an identifier so it parses as a single name.", "SELECT quote_ident('weird name');", pg("functions-string.html"));
  f!("quote_nullable", "quote_nullable(anyelement) -> text", "Like quote_literal but renders NULL as the unquoted token.", "SELECT quote_nullable(NULL);", pg("functions-string.html"));
  f!("translate",      "translate(text, from text, to text) -> text", "Per-character substitution.", "SELECT translate('hello','el','EL');", pg("functions-string.html"));
  f!("repeat",         "repeat(text, n int) -> text", "Repeat the input n times.", "SELECT repeat('ab', 3);", pg("functions-string.html"));
  f!("reverse",        "reverse(text) -> text", "Reverse the characters.", "SELECT reverse('hello');", pg("functions-string.html"));
  f!("replace",        "replace(text, from text, to text) -> text", "Replace every occurrence.", "SELECT replace('a b a','a','x');", pg("functions-string.html"));
  f!("split_part",     "split_part(text, sep text, n int) -> text", "Return the n-th field after splitting on sep.", "SELECT split_part('a,b,c', ',', 2);", pg("functions-string.html"));
  f!("strpos",         "strpos(haystack text, needle text) -> integer", "1-based index of needle in haystack, 0 if not found.", "SELECT strpos('hello world','world');", pg("functions-string.html"));
  f!("btrim",          "btrim(text, chars text) -> text", "Trim chars from both ends (default whitespace).", "SELECT btrim('  hi  ');", pg("functions-string.html"));
  f!("ltrim",          "ltrim(text, chars text) -> text", "Trim chars from the left.", "SELECT ltrim('xxxhi','x');", pg("functions-string.html"));
  f!("rtrim",          "rtrim(text, chars text) -> text", "Trim chars from the right.", "SELECT rtrim('hiyyy','y');", pg("functions-string.html"));
  f!("initcap",        "initcap(text) -> text", "Capitalize the first letter of each word.", "SELECT initcap('hello WORLD');", pg("functions-string.html"));
  f!("octet_length",   "octet_length(text|bytea) -> integer", "Length in bytes.", "SELECT octet_length('héllo');", pg("functions-string.html"));
  f!("bit_length",     "bit_length(text|bytea|bit) -> integer", "Length in bits.", "SELECT bit_length('a');", pg("functions-string.html"));
  f!("ord",            "ord(text) -> integer", "Codepoint of the first character.", "SELECT ord('A');", pg("functions-string.html"));
  // Bit / byte ops on bytea/bit.
  f!("get_bit",  "get_bit(bytea|bit, n int) -> integer", "Extract the n-th bit.", "SELECT get_bit(B'10101010', 3);", pg("functions-binarystring.html"));
  f!("set_bit",  "set_bit(bytea|bit, n int, v int) -> same", "Set the n-th bit to v.", "SELECT set_bit(B'10000000', 3, 1);", pg("functions-binarystring.html"));
  f!("get_byte", "get_byte(bytea, n int) -> integer", "Extract the n-th byte.", "SELECT get_byte('\\xDEADBEEF'::bytea, 1);", pg("functions-binarystring.html"));
  f!("set_byte", "set_byte(bytea, n int, v int) -> bytea", "Set the n-th byte to v.", "SELECT set_byte('\\xDEAD'::bytea, 0, 255);", pg("functions-binarystring.html"));
  // Conversion / encoding helpers.
  f!("convert_from", "convert_from(bytea, src_encoding text) -> text", "Decode bytes from a specific encoding to TEXT.", "SELECT convert_from(E'\\\\xC3\\\\xA9'::bytea, 'UTF8');", pg("functions-string.html"));
  f!("convert_to",   "convert_to(text, dest_encoding text) -> bytea", "Encode TEXT into bytes in the requested encoding.", "SELECT convert_to('hi', 'UTF8');", pg("functions-string.html"));
  f!("convert",      "convert(bytea, src text, dest text) -> bytea", "Recode bytes between encodings.", "SELECT convert('hi'::bytea, 'LATIN1', 'UTF8');", pg("functions-string.html"));
  // pg_catalog admin helpers.
  f!("pg_relation_filepath",     "pg_relation_filepath(regclass) -> text", "Path (relative to data directory) of the relation's main file.", "SELECT pg_relation_filepath('users');", pg("functions-admin.html"));
  f!("pg_get_viewdef",            "pg_get_viewdef(regclass) -> text", "SELECT statement that defines a view.", "SELECT pg_get_viewdef('my_view'::regclass);", pg("functions-info.html"));
  f!("pg_get_function_arguments", "pg_get_function_arguments(oid) -> text", "Argument signature of a function.", "SELECT pg_get_function_arguments(p.oid) FROM pg_proc p;", pg("functions-info.html"));
  f!("pg_get_function_result",    "pg_get_function_result(oid) -> text", "Return-type signature of a function.", "SELECT pg_get_function_result(p.oid) FROM pg_proc p;", pg("functions-info.html"));
  f!("pg_get_functiondef",        "pg_get_functiondef(oid) -> text", "Full CREATE FUNCTION text.", "SELECT pg_get_functiondef('public.fn'::regproc);", pg("functions-info.html"));
  f!("pg_get_triggerdef",         "pg_get_triggerdef(oid) -> text", "CREATE TRIGGER text.", "SELECT pg_get_triggerdef(t.oid) FROM pg_trigger t LIMIT 1;", pg("functions-info.html"));
  f!("pg_get_constraintdef",      "pg_get_constraintdef(oid) -> text", "Constraint definition text.", "SELECT pg_get_constraintdef(c.oid) FROM pg_constraint c LIMIT 1;", pg("functions-info.html"));
  f!("pg_terminate_backend",      "pg_terminate_backend(pid int) -> boolean", "Terminate a backend by PID (needs pg_signal_backend role).", "SELECT pg_terminate_backend(12345);", pg("functions-admin.html"));
  f!("pg_cancel_backend",         "pg_cancel_backend(pid int) -> boolean", "Cancel the current query on a backend by PID.", "SELECT pg_cancel_backend(12345);", pg("functions-admin.html"));
  f!("pg_trigger_depth",          "pg_trigger_depth() -> integer", "Current nesting level of PG triggers (0 outside any trigger).", "SELECT pg_trigger_depth();", pg("functions-info.html"));
  f!("array_ndims",               "array_ndims(anyarray) -> integer", "Number of dimensions of the array.", "SELECT array_ndims(ARRAY[[1,2],[3,4]]);", pg("functions-array.html"));
  f!("array_upper",               "array_upper(anyarray, dim int) -> integer", "Upper bound of the requested dimension.", "SELECT array_upper(ARRAY[10,20,30], 1);", pg("functions-array.html"));
  f!("array_lower",               "array_lower(anyarray, dim int) -> integer", "Lower bound of the requested dimension.", "SELECT array_lower(ARRAY[10,20,30], 1);", pg("functions-array.html"));
  f!("array_dims",                "array_dims(anyarray) -> text", "Textual representation of the array dimensions.", "SELECT array_dims(ARRAY[[1,2],[3,4]]);", pg("functions-array.html"));
  f!("array_prepend",             "array_prepend(elt anyelement, arr anyarray) -> anyarray", "Prepend an element.", "SELECT array_prepend(0, ARRAY[1,2,3]);", pg("functions-array.html"));
  f!("array_append",              "array_append(arr anyarray, elt anyelement) -> anyarray", "Append an element (also operator `||`).", "SELECT array_append(ARRAY[1,2], 3);", pg("functions-array.html"));
  f!("array_remove",              "array_remove(arr anyarray, elt anyelement) -> anyarray", "Remove all occurrences of elt from the array.", "SELECT array_remove(ARRAY[1,2,3,2], 2);", pg("functions-array.html"));
  f!("array_replace",             "array_replace(arr anyarray, from anyelement, to anyelement) -> anyarray", "Replace every from with to.", "SELECT array_replace(ARRAY[1,2,3,2], 2, 99);", pg("functions-array.html"));
  f!("array_cat",                 "array_cat(a anyarray, b anyarray) -> anyarray", "Concatenate two arrays.", "SELECT array_cat(ARRAY[1,2], ARRAY[3,4]);", pg("functions-array.html"));
  f!("array_position",            "array_position(arr anyarray, elt anyelement) -> integer", "1-based index of elt, NULL if not present.", "SELECT array_position(ARRAY[10,20,30,40], 30);", pg("functions-array.html"));
  f!("array_positions",           "array_positions(arr anyarray, elt anyelement) -> int[]", "All 1-based indexes of elt.", "SELECT array_positions(ARRAY[1,2,1,3], 1);", pg("functions-array.html"));
  f!("array_to_string",           "array_to_string(arr anyarray, sep text [, null_str text]) -> text", "Join array elements with sep.", "SELECT array_to_string(ARRAY[1,2,3], ',', '*');", pg("functions-array.html"));
  f!("string_to_array",           "string_to_array(text, sep text [, null_str text]) -> text[]", "Split text into a text[].", "SELECT string_to_array('a,b,,c', ',');", pg("functions-array.html"));
  f!("cardinality",               "cardinality(anyarray) -> integer", "Total element count across all dimensions.", "SELECT cardinality(ARRAY[1,2,3]);", pg("functions-array.html"));
  f!("trim_array",                "trim_array(anyarray, n int) -> anyarray", "Return all but the last n elements.", "SELECT trim_array(ARRAY[1,2,3,4], 1);", pg("functions-array.html"));
  // current_* time/date are SQL-standard functions; they also work without parens.
  f!("current_time",      "current_time [(p int)] -> time with time zone", "Current TIME WITH TIME ZONE (optional precision).", "SELECT current_time(3);", pg("functions-datetime.html"));
  f!("current_timestamp", "current_timestamp [(p int)] -> timestamp with time zone", "Current TIMESTAMPTZ (optional precision).", "SELECT current_timestamp(0);", pg("functions-datetime.html"));
  f!("current_date",      "current_date -> date", "Current DATE (no parens).", "SELECT current_date;", pg("functions-datetime.html"));
  f!("clock_timestamp",   "clock_timestamp() -> timestamp with time zone", "Wall-clock TIMESTAMPTZ; changes within a transaction.", "SELECT clock_timestamp();", pg("functions-datetime.html"));
  f!("statement_timestamp",   "statement_timestamp() -> timestamp with time zone", "TIMESTAMPTZ of statement start.", "SELECT statement_timestamp();", pg("functions-datetime.html"));
  f!("transaction_timestamp", "transaction_timestamp() -> timestamp with time zone", "Alias for now(); TIMESTAMPTZ of transaction start.", "SELECT transaction_timestamp();", pg("functions-datetime.html"));
  f!("timeofday",         "timeofday() -> text", "Wall-clock time as text (legacy).", "SELECT timeofday();", pg("functions-datetime.html"));
  f!("substr",            "substr(text, start int [, len int]) -> text", "Positional substring (PG alias for SQL-standard substring(... FROM n FOR m)).", "SELECT substr('hello', 2, 3);", pg("functions-string.html"));
  f!("trim",              "trim([LEADING|TRAILING|BOTH] [chars] FROM text) -> text | trim(text [, chars]) -> text", "Trim characters from both/leading/trailing.", "SELECT trim(BOTH 'x' FROM 'xxhixx');", pg("functions-string.html"));
  f!("char_length",       "char_length(text) -> integer", "Character count of text.", "SELECT char_length('héllo');", pg("functions-string.html"));
  f!("character_length",  "character_length(text) -> integer", "Character count of text (SQL-standard spelling).", "SELECT character_length('héllo');", pg("functions-string.html"));
  f!("md5",               "md5(text|bytea) -> text", "MD5 hash hex digest.", "SELECT md5('hello');", pg("functions-string.html"));
  f!("position",          "position(needle IN haystack) -> integer", "SQL-standard 1-based index of needle.", "SELECT position('world' IN 'hello world');", pg("functions-string.html"));
  f!("cbrt",          "cbrt(double) -> double precision", "Cube root.", "SELECT cbrt(27);", pg("functions-math.html"));
  f!("gcd",           "gcd(int, int) -> integer", "Greatest common divisor.", "SELECT gcd(12, 18);", pg("functions-math.html"));
  f!("lcm",           "lcm(int, int) -> integer", "Least common multiple.", "SELECT lcm(4, 6);", pg("functions-math.html"));
  f!("scale",         "scale(numeric) -> integer", "Scale of a numeric value (digits after decimal).", "SELECT scale(1.230);", pg("functions-math.html"));
  f!("min_scale",     "min_scale(numeric) -> integer", "Minimum scale needed to represent value exactly.", "SELECT min_scale(1.230);", pg("functions-math.html"));
  f!("trim_scale",    "trim_scale(numeric) -> numeric", "Strip trailing zeros after decimal point.", "SELECT trim_scale(1.2300);", pg("functions-math.html"));
  f!("width_bucket",  "width_bucket(operand, b1, b2, count int) -> integer", "Histogram bucket number for operand in count equal-width buckets between b1 and b2.", "SELECT width_bucket(5.0, 0, 10, 4);", pg("functions-math.html"));
  // Statistical / regression aggregates.
  f!("regr_slope",     "regr_slope(y, x) -> double precision", "Slope of the linear regression line.", "SELECT regr_slope(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_intercept", "regr_intercept(y, x) -> double precision", "Intercept of the linear regression line.", "SELECT regr_intercept(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_r2",        "regr_r2(y, x) -> double precision", "Square of the correlation coefficient (R²).", "SELECT regr_r2(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_count",     "regr_count(y, x) -> bigint", "Count of input rows in which both inputs are non-null.", "SELECT regr_count(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_avgx",      "regr_avgx(y, x) -> double precision", "Average of the x values.", "SELECT regr_avgx(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_avgy",      "regr_avgy(y, x) -> double precision", "Average of the y values.", "SELECT regr_avgy(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_sxx",       "regr_sxx(y, x) -> double precision", "Sum of squares of the x values.", "SELECT regr_sxx(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_syy",       "regr_syy(y, x) -> double precision", "Sum of squares of the y values.", "SELECT regr_syy(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("regr_sxy",       "regr_sxy(y, x) -> double precision", "Sum of products of x*y.", "SELECT regr_sxy(price, qty) FROM items;", pg("functions-aggregate.html"));
  f!("array_to_json",  "array_to_json(anyarray [, pretty bool]) -> json", "Convert an array to a JSON array.", "SELECT array_to_json(ARRAY[1,2,3]);", pg("functions-json.html"));
  f!("row_to_json",    "row_to_json(record [, pretty bool]) -> json", "Convert a row to a JSON object.", "SELECT row_to_json(t) FROM users t;", pg("functions-json.html"));
  f!("date_bin",       "date_bin(interval, source ts, origin ts) -> timestamp(tz)", "Snap a timestamp to the nearest bucket of size `interval` aligned to `origin`.", "SELECT date_bin('15 minutes', now(), '2000-01-01');", pg("functions-datetime.html"));
  f!("timezone",       "timezone(zone text, ts timestamp(tz)) -> timestamp(tz) | timezone(zone, time) -> time", "SQL-standard alternate to `AT TIME ZONE`.", "SELECT timezone('UTC', now());", pg("functions-datetime.html"));
  f!("isfinite",       "isfinite(date|timestamp|interval) -> boolean", "True when value is not -infinity/+infinity.", "SELECT isfinite(now());", pg("functions-datetime.html"));

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
  // Full-text search helpers.
  f!("setweight",       "setweight(tsvector, weight char) -> tsvector",      "Tag every lexeme with a weight letter (A/B/C/D).", "SELECT setweight(to_tsvector('title'), 'A');",   pg("textsearch-features.html#TEXTSEARCH-MANIPULATE-TSVECTOR"));
  f!("ts_headline",     "ts_headline([config], doc, query [, options]) -> text", "Highlight query matches in a document.",        "SELECT ts_headline('eng', body, query) FROM docs;", pg("textsearch-controls.html#TEXTSEARCH-HEADLINE"));
  f!("plainto_tsquery", "plainto_tsquery([config], text) -> tsquery",        "Convert text to a tsquery with AND semantics.",    "SELECT plainto_tsquery('eng', 'hello world');",  pg("textsearch-controls.html"));
  f!("similarity",      "similarity(text, text) -> real",                    "pg_trgm similarity score (0..1).",                  "SELECT similarity('foo', 'foobar');",            pg("pgtrgm.html"));
  f!("word_similarity", "word_similarity(text, text) -> real",               "pg_trgm word-level similarity.",                    "SELECT word_similarity('rust', 'postgres rust');", pg("pgtrgm.html"));
  f!("strict_word_similarity", "strict_word_similarity(text, text) -> real", "Strict word similarity (pg_trgm).",                 "SELECT strict_word_similarity('foo', 'foobar');", pg("pgtrgm.html"));
  f!("show_trgm",       "show_trgm(text) -> text[]",                         "Return trigrams of a string.",                      "SELECT show_trgm('foobar');",                   pg("pgtrgm.html"));

  // Additional jsonb fns.
  f!("jsonb_build_array",          "jsonb_build_array(VARIADIC \"any\") -> jsonb", "Build a jsonb array from variadic args.",       "SELECT jsonb_build_array('a', 1, true);",                 pg("functions-json.html"));
  f!("jsonb_object_agg",           "jsonb_object_agg(key, value) -> jsonb",        "Aggregate key/value pairs into a jsonb object.", "SELECT jsonb_object_agg(k, v) FROM kv;",                    pg("functions-aggregate.html"));
  f!("jsonb_array_elements_text",  "jsonb_array_elements_text(jsonb) -> setof text", "Expand jsonb array into rows of text.",        "SELECT * FROM jsonb_array_elements_text('[\"a\",\"b\"]');", pg("functions-json.html"));
  f!("jsonb_each_text",            "jsonb_each_text(jsonb) -> setof (text, text)", "Expand jsonb object into key/value text rows.",  "SELECT * FROM jsonb_each_text('{\"a\":1}');",               pg("functions-json.html"));
  f!("json_build_array",           "json_build_array(VARIADIC \"any\") -> json",   "Build a json array.",                            "SELECT json_build_array(1, 2, 3);",                          pg("functions-json.html"));
  f!("json_object_agg",            "json_object_agg(key, value) -> json",          "Aggregate key/value pairs into a json object.",  "SELECT json_object_agg(k, v) FROM kv;",                      pg("functions-aggregate.html"));
  f!("json_array_elements_text",   "json_array_elements_text(json) -> setof text", "Expand json array as text rows.",                "SELECT * FROM json_array_elements_text('[\"a\"]');",         pg("functions-json.html"));
  f!("json_each_text",             "json_each_text(json) -> setof (text, text)",   "Expand json object as text key/value rows.",     "SELECT * FROM json_each_text('{\"a\":\"b\"}');",             pg("functions-json.html"));
  f!("jsonb_insert",               "jsonb_insert(target, path, new_value [, insert_after]) -> jsonb", "Insert into a jsonb structure.",            "SELECT jsonb_insert(data, '{0}', '\"x\"');",       pg("functions-json.html"));
  f!("jsonb_object",               "jsonb_object(text[] [, text[]]) -> jsonb",     "Build a jsonb object from a key array (+ optional value array).", "SELECT jsonb_object('{a,b}', '{1,2}');", pg("functions-json.html"));
  f!("json_object",                "json_object(text[] [, text[]]) -> json",       "Build a json object from key/value arrays.",     "SELECT json_object('{a,b}', '{1,2}');",                      pg("functions-json.html"));

  // Geometric type constructors.
  f!("point",   "point(x float8, y float8) -> point",  "Construct a geometric point.", "SELECT point(40.7, -74.0);", pg("functions-geometry.html"));
  f!("box",     "box(point, point) -> box",            "Construct a rectangular box from two points.", "SELECT box(point(0,0), point(1,1));", pg("functions-geometry.html"));
  f!("circle",  "circle(point, float8) -> circle",     "Construct a circle from center + radius.", "SELECT circle(point(0,0), 5);", pg("functions-geometry.html"));
  f!("line",    "line(point, point) -> line",          "Construct a line from two points.", "SELECT line(point(0,0), point(1,1));", pg("functions-geometry.html"));
  f!("lseg",    "lseg(point, point) -> lseg",          "Construct a line segment.", "SELECT lseg(point(0,0), point(1,1));", pg("functions-geometry.html"));
  f!("path",    "path(polygon) -> path",               "Construct a path from a polygon.", "SELECT path('((0,0),(1,1),(2,2))'::polygon);", pg("functions-geometry.html"));
  f!("polygon", "polygon(box) -> polygon",             "Construct a polygon from a box.", "SELECT polygon(box(point(0,0), point(1,1)));", pg("functions-geometry.html"));

  // uuid-ossp extension fns (heavily used in real schemas).
  f!("uuid_generate_v4", "uuid_generate_v4() -> uuid",      "Random UUID (v4). Requires the uuid-ossp extension.", "SELECT uuid_generate_v4();", pg("uuid-ossp.html"));
  f!("uuid_generate_v1", "uuid_generate_v1() -> uuid",      "MAC-address + timestamp UUID (v1).",                  "SELECT uuid_generate_v1();", pg("uuid-ossp.html"));
  f!("uuid_generate_v3", "uuid_generate_v3(namespace uuid, name text) -> uuid", "Name-based UUID (v3) using MD5.", "SELECT uuid_generate_v3(uuid_ns_dns(), 'example.com');", pg("uuid-ossp.html"));
  f!("uuid_generate_v5", "uuid_generate_v5(namespace uuid, name text) -> uuid", "Name-based UUID (v5) using SHA-1.", "SELECT uuid_generate_v5(uuid_ns_dns(), 'example.com');", pg("uuid-ossp.html"));
  f!("uuid_nil",         "uuid_nil() -> uuid",              "All-zero UUID.",                                       "SELECT uuid_nil();", pg("uuid-ossp.html"));
  f!("uuid_ns_dns",      "uuid_ns_dns() -> uuid",           "DNS namespace UUID.",                                  "SELECT uuid_ns_dns();", pg("uuid-ossp.html"));
  f!("uuid_ns_url",      "uuid_ns_url() -> uuid",           "URL namespace UUID.",                                  "SELECT uuid_ns_url();", pg("uuid-ossp.html"));
  f!("gen_random_uuid",  "gen_random_uuid() -> uuid",       "Random UUID (built-in on PG13+, no extension).",       "SELECT gen_random_uuid();", pg("functions-uuid.html"));

  // Common math + date helpers that were missing.
  f!("ceil",  "ceil(numeric) -> numeric",   "Round up to integer.",   "SELECT ceil(3.2);", pg("functions-math.html"));
  f!("div",   "div(y numeric, x numeric) -> numeric", "Integer quotient of y/x.", "SELECT div(10, 3);", pg("functions-math.html"));
  f!("justify_days",  "justify_days(interval) -> interval",  "Adjust 30-day periods into months.", "SELECT justify_days(interval '60 days');", pg("functions-datetime.html"));
  f!("justify_hours", "justify_hours(interval) -> interval", "Adjust 24-hour periods into days.",  "SELECT justify_hours(interval '50 hours');", pg("functions-datetime.html"));
  f!("justify_interval", "justify_interval(interval) -> interval", "Adjust both days and hours.", "SELECT justify_interval(interval '60 days 50 hours');", pg("functions-datetime.html"));
  f!("sha256", "sha256(bytea) -> bytea", "SHA-256 digest.", "SELECT encode(sha256('hello'::bytea), 'hex');", pg("functions-binarystring.html"));
  f!("sha224", "sha224(bytea) -> bytea", "SHA-224 digest.", "SELECT sha224('hello'::bytea);", pg("functions-binarystring.html"));
  f!("sha384", "sha384(bytea) -> bytea", "SHA-384 digest.", "SELECT sha384('hello'::bytea);", pg("functions-binarystring.html"));
  f!("sha512", "sha512(bytea) -> bytea", "SHA-512 digest.", "SELECT sha512('hello'::bytea);", pg("functions-binarystring.html"));

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
