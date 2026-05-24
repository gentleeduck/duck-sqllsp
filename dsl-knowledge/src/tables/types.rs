//! SQL data type table. Append new types with the `t!` macro.

use crate::entry::{Entry, Kind, pg};
use std::collections::HashMap;

pub fn build() -> HashMap<&'static str, Entry> {
  let mut m = HashMap::new();
  macro_rules! t {
    ($label:expr, $doc:expr, $example:expr, $url:expr) => {
      m.insert(
        $label,
        Entry { label: $label, kind: Kind::Type, doc: $doc, signature: None, example: $example, url: $url },
      );
    };
  }

  t!("BOOLEAN", "true / false / NULL.", "is_active BOOLEAN NOT NULL DEFAULT false", pg("datatype-boolean.html"));
  t!("SMALLINT", "2-byte signed integer.", "qty SMALLINT", pg("datatype-numeric.html"));
  t!("INTEGER", "4-byte signed integer. Alias INT.", "count INTEGER NOT NULL", pg("datatype-numeric.html"));
  t!("INT", "4-byte signed integer (alias for INTEGER).", "count INT NOT NULL", pg("datatype-numeric.html"));
  t!("INT2", "Postgres-internal name for SMALLINT (2-byte signed).", "qty INT2", pg("datatype-numeric.html"));
  t!("INT4", "Postgres-internal name for INTEGER (4-byte signed).", "count INT4", pg("datatype-numeric.html"));
  t!("INT8", "Postgres-internal name for BIGINT (8-byte signed).", "id INT8", pg("datatype-numeric.html"));
  t!(
    "FLOAT4",
    "Postgres-internal name for REAL (4-byte IEEE float).",
    "weight FLOAT4",
    pg("datatype-numeric.html#DATATYPE-FLOAT")
  );
  t!(
    "FLOAT8",
    "Postgres-internal name for DOUBLE PRECISION (8-byte float).",
    "ratio FLOAT8",
    pg("datatype-numeric.html#DATATYPE-FLOAT")
  );
  t!(
    "DECIMAL",
    "Exact decimal. Synonym for NUMERIC.",
    "price DECIMAL(10, 2)",
    pg("datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL")
  );
  t!(
    "MONEY",
    "Currency amount. Locale-formatted. Prefer NUMERIC for portability.",
    "amount MONEY",
    pg("datatype-money.html")
  );
  t!("BOOL", "Alias for BOOLEAN.", "active BOOL NOT NULL DEFAULT false", pg("datatype-boolean.html"));
  t!(
    "TIMETZ",
    "TIME WITH TIME ZONE -- rarely useful, see datatype docs for caveats.",
    "open_at TIMETZ",
    pg("datatype-datetime.html")
  );
  t!("CITEXT", "Case-insensitive TEXT. Requires the `citext` extension.", "email CITEXT NOT NULL", pg("citext.html"));
  t!("CIDR", "IPv4 / IPv6 network. Stricter than INET (no host bits).", "subnet CIDR", pg("datatype-net-types.html"));
  t!("MACADDR", "6-byte MAC address.", "mac MACADDR", pg("datatype-net-types.html"));
  t!("MACADDR8", "8-byte EUI-64 MAC address.", "mac8 MACADDR8", pg("datatype-net-types.html"));
  t!("INT4RANGE", "Range of INTEGER values.", "ages INT4RANGE", pg("rangetypes.html"));
  t!("INT8RANGE", "Range of BIGINT values.", "ids INT8RANGE", pg("rangetypes.html"));
  t!("NUMRANGE", "Range of NUMERIC values.", "prices NUMRANGE", pg("rangetypes.html"));
  t!("TSRANGE", "Range of TIMESTAMP (no zone) values.", "span TSRANGE", pg("rangetypes.html"));
  t!("TSTZRANGE", "Range of TIMESTAMPTZ values. Most common range type.", "window TSTZRANGE", pg("rangetypes.html"));
  t!("DATERANGE", "Range of DATE values.", "stay DATERANGE", pg("rangetypes.html"));
  t!("OID", "Object identifier. Internal Postgres reference type.", "ref OID", pg("datatype-oid.html"));
  t!("XML", "XML document. Stored as text; needs libxml support.", "payload XML", pg("datatype-xml.html"));
  t!("TSVECTOR", "Tokenised full-text search vector.", "search_doc TSVECTOR", pg("datatype-textsearch.html"));
  t!("TSQUERY", "Full-text search query.", "q TSQUERY", pg("datatype-textsearch.html"));
  t!("POINT", "Geometric point (x, y).", "p POINT", pg("datatype-geometric.html"));
  t!("LINE", "Geometric infinite line.", "l LINE", pg("datatype-geometric.html"));
  t!("BOX", "Geometric rectangular box.", "b BOX", pg("datatype-geometric.html"));
  t!("POLYGON", "Closed geometric polygon.", "shape POLYGON", pg("datatype-geometric.html"));
  t!("CIRCLE", "Geometric circle (center + radius).", "c CIRCLE", pg("datatype-geometric.html"));
  t!("BIGINT", "8-byte signed integer.", "id BIGINT", pg("datatype-numeric.html"));
  t!(
    "NUMERIC",
    "Exact decimal with up to 1000 digits. `NUMERIC(precision, scale)` -- e.g. NUMERIC(10,2) \
        means 8 digits before the decimal and 2 after. Without (precision, scale) it stores any \
        scale up to system limits. Default text format omits trailing zeros; use `to_char(n, '999G999D00')` \
        for fixed formatting. Aliases: DECIMAL.",
    "price NUMERIC(10, 2) NOT NULL",
    pg("datatype-numeric.html#DATATYPE-NUMERIC-DECIMAL")
  );
  t!("REAL", "4-byte IEEE float.", "weight REAL", pg("datatype-numeric.html#DATATYPE-FLOAT"));
  t!("DOUBLE PRECISION", "8-byte IEEE float.", "ratio DOUBLE PRECISION", pg("datatype-numeric.html#DATATYPE-FLOAT"));
  t!(
    "SERIAL",
    "Auto-incrementing INTEGER backed by a sequence.",
    "id SERIAL PRIMARY KEY",
    pg("datatype-numeric.html#DATATYPE-SERIAL")
  );
  t!("BIGSERIAL", "BIGINT auto-increment.", "id BIGSERIAL PRIMARY KEY", pg("datatype-numeric.html#DATATYPE-SERIAL"));
  t!(
    "VARCHAR",
    "Variable-length character with optional length cap.",
    "name VARCHAR(255) NOT NULL",
    pg("datatype-character.html")
  );
  t!(
    "TEXT",
    "Variable-length character with no cap. Default choice in Postgres.",
    "body TEXT NOT NULL",
    pg("datatype-character.html")
  );
  t!("CHAR", "Fixed-length, blank-padded.", "code CHAR(3)", pg("datatype-character.html"));
  t!("BYTEA", "Binary blob.", "avatar BYTEA", pg("datatype-binary.html"));
  t!(
    "DATE",
    "Calendar date, no time, no zone. Default output is ISO 8601 `YYYY-MM-DD`. \
        Accepted inputs: `'2026-01-15'` (ISO), `'15-Jan-2026'`, `'20260115'`, `current_date`, \
        plus `DATE 'YYYY-MM-DD'` literals.",
    "birth_date DATE DEFAULT current_date",
    pg("datatype-datetime.html")
  );
  t!(
    "TIME",
    "Time of day without time zone. Default output `HH24:MI:SS[.ffffff]`. \
        Accepted inputs: `'14:30'`, `'14:30:00'`, `'14:30:00.123456'`. \
        Use `TIME WITH TIME ZONE` (TIMETZ) for zone-aware values (rarely useful).",
    "open_at TIME DEFAULT '09:00'",
    pg("datatype-datetime.html")
  );
  t!(
    "TIMESTAMP",
    "Date + time WITHOUT time zone. Default output `YYYY-MM-DD HH24:MI:SS[.ffffff]`. \
        Avoid for application data: use TIMESTAMPTZ instead so daylight-saving and \
        cross-region inserts stay correct. Accepted inputs: `'2026-01-15 14:30'`, \
        `'2026-01-15T14:30:00'`, `current_timestamp`.",
    "ts TIMESTAMP",
    pg("datatype-datetime.html")
  );
  t!(
    "TIMESTAMPTZ",
    "Date + time WITH time zone. Storage is always UTC; the session `TimeZone` \
        decides input parsing and display. Default output `YYYY-MM-DD HH24:MI:SS+00`. \
        Accepted inputs: `'2026-01-15 14:30Z'`, `'2026-01-15 14:30+02'`, \
        `'2026-01-15 14:30:00 Europe/Berlin'`, `now()`, `current_timestamp`. \
        Formatting helpers: `to_char(ts, 'YYYY-MM-DD HH24:MI:SS TZ')`, \
        `to_char(ts, 'Day, DD Mon YYYY')`, `to_char(ts, 'IYYY-IW')` (ISO week).",
    "created_at TIMESTAMPTZ NOT NULL DEFAULT now()",
    pg("datatype-datetime.html")
  );
  t!(
    "INTERVAL",
    "Time span. Accepted inputs: `INTERVAL '1 day 2 hours 3 minutes'`, \
        `INTERVAL '90 seconds'`, `INTERVAL '1-2'` (years-months), `INTERVAL 'P1DT2H'` (ISO 8601). \
        Field selectors: `INTERVAL '1' YEAR / MONTH / DAY / HOUR / MINUTE / SECOND`.",
    "deleted_after INTERVAL DEFAULT '30 days'",
    pg("datatype-datetime.html#DATATYPE-INTERVAL-INPUT")
  );
  t!(
    "UUID",
    "128-bit identifier. Canonical text form: `'xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx'`. \
        Generators: `gen_random_uuid()` (v4, Postgres 13+), `uuid_generate_v4()` (uuid-ossp). \
        Accepts hex with or without dashes; output is always dashed lowercase.",
    "id UUID PRIMARY KEY DEFAULT gen_random_uuid()",
    pg("datatype-uuid.html")
  );
  t!(
    "JSON",
    "JSON stored verbatim. Whitespace and key order preserved. Slower for queries than JSONB. \
        Operators: `->` (key as json), `->>` (key as text), `#>` (path), `#>>` (path as text). \
        Validation happens on input.",
    "metadata JSON DEFAULT '{}'::json",
    pg("datatype-json.html")
  );
  t!(
    "JSONB",
    "Binary JSON. Indexable, fast, the right default for application data. \
        Operators add `@>` (contains), `<@` (contained), `?` (key exists), `?|` (any), `?&` (all). \
        Build with `jsonb_build_object('k', v, ...)`; mutate with `jsonb_set(j, path, value)`.",
    "metadata JSONB NOT NULL DEFAULT '{}'::jsonb",
    pg("datatype-json.html")
  );
  t!("INET", "IPv4 / IPv6 host or network.", "client_ip INET", pg("datatype-net-types.html"));

  // Range types -- built-ins added in PG 9.2. Each holds an ordered
  // pair of bounds over the underlying subtype with inclusive/
  // exclusive notation. Multirange types (PG 14+) hold a set of
  // disjoint ranges.
  t!(
    "INT4RANGE",
    "Range of integer. `'[1,10)'::int4range`. Use `@>` for containment, `&&` for overlap.",
    "valid_ids INT4RANGE",
    pg("rangetypes.html")
  );
  t!("INT8RANGE", "Range of bigint.", "id_window INT8RANGE", pg("rangetypes.html"));
  t!("NUMRANGE", "Range of numeric.", "price_band NUMRANGE", pg("rangetypes.html"));
  t!("TSRANGE", "Range of timestamp without time zone.", "valid TSRANGE", pg("rangetypes.html"));
  t!(
    "TSTZRANGE",
    "Range of timestamp with time zone. The right default for time-bounded data.",
    "active TSTZRANGE NOT NULL",
    pg("rangetypes.html")
  );
  t!("DATERANGE", "Range of date.", "booking DATERANGE", pg("rangetypes.html"));
  t!(
    "INT4MULTIRANGE",
    "Multirange of integer (PG 14+). Set of disjoint int4ranges.",
    "windows INT4MULTIRANGE",
    pg("rangetypes.html")
  );
  t!("INT8MULTIRANGE", "Multirange of bigint (PG 14+).", "windows INT8MULTIRANGE", pg("rangetypes.html"));
  t!("NUMMULTIRANGE", "Multirange of numeric (PG 14+).", "bands NUMMULTIRANGE", pg("rangetypes.html"));
  t!("TSMULTIRANGE", "Multirange of timestamp (PG 14+).", "windows TSMULTIRANGE", pg("rangetypes.html"));
  t!(
    "TSTZMULTIRANGE",
    "Multirange of timestamp with time zone (PG 14+).",
    "active TSTZMULTIRANGE",
    pg("rangetypes.html")
  );
  t!("DATEMULTIRANGE", "Multirange of date (PG 14+).", "blackout DATEMULTIRANGE", pg("rangetypes.html"));

  m
}
