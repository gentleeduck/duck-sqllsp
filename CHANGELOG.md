# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/gentleeduck/duck-sqllsp/releases/tag/dsl-knowledge-v0.1.0) - 2026-05-26

### Fixed

- sql348 false positive on DDL name slots + sql212 false positive on INSERT INTO + dsl-knowledge missing built-ins

### Other

- harden cycle 77: add numnode/ts_lexize/ts_parse/ts_token_type/ts_debug/tsvector_to_array/array_to_tsvector/strip/length to dsl-knowledge (text-search internals)
- harden cycle 63: add array_to_json/row_to_json/date_bin/timezone/isfinite to dsl-knowledge
- harden cycle 62: add cbrt/gcd/lcm/scale/min_scale/trim_scale/width_bucket math + regr_slope/intercept/r2/count/avgx/avgy/sxx/syy/sxy regression aggregates to dsl-knowledge
- harden cycle 60: add substr/trim/char_length/character_length/md5/position to dsl-knowledge
- harden cycle 51: sql045 distinguish RAISE NOTICE/INFO from RAISE EXCEPTION (only EXCEPTION terminates); sql180 use 1147981 count parity to detect inside function body; add 23 array/time fns to dsl-knowledge (array_ndims/upper/lower/dims/prepend/append/remove/replace/cat/position/positions/to_string/string_to_array/cardinality/trim_array + current_time/current_timestamp/current_date + clock/statement/transaction_timestamp + timeofday + pg_trigger_depth)
- harden cycle 45: dsl-knowledge +30 fns (quote_*/translate/repeat/reverse/replace/split_part/strpos/btrim/ltrim/rtrim/initcap/octet_length/bit_length/ord/get_bit/set_bit/get_byte/set_byte/convert_from/convert_to/convert/pg_relation_filepath/pg_get_viewdef/pg_get_function_arguments/pg_get_function_result/pg_get_functiondef/pg_get_triggerdef/pg_get_constraintdef/pg_terminate_backend/pg_cancel_backend); sql001 skip pg_* + information_schema.* tables
- harden cycle 43: sql351 advance past qualified.col so UPDATE..FROM/DELETE..USING WHERE b.x doesn't flag x as missing on a; add make_interval/make_time/make_timestamp to dsl-knowledge
- harden cycle 41: sql141 strip noise + dollar blocks; add pg_advisory_xact_lock/unlock variants + hashtext/hashbpchar to dsl-knowledge
- harden cycle 23: FTS fns setweight/ts_headline/plainto_tsquery + pg_trgm similarity to knowledge
- harden cycle 22: dsl-knowledge add jsonb_build_array + jsonb_object_agg + _text variants + json_object
- harden cycle 21: geometric type constructors point/box/circle/line/lseg/path/polygon to dsl-knowledge
- harden cycle 13: sql054 skip SET/RESET/ALTER SYSTEM + uuid_generate fns in knowledge
- harden cycle 9: dsl-knowledge math+date+sha + sql316 EXTRACT field skip
- harden cycle 5: window fns + GROUPING SETS/CUBE/ROLLUP + percent_rank/cume_dist
- +11 missing types (BIT/VARBIT/SMALLSERIAL/PG_LSN/LSEG/PATH/REG*)
- +50 builtins (trig, bit aggs, stats aggs, FTS, jsonb_path, regclass lookups, sizes, enums)
- 2-space reformat pass across all crates
- built-in range + multirange types
- flatten crates/ to root, rewrite README + delete CLAUDE_CONTEXT to duck-sqllsp
- batman
# Changelog

All notable changes to this project will be documented in this file.

