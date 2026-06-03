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

  // ---- Bit / serial / lsn / geometric ----
  t!("BIT", "Fixed-width bit string. BIT(n).", "flags BIT(8)", pg("datatype-bit.html"));
  t!("VARBIT", "Variable-width bit string. BIT VARYING(n).", "mask VARBIT(64)", pg("datatype-bit.html"));
  t!(
    "SMALLSERIAL",
    "Auto-increment INT2. Sugar for INT2 + sequence + NOT NULL.",
    "id SMALLSERIAL PRIMARY KEY",
    pg("datatype-numeric.html#DATATYPE-SERIAL")
  );
  t!(
    "PG_LSN",
    "Postgres log sequence number (8 bytes). Used for WAL positions.",
    "lsn PG_LSN",
    pg("datatype-pg-lsn.html")
  );
  t!("LSEG", "Geometric line segment.", "edge LSEG", pg("datatype-geometric.html"));
  t!("PATH", "Geometric path (closed or open).", "trail PATH", pg("datatype-geometric.html"));
  t!(
    "REGCLASS",
    "OID alias for tables / sequences / views. Validates the name at parse.",
    "tbl REGCLASS",
    pg("datatype-oid.html")
  );
  t!("REGPROC", "OID alias for functions (no argument types).", "fn REGPROC", pg("datatype-oid.html"));
  t!("REGTYPE", "OID alias for types.", "ty REGTYPE", pg("datatype-oid.html"));
  t!("REGROLE", "OID alias for roles.", "r REGROLE", pg("datatype-oid.html"));
  t!("REGNAMESPACE", "OID alias for schemas.", "ns REGNAMESPACE", pg("datatype-oid.html"));

  // ---- pseudo + REG + standard-spelling sweep ---------------
  t!("ANY", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANY", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYARRAY", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYARRAY", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYCOMPATIBLE", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYCOMPATIBLE", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYCOMPATIBLEARRAY", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYCOMPATIBLEARRAY", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYCOMPATIBLEMULTIRANGE", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYCOMPATIBLEMULTIRANGE", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYCOMPATIBLENONARRAY", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYCOMPATIBLENONARRAY", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYCOMPATIBLERANGE", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYCOMPATIBLERANGE", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYELEMENT", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYELEMENT", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYENUM", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYENUM", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYMULTIRANGE", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYMULTIRANGE", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYNONARRAY", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYNONARRAY", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("ANYRANGE", "Polymorphic pseudo-type accepted by functions; bound to a concrete type at call time.", "x ANYRANGE", pg("extend-type-system.html#EXTEND-TYPES-POLYMORPHIC"));
  t!("CSTRING", "Pseudo-type internal to PG -- not usable in user table columns.", "x CSTRING", pg("datatype-pseudo.html"));
  t!("DEC", "SQL standard alias for NUMERIC / DECIMAL.", "price DEC(10,2)", pg("datatype-numeric.html"));
  t!("EVENT_TRIGGER", "Pseudo-type internal to PG -- not usable in user table columns.", "x EVENT_TRIGGER", pg("datatype-pseudo.html"));
  t!("FDW_HANDLER", "Pseudo-type for the entry point of a feature handler. Cannot appear in user SQL.", "x FDW_HANDLER", pg("extend-type-system.html"));
  t!("INDEX_AM_HANDLER", "Pseudo-type for the entry point of a feature handler. Cannot appear in user SQL.", "x INDEX_AM_HANDLER", pg("extend-type-system.html"));
  t!("INTERNAL", "Pseudo-type internal to PG -- not usable in user table columns.", "x INTERNAL", pg("datatype-pseudo.html"));
  t!("JSONPATH", "JSONPath expression value -- used by jsonb_path_* functions.", "p JSONPATH", pg("datatype-json.html#DATATYPE-JSONPATH"));
  t!("LANGUAGE_HANDLER", "Pseudo-type for the entry point of a feature handler. Cannot appear in user SQL.", "x LANGUAGE_HANDLER", pg("extend-type-system.html"));
  t!("NAME", "63-byte identifier -- internal PG type used in pg_catalog rows.", "-- pg_catalog only", pg("datatype-character.html"));
  t!("NATIONAL CHAR", "SQL standard alias for CHARACTER (NCHAR = national char). PG treats them as plain text.", "c NATIONAL CHAR(20)", pg("datatype-character.html"));
  t!("NATIONAL CHARACTER", "SQL standard alias for CHARACTER (NCHAR = national char). PG treats them as plain text.", "c NATIONAL CHARACTER(20)", pg("datatype-character.html"));
  t!("NCHAR", "SQL standard alias for CHARACTER (NCHAR = national char). PG treats them as plain text.", "c NCHAR(20)", pg("datatype-character.html"));
  t!("OPAQUE", "Pseudo-type internal to PG -- not usable in user table columns.", "x OPAQUE", pg("datatype-pseudo.html"));
  t!("PG_DDL_COMMAND", "Pseudo-type internal to PG -- not usable in user table columns.", "x PG_DDL_COMMAND", pg("datatype-pseudo.html"));
  t!("PG_SNAPSHOT", "Visibility snapshot -- replaces deprecated txid_snapshot.", "snap PG_SNAPSHOT", pg("datatype-pg-lsn.html"));
  t!("RECORD", "Pseudo-type internal to PG -- not usable in user table columns.", "x RECORD", pg("datatype-pseudo.html"));
  t!("REGCOLLATION", "Object-identifier alias type -- accepts a name + resolves to OID. Useful in queries against pg_catalog.", "t REGCOLLATION", pg("datatype-oid.html"));
  t!("REGCONFIG", "Object-identifier alias type -- accepts a name + resolves to OID. Useful in queries against pg_catalog.", "t REGCONFIG", pg("datatype-oid.html"));
  t!("REGDICTIONARY", "Object-identifier alias type -- accepts a name + resolves to OID. Useful in queries against pg_catalog.", "t REGDICTIONARY", pg("datatype-oid.html"));
  t!("REGOPER", "Object-identifier alias type -- accepts a name + resolves to OID. Useful in queries against pg_catalog.", "t REGOPER", pg("datatype-oid.html"));
  t!("REGOPERATOR", "Object-identifier alias type -- accepts a name + resolves to OID. Useful in queries against pg_catalog.", "t REGOPERATOR", pg("datatype-oid.html"));
  t!("REGPROCEDURE", "Object-identifier alias type -- accepts a name + resolves to OID. Useful in queries against pg_catalog.", "t REGPROCEDURE", pg("datatype-oid.html"));
  t!("TABLE_AM_HANDLER", "Pseudo-type for the entry point of a feature handler. Cannot appear in user SQL.", "x TABLE_AM_HANDLER", pg("extend-type-system.html"));
  t!("TIMESTAMP WITHOUT TIME ZONE", "SQL standard spelling for the time/timestamp variants (with/without time zone).", "x TIMESTAMP WITHOUT TIME ZONE", pg("datatype-datetime.html"));
  t!("TIMESTAMP WITH TIME ZONE", "SQL standard spelling for the time/timestamp variants (with/without time zone).", "x TIMESTAMP WITH TIME ZONE", pg("datatype-datetime.html"));
  t!("TIME WITHOUT TIME ZONE", "SQL standard spelling for the time/timestamp variants (with/without time zone).", "x TIME WITHOUT TIME ZONE", pg("datatype-datetime.html"));
  t!("TIME WITH TIME ZONE", "SQL standard spelling for the time/timestamp variants (with/without time zone).", "x TIME WITH TIME ZONE", pg("datatype-datetime.html"));
  t!("TRIGGER", "Pseudo-type internal to PG -- not usable in user table columns.", "x TRIGGER", pg("datatype-pseudo.html"));
  t!("TSM_HANDLER", "Pseudo-type for the entry point of a feature handler. Cannot appear in user SQL.", "x TSM_HANDLER", pg("extend-type-system.html"));
  t!("TXID_SNAPSHOT", "Deprecated -- use PG_SNAPSHOT in PG13+.", "snap TXID_SNAPSHOT", pg("datatype-pg-lsn.html"));
  t!("UNKNOWN", "Pseudo-type internal to PG -- not usable in user table columns.", "x UNKNOWN", pg("datatype-pseudo.html"));
  t!("VOID", "Pseudo-type internal to PG -- not usable in user table columns.", "x VOID", pg("datatype-pseudo.html"));
  t!("XID", "4-byte transaction id. Wraps around -- compare with `txid_*` fns or convert to `xid8` (PG13+) for full ordering.", "id XID", pg("datatype-oid.html"));
  t!("XID8", "8-byte transaction id (PG13+) -- no wraparound concerns. Pair with `pg_snapshot`.", "id XID8", pg("datatype-oid.html"));
  t!("ACLITEM", "Single ACL entry stored inside an aclitem[] (used by pg_class.relacl etc). Read via `aclexplode()` / `pg_get_userbyid()`.", "x ACLITEM", pg("ddl-priv.html"));

  // ---- Extension types ----
  t!("HSTORE", "Key/value text map (hstore extension). Use jsonb instead in new code.", "tags HSTORE", "https://www.postgresql.org/docs/current/hstore.html");
  t!("VECTOR", "Dense float vector (pgvector extension). Specify dimensions: VECTOR(1536).", "embedding VECTOR(1536)", "https://github.com/pgvector/pgvector");
  t!("HALFVEC", "Half-precision (float2) vector (pgvector 0.7+). Half the storage of VECTOR.", "embedding HALFVEC(1536)", "https://github.com/pgvector/pgvector");
  t!("BIT_VEC", "Binary vector (pgvector 0.7+). Bit-packed for hamming/jaccard distance.", "fp BIT_VEC(64)", "https://github.com/pgvector/pgvector");
  t!("SPARSEVEC", "Sparse vector (pgvector 0.7+). Only stores nonzero indices.", "v SPARSEVEC(10000)", "https://github.com/pgvector/pgvector");
  t!("LTREE", "Hierarchical label tree (ltree extension). Use ~ and @> for ancestor queries.", "path LTREE", "https://www.postgresql.org/docs/current/ltree.html");
  t!("LQUERY", "ltree label path query.", "q LQUERY", "https://www.postgresql.org/docs/current/ltree.html");
  t!("LTXTQUERY", "ltree full-text query.", "q LTXTQUERY", "https://www.postgresql.org/docs/current/ltree.html");
  t!("CITEXT", "Case-insensitive text (citext extension). UNIQUE/= compare folded.", "email CITEXT", "https://www.postgresql.org/docs/current/citext.html");
  t!("ISBN", "ISBN identifier (isn extension).", "x ISBN", "https://www.postgresql.org/docs/current/isn.html");
  t!("ISSN", "ISSN identifier (isn extension).", "x ISSN", "https://www.postgresql.org/docs/current/isn.html");
  t!("CUBE", "Multi-dim cube (cube extension). Used by earthdistance.", "c CUBE", "https://www.postgresql.org/docs/current/cube.html");
  t!("EARTH", "earth-distance type (earthdistance extension).", "e EARTH", "https://www.postgresql.org/docs/current/earthdistance.html");
  t!("SEG", "Floating point intervals (seg extension).", "x SEG", "https://www.postgresql.org/docs/current/seg.html");
  // ---- PostGIS ----
  t!("GEOMETRY", "PostGIS geometry. Specify subtype/SRID: GEOMETRY(Point, 4326).", "geom GEOMETRY(Point, 4326)", "https://postgis.net/docs/geometry.html");
  t!("GEOGRAPHY", "PostGIS geography (spherical, computations in meters).", "loc GEOGRAPHY(Point, 4326)", "https://postgis.net/docs/geography.html");
  t!("RASTER", "PostGIS raster type.", "tile RASTER", "https://postgis.net/docs/raster.html");
  // ---- TimescaleDB ----
  t!("HYPERTABLE", "TimescaleDB virtual table -- not a real column type; created via create_hypertable().", "-- see create_hypertable()", "https://docs.timescale.com/");

  m
}
