//! Registered lint rules.
//!
//! New rules: add the module, then push an instance into [`all`].

pub mod add_column_notnull_no_default;
pub mod function_arg_validation;
pub mod empty_expression_paren;
pub mod advisory_lock_literal_key;
pub mod advisory_lock_no_unlock;
pub mod after_trigger_return_row;
pub mod alter_add_check_no_not_valid;
pub mod alter_column_type;
pub mod alter_drop_just_created;
pub mod alter_set_not_null_scan;
pub mod alter_set_tablespace;
pub mod alter_table_no_owner;
pub mod alter_trigger_lock;
pub mod alter_type_add_value_in_tx;
pub mod alter_type_label_unknown;
pub mod ambiguous_column;
pub mod duplicate_alias;
pub mod order_by_unknown_column;
pub mod group_by_unknown_column;
pub mod having_unknown_column;
pub mod duplicate_dml_column;
pub mod where_always_false;
pub mod where_column_self_compare;
pub mod between_self_bound;
pub mod duplicate_select_projection;
pub mod limit_one_offset_no_order;
pub mod duplicate_order_or_group_item;
pub mod concat_with_null_literal;
pub mod in_list_self_member;
pub mod cast_same_type;
pub mod case_all_branches_same;
pub mod coalesce_dead_arg;
pub mod distinct_on_unique;
pub mod nullif_with_null_literal;
pub mod any_array_self_member;
pub mod duplicate_where_predicate;
pub mod where_pred_and_negation;
pub mod regex_prefix_could_be_like;
pub mod aggregate_in_where;
pub mod window_in_where;
pub mod distinct_order_by_must_be_in_projection;
pub mod wrap_blocks_index;
pub mod aggregate_star_only_count;
pub mod invalid_equality_operator;
pub mod select_star_with_named_columns;
pub mod for_update_in_setop;
pub mod case_duplicate_when;
pub mod order_by_constant;
pub mod redundant_is_not_null;
pub mod where_is_null_contradiction;
pub mod window_in_aggregate;
pub mod null_in_list;
pub mod generated_identity_with_default;
pub mod invalid_date_literal;
pub mod invalid_interval_unit;
pub mod uncorrelated_exists;
pub mod regexp_replace_no_global;
pub mod substring_negative_length;
pub mod generate_series_bad_step;
pub mod array_position_null;
pub mod position_empty_substring;
pub mod power_trivial_exponent;
pub mod lpad_rpad_negative;
pub mod jsonb_build_object_duplicate_key;
pub mod numeric_scale_exceeds_precision;
pub mod varchar_char_zero_length;
pub mod repeat_trivial_count;
pub mod array_length_missing_dim;
pub mod to_timestamp_hh_mm_confusion;
pub mod where_pred_or_negation;
pub mod int_literal_out_of_range;
pub mod positional_out_of_range;
pub mod sum_avg_of_boolean;
pub mod count_notnull_column;
pub mod having_without_aggregate;
pub mod array_func_null_array;
pub mod null_arithmetic;
pub mod tg_op_invalid_literal;
pub mod is_distinct_self;
pub mod concat_ws_empty_sep;
pub mod offset_zero;
pub mod empty_needle_string_fn;
pub mod greatest_least_all_null;
pub mod not_is_null;
pub mod not_paren_predicate;
pub mod distinct_inside_in_subquery;
pub mod extract_invalid_interval_field;
pub mod any_all_empty_array;
pub mod where_literal_literal;
pub mod insert_self_select;
pub mod case_when_null;
pub mod contains_empty_container;
pub mod contained_by_empty;
pub mod substring_zero_start;
pub mod group_by_constant;
pub mod position_empty_haystack;
pub mod having_constant;
pub mod split_part_zero_field;
pub mod partition_by_constant;
pub mod regexp_empty_pattern;
pub mod distinct_star;
pub mod array_dim_zero;
pub mod jsonpath_missing_anchor;
pub mod where_arith_identity;
pub mod concat_empty_string;
pub mod having_literal_literal;
pub mod not_in_null_list;
pub mod coalesce_not_null;
pub mod jsonb_set_empty_path;
pub mod eq_all_array;
pub mod set_default_no_default;
pub mod agg_distinct_order_mismatch;
pub mod similar_to_deprecated;
pub mod tsvector_text_literal;
pub mod date_minus_date;
pub mod order_nulls_on_not_null;
pub mod timestamp_lit_on_tstz_col;
pub mod jsonb_question_on_non_jsonb;
pub mod integer_division_truncation;
pub mod json_extract_on_text;
pub mod array_all_null;
pub mod execute_string_concat;
pub mod self_like;
pub mod pg_temp_explicit;
pub mod self_similar;
pub mod self_containment;
pub mod analyze_in_tx;
pub mod any_all_multicol;
pub mod array_elem_vs_col;
pub mod array_eq_with_null;
pub mod array_fn_on_scalar;
pub mod array_mixed_types;
pub mod array_subscript_zero;
pub mod backslash_in_string;
pub mod bare_return_typed;
pub mod begin_no_lock_mode;
pub mod between_reversed;
pub mod bool_agg_nullable;
pub mod bool_compare_equals;
pub mod boolean_in_text_column;
pub mod brin_small_table;
pub mod bytea_literal_no_escape;
pub mod case_branch_types;
pub mod case_no_else;
pub mod case_single_when;
pub mod cast_literal_invalid;
pub mod cast_text_in_distinct;
pub mod cast_text_to_int_in_where;
pub mod chained_comparison;
pub mod char_length_vs_length;
pub mod char_n_type;
pub mod character_varying_no_limit;
pub mod check_always_false;
pub mod check_always_true;
pub mod coalesce_single_arg;
pub mod column_default_volatile;
pub mod comment_clears_existing;
pub mod comment_constraint_no_on;
pub mod comment_fn_no_args;
pub mod comment_on_unknown;
pub mod commit_in_function;
pub mod copy_file_path;
pub mod copy_header_no_csv;
pub mod copy_no_format;
pub mod copy_program_exec;
pub mod count_nullable;
pub mod count_one_vs_star;
pub mod count_star_returns_one;
pub mod create_table_no_schema;
pub mod cte_dml_no_returning;
pub mod cte_missing_recursive;
pub mod current_setting_no_missing_ok;
pub mod cursor_with_hold_no_tx;
pub mod date_literal_format;
pub mod ddl_in_immutable;
pub mod deep_case_nesting;
pub mod default_references_column;
pub mod default_values_no_default_col;
pub mod delete_no_where_in_fn;
pub mod deprecated_function;
pub mod distinct_after_group_by;
pub mod distinct_on_no_order;
pub mod distinct_on_subq_no_order;
pub mod distinct_with_aggregate;
pub mod dml_where_unknown_column;
pub mod do_block_bare_select;
pub mod drop_cascade_chain;
pub mod drop_column_fk;
pub mod drop_function_no_args;
pub mod drop_index_concurrently_in_tx;
pub mod drop_role_no_reassign;
pub mod drop_schema_no_cascade;
pub mod drop_table_no_if_exists;
pub mod empty_array_no_cast;
pub mod empty_comment;
pub mod empty_in_list;
pub mod exists_select_star;
pub mod exit_outside_loop;
pub mod explain_analyze_in_dml;
pub mod extension_no_if_not_exists;
pub mod extract_on_indexable;
pub mod extract_unknown_field;
pub mod constraint_unknown_column;
pub mod fk_target_not_unique;
pub mod fk_unknown_column;
pub mod for_update_aggregate;
pub mod for_update_left_join;
pub mod for_update_of_unknown;
pub mod for_update_on_view;
pub mod format_no_placeholders;
pub mod generate_series_no_alias;
pub mod generated_uses_volatile;
pub mod gin_on_scalar;
pub mod gist_on_scalar;
pub mod grant_all_too_broad;
pub mod grant_to_public;
pub mod grant_with_grant_option;
pub mod group_by_alias;
pub mod group_by_all;
pub mod group_by_position;
pub mod group_by_required;
pub mod identifier_too_long;
pub mod immutable_calls_volatile;
pub mod implicit_cross_join;
pub mod between_equal_bounds;
pub mod boolean_literal_dominates;
pub mod now_cast_to_date;
pub mod coalesce_nested;
pub mod distinct_looks_like_function;
pub mod length_compare_zero;
pub mod group_by_aggregate;
pub mod extract_value_out_of_range;
pub mod array_length_zero_check;
pub mod table_self_alias;
pub mod redundant_nested_function;
pub mod like_operator_form;
pub mod is_true_redundant;
pub mod create_table_dup_column;
pub mod index_dup_column;
pub mod limit_all_redundant;
pub mod any_array_duplicate;
pub mod self_arithmetic;
pub mod builtin_too_few_args;
pub mod exists_order_by;
pub mod plaintext_password;
pub mod role_bypassrls;
pub mod policy_using_true;
pub mod view_order_by;
pub mod autovacuum_disabled;
pub mod json_prefer_jsonb;
pub mod exists_group_by;
pub mod cluster_locks_table;
pub mod add_column_volatile_default;
pub mod add_fk_not_valid;
pub mod values_inconsistent_length;
pub mod mysql_limit_comma;
pub mod mysql_replace_into;
pub mod mysql_regexp_operator;
pub mod use_statement;
pub mod mysql_unsigned;
pub mod backtick_identifier;
pub mod oracle_varchar2;
pub mod oracle_decode;
pub mod oracle_minus;
pub mod oracle_lob_types;
pub mod fk_set_null_not_null;
pub mod check_subquery;
pub mod type_no_modifier;
pub mod unique_index_non_btree;
pub mod for_update_distinct;
pub mod for_update_window;
pub mod update_delete_order_by;
pub mod returning_aggregate;
pub mod generated_column_not_stored;
pub mod mysql_inline_index;
pub mod with_oids;
pub mod mysql_character_set;
pub mod natural_join;
pub mod with_ties_no_order;
pub mod date_trunc_invalid_unit;
pub mod non_pg_date_diff_fns;
pub mod mysql_if_function;
pub mod mysql_string_functions;
pub mod mysql_enum_inline;
pub mod mysql_on_update_timestamp;
pub mod mysql_zerofill;
pub mod mysql_query_modifiers;
pub mod mysql_xor_div;
pub mod cross_dialect_scalar_fns;
pub mod tsql_types;
pub mod tsql_identity_fns;
pub mod last_value_default_frame;
pub mod large_object_file_access;
pub mod server_file_read_fns;
pub mod weak_gen_salt;
pub mod sqlite_pragma;
pub mod sqlite_autoincrement;
pub mod sqlite_glob;
pub mod sqlite_functions;
pub mod cross_dialect_string_fns2;
pub mod mysql_datepart_fns;
pub mod default_relative_datetime_string;
pub mod mysql_file_io;
pub mod oracle_date_misc_fns;
pub mod mysql_date_arith_fns;
pub mod srf_in_where;
pub mod aggregate_distinct_star;
pub mod in_subquery_multi_column;
pub mod tablesample_out_of_range;
pub mod on_conflict_do_update_no_target;
pub mod row_comparison_arity;
pub mod srf_in_group_order;
pub mod duplicate_cte_name;
pub mod aggregate_in_check;
pub mod aggregate_in_index;
pub mod update_set_arity;
pub mod truncate_with_where;
pub mod order_by_after_limit;
pub mod limit_and_fetch;
pub mod where_after_group_by;
pub mod cross_join_with_on;
pub mod window_fn_without_over;
pub mod distinct_on_no_parens;
pub mod tail_clause_before_setop;
pub mod having_before_group_by;
pub mod update_where_before_set;
pub mod mysql_insert_ignore;
pub mod mysql_insert_set;
pub mod delete_alias_before_from;
pub mod mysql_lock_in_share_mode;
pub mod mysql_show_statement;
pub mod mysql_describe;
pub mod mysql_alter_change_modify;
pub mod mysql_functions;
pub mod mysql_on_duplicate_key;
pub mod where_bare_integer;
pub mod between_null_bound;
pub mod ranking_fn_with_frame;
pub mod union_branch_distinct;
pub mod count_distinct_constant;
pub mod modulo_by_one;
pub mod zero_date_literal;
pub mod left_right_zero;
pub mod substring_zero_length;
pub mod multiply_by_zero;
pub mod coalesce_count_redundant;
pub mod case_constant_when;
pub mod greatest_least_null_arg;
pub mod power_base_one;
pub mod not_not_double_negation;
pub mod coalesce_constant_first;
pub mod modulo_self;
pub mod sqrt_negative_literal;
pub mod min_max_distinct;
pub mod ln_log_nonpositive_literal;
pub mod log_base_one;
pub mod acos_asin_domain;
pub mod nested_aggregate;
pub mod count_of_coalesce;
pub mod degrees_radians_roundtrip;
pub mod chr_zero;
pub mod lpad_rpad_zero;
pub mod setseed_out_of_range;
pub mod nullif_distinct_literals;
pub mod coalesce_is_null_always_false;
pub mod ntile_nonpositive;
pub mod nth_value_nonpositive;
pub mod width_bucket_nonpositive_count;
pub mod lag_lead_zero_offset;
pub mod jsonb_typeof_invalid_literal;
pub mod coalesce_is_not_null_always_true;
pub mod bitand_zero;
pub mod bitwise_self;
pub mod starts_with_empty_string;
pub mod translate_empty_from;
pub mod to_char_empty_format;
pub mod repeat_one;
pub mod power_zero_negative_exponent;
pub mod factorial_negative;
pub mod random_compare_out_of_range;
pub mod ascii_empty_string;
pub mod exp_ln_roundtrip;
pub mod bitor_zero;
pub mod bitshift_zero;
pub mod chr_above_max;
pub mod ln_log_one;
pub mod acosh_atanh_domain;
pub mod exists_aggregate;
pub mod width_bucket_equal_bounds;
pub mod nonneg_func_negative_compare2;
pub mod not_boolean_literal;
pub mod modulo_by_negative_one;
pub mod percentile_fraction_out_of_range;
pub mod reindex_not_concurrent;
pub mod alter_add_key_lock;
pub mod vacuum_full_locks;
pub mod internal_type_alias;
pub mod money_type;
pub mod unlogged_table;
pub mod create_rule_legacy;
pub mod disable_trigger_all;
pub mod disable_row_level_security;
pub mod role_superuser;
pub mod exists_distinct;
pub mod regex_literal_could_be_like;
pub mod col_eq_col_offset;
pub mod null_not_null_conflict;
pub mod default_subquery;
pub mod fk_column_count_mismatch;
pub mod multiple_primary_keys;
pub mod eq_any_array_to_in;
pub mod default_null_redundant;
pub mod nonneg_func_negative_compare;
pub mod redundant_range_bound;
pub mod neq_all_array_to_not_in;
pub mod modulo_out_of_range;
pub mod range_is_equality;
pub mod neq_chain_to_not_in;
pub mod not_paren_comparison;
pub mod on_conflict_self_assignment;
pub mod round_trunc_zero_scale;
pub mod greatest_least_dup_arg;
pub mod having_count_always_true;
pub mod redundant_column_alias;
pub mod replace_same_from_to;
pub mod setop_identical_branches;
pub mod in_list_duplicates;
pub mod eq_contradiction;
pub mod impossible_range;
pub mod like_all_wildcard;
pub mod exists_with_limit;
pub mod left_join_defeated_by_where;
pub mod is_null_or_is_not_null;
pub mod case_fold_impossible_compare;
pub mod any_all_single_element_array;
pub mod case_boolean_redundant;
pub mod or_chain_to_in;
pub mod in_list_single_value;
pub mod update_self_assignment;
pub mod join_on_constant_tautology;
pub mod index_concurrently_in_tx;
pub mod index_expr_volatile;
pub mod index_no_name;
pub mod information_schema_perf;
pub mod inline_check_other_col;
pub mod insert_col_value_count;
pub mod insert_into_generated;
pub mod insert_no_columns;
pub mod insert_no_on_conflict;
pub mod insert_subquery_col_count;
pub mod insert_type_literal;
pub mod insert_unknown_column;
pub mod int_range;
pub mod is_distinct_null;
pub mod is_null_on_not_null;
pub mod join_no_on;
pub mod jsonb_build_odd_args;
pub mod jsonb_contains_no_cast;
pub mod jsonb_no_cast;
pub mod jsonb_set_no_create;
pub mod jsonb_set_path_format;
pub mod lateral_no_ref;
pub mod like_include_indexes_partition;
pub mod like_leading_wildcard;
pub mod like_with_no_collation;
pub mod like_without_wildcard;
pub mod limit_for_update_subq;
pub mod limit_without_order;
pub mod limit_zero;
pub mod listen_unbounded;
pub mod literal_div_zero;
pub mod lock_table_no_tx;
pub mod long_in_list;
pub mod merge_missing_when;
pub mod missing_lateral;
pub mod missing_primary_key;
pub mod missing_trigger_return;
pub mod mssql_begin_tran;
pub mod mssql_bracket_quote;
pub mod mssql_go;
pub mod mssql_top;
pub mod multi_raise_exception;
pub mod multi_where;
pub mod multiple_truncate_in_tx;
pub mod mutating_without_where;
pub mod mv_no_data_query;
pub mod mysql_auto_increment;
pub mod mysql_engine;
pub mod mysql_interval_syntax;
pub mod mysql_table_comment;
pub mod mysql_types;
pub mod negative_limit_offset;
pub mod nested_begin;
pub mod new_assign_pk_in_before_insert;
pub mod non_pg_date_fns;
pub mod non_pg_null_fns;
pub mod not_in_nullable;
pub mod not_in_subquery;
pub mod notify_payload_too_large;
pub mod notify_unlistened;
pub mod now_default_on_timestamp;
pub mod null_comparison;
pub mod null_default_not_null;
pub mod null_in_values;
pub mod null_into_not_null;
pub mod nullif_same_args;
pub mod nullif_type_mismatch;
pub mod nulls_first_last_no_order;
pub mod numeric_no_precision;
pub mod on_conflict_do_nothing;
pub mod on_conflict_no_unique;
pub mod on_update_cascade_pk;
pub mod oracle_connect_by;
pub mod oracle_dual;
pub mod oracle_outer_join;
pub mod oracle_rownum;
pub mod order_by_in_subquery;
pub mod order_by_position;
pub mod order_by_random;
pub mod order_by_using_noncomparable;
pub mod owner_to_unknown_role;
pub mod percentile_no_within;
pub mod percentile_non_numeric_order;
pub mod perform_for_pure_select;
pub mod pg_catalog_no_schema;
pub mod pg_sleep_in_tx;
pub mod pg_terminate_backend;
pub mod pk_duplicate_col;
pub mod plpgsql_assign_type;
pub mod prefer_alias;
pub mod psql_backslash;
pub mod raise_arg_count;
pub mod raise_message_no_args;
pub mod raise_no_level;
pub mod raise_using_errcode;
pub mod recursive_cte_no_union;
pub mod redundant_index_on_pk;
pub mod redundant_parens;
pub mod redundant_unique_index;
pub mod reindex_in_tx;
pub mod reindex_system;
pub mod rename_column_breaks_view;
pub mod reserved_word_identifier;
pub mod return_type_literal;
pub mod returning_no_assign;
pub mod returning_unknown_column;
pub mod returning_with_truncate;
pub mod revoke_cascade;
pub mod revoke_missing_from;
pub mod revoke_without_grant;
pub mod rollback_outside_tx;
pub mod rollup_cube_single;
pub mod row_constructor_single;
pub mod row_count_after_dml;
pub mod savepoint_name_reuse;
pub mod savepoint_no_release;
pub mod savepoint_outside_tx;
pub mod schema_drift;
pub mod secdef_no_search_path;
pub mod select_for_update_in_recursive_cte;
pub mod select_for_update_no_where;
pub mod select_into_existing;
pub mod select_into_outside_plpgsql;
pub mod select_into_shape;
pub mod select_into_strict_no_exception;
pub mod select_into_temp_shadows;
pub mod select_no_from_no_agg;
pub mod select_star_insert;
pub mod select_trailing_comma;
pub mod self_fk_no_deferrable;
pub mod serial_vs_identity;
pub mod set_constraints_outside_tx;
pub mod set_local_outside_tx;
pub mod set_role_in_function;
pub mod set_role_no_reset;
pub mod set_transaction_in_function;
pub mod setseed_no_determinism_guard;
pub mod shell_command_in_sql;
pub mod single_stmt_transaction;
pub mod sql_lang_uses_new_old;
pub mod star_with_order_by_position;
pub mod string_agg_no_order;
pub mod substring_from_no_for;
pub mod system_catalog_dml;
pub mod table_inherits;
pub mod tablespace_specified;
pub mod text_int_arithmetic;
pub mod tg_var_in_non_trigger;
pub mod time_with_timezone;
pub mod timestamp_int_arithmetic;
pub mod timestamp_precision_over;
pub mod timestamp_without_tz;
pub mod trailing_comma_values;
pub mod transaction_isolation_no_set;
pub mod trigger_stmt_uses_new;
pub mod trigger_when_uses_new_in_delete;
pub mod trigger_when_uses_old_in_insert;
pub mod trigger_wrong_row_alias;
pub mod truncate_in_plpgsql_exception;
pub mod truncate_in_trigger;
pub mod truncate_no_cascade;
pub mod truncate_with_fk;
pub mod union_column_count;
pub mod union_inner_order_by;
pub mod union_vs_all;
pub mod unique_on_nullable;
pub mod unknown_column;
pub mod unknown_function;
pub mod unreachable_after_return;
pub mod unresolved_table;
pub mod update_delete_limit;
pub mod update_from_no_pk_filter;
pub mod update_set_alias_mismatch;
pub mod update_set_no_change;
pub mod update_set_type_literal;
pub mod update_set_unknown_col;
pub mod using_clause_columns;
pub mod uuid_literal_format;
pub mod vacuum_in_transaction;
pub mod values_row_width;
pub mod values_subq_no_alias;
pub mod varchar_length;
pub mod view_select_star;
pub mod where_true_placeholder;
pub mod where_type_literal;
pub mod window_frame_reversed;
pub mod window_no_order;

use crate::LintRule;

pub fn all() -> Vec<Box<dyn LintRule>> {
  vec![
    Box::new(unresolved_table::Rule),
    Box::new(unknown_column::Rule),
    Box::new(ambiguous_column::Rule),
    Box::new(duplicate_alias::Rule),
    Box::new(order_by_unknown_column::Rule),
    Box::new(group_by_unknown_column::Rule),
    Box::new(having_unknown_column::Rule),
    Box::new(duplicate_dml_column::Rule),
    Box::new(where_always_false::Rule),
    Box::new(where_column_self_compare::Rule),
    Box::new(between_self_bound::Rule),
    Box::new(duplicate_select_projection::Rule),
    Box::new(limit_one_offset_no_order::Rule),
    Box::new(duplicate_order_or_group_item::Rule),
    Box::new(concat_with_null_literal::Rule),
    Box::new(in_list_self_member::Rule),
    Box::new(cast_same_type::Rule),
    Box::new(case_all_branches_same::Rule),
    Box::new(coalesce_dead_arg::Rule),
    Box::new(distinct_on_unique::Rule),
    Box::new(nullif_with_null_literal::Rule),
    Box::new(any_array_self_member::Rule),
    Box::new(duplicate_where_predicate::Rule),
    Box::new(where_pred_and_negation::Rule),
    Box::new(regex_prefix_could_be_like::Rule),
    Box::new(aggregate_in_where::Rule),
    Box::new(window_in_where::Rule),
    Box::new(distinct_order_by_must_be_in_projection::Rule),
    Box::new(wrap_blocks_index::Rule),
    Box::new(aggregate_star_only_count::Rule),
    Box::new(invalid_equality_operator::Rule),
    Box::new(select_star_with_named_columns::Rule),
    Box::new(for_update_in_setop::Rule),
    Box::new(case_duplicate_when::Rule),
    Box::new(order_by_constant::Rule),
    Box::new(redundant_is_not_null::Rule),
    Box::new(where_is_null_contradiction::Rule),
    Box::new(window_in_aggregate::Rule),
    Box::new(null_in_list::Rule),
    Box::new(generated_identity_with_default::Rule),
    Box::new(invalid_date_literal::Rule),
    Box::new(invalid_interval_unit::Rule),
    Box::new(uncorrelated_exists::Rule),
    Box::new(regexp_replace_no_global::Rule),
    Box::new(substring_negative_length::Rule),
    Box::new(generate_series_bad_step::Rule),
    Box::new(array_position_null::Rule),
    Box::new(position_empty_substring::Rule),
    Box::new(power_trivial_exponent::Rule),
    Box::new(lpad_rpad_negative::Rule),
    Box::new(jsonb_build_object_duplicate_key::Rule),
    Box::new(numeric_scale_exceeds_precision::Rule),
    Box::new(varchar_char_zero_length::Rule),
    Box::new(repeat_trivial_count::Rule),
    Box::new(array_length_missing_dim::Rule),
    Box::new(to_timestamp_hh_mm_confusion::Rule),
    Box::new(where_pred_or_negation::Rule),
    Box::new(int_literal_out_of_range::Rule),
    Box::new(positional_out_of_range::Rule),
    Box::new(sum_avg_of_boolean::Rule),
    Box::new(count_notnull_column::Rule),
    Box::new(having_without_aggregate::Rule),
    Box::new(array_func_null_array::Rule),
    Box::new(null_arithmetic::Rule),
    Box::new(tg_op_invalid_literal::Rule),
    Box::new(is_distinct_self::Rule),
    Box::new(concat_ws_empty_sep::Rule),
    Box::new(offset_zero::Rule),
    Box::new(empty_needle_string_fn::Rule),
    Box::new(greatest_least_all_null::Rule),
    Box::new(not_is_null::Rule),
    Box::new(not_paren_predicate::Rule),
    Box::new(distinct_inside_in_subquery::Rule),
    Box::new(extract_invalid_interval_field::Rule),
    Box::new(any_all_empty_array::Rule),
    Box::new(where_literal_literal::Rule),
    Box::new(insert_self_select::Rule),
    Box::new(case_when_null::Rule),
    Box::new(contains_empty_container::Rule),
    Box::new(contained_by_empty::Rule),
    Box::new(substring_zero_start::Rule),
    Box::new(group_by_constant::Rule),
    Box::new(position_empty_haystack::Rule),
    Box::new(having_constant::Rule),
    Box::new(split_part_zero_field::Rule),
    Box::new(partition_by_constant::Rule),
    Box::new(regexp_empty_pattern::Rule),
    Box::new(distinct_star::Rule),
    Box::new(array_dim_zero::Rule),
    Box::new(jsonpath_missing_anchor::Rule),
    Box::new(where_arith_identity::Rule),
    Box::new(concat_empty_string::Rule),
    Box::new(having_literal_literal::Rule),
    Box::new(not_in_null_list::Rule),
    Box::new(coalesce_not_null::Rule),
    Box::new(jsonb_set_empty_path::Rule),
    Box::new(eq_all_array::Rule),
    Box::new(set_default_no_default::Rule),
    Box::new(agg_distinct_order_mismatch::Rule),
    Box::new(similar_to_deprecated::Rule),
    Box::new(tsvector_text_literal::Rule),
    Box::new(date_minus_date::Rule),
    Box::new(order_nulls_on_not_null::Rule),
    Box::new(timestamp_lit_on_tstz_col::Rule),
    Box::new(jsonb_question_on_non_jsonb::Rule),
    Box::new(integer_division_truncation::Rule),
    Box::new(json_extract_on_text::Rule),
    Box::new(array_all_null::Rule),
    Box::new(execute_string_concat::Rule),
    Box::new(self_like::Rule),
    Box::new(pg_temp_explicit::Rule),
    Box::new(self_similar::Rule),
    Box::new(self_containment::Rule),
    Box::new(mutating_without_where::Rule),
    Box::new(null_comparison::Rule),
    Box::new(implicit_cross_join::Rule),
    Box::new(select_star_insert::Rule),
    Box::new(deprecated_function::Rule),
    Box::new(union_column_count::Rule),
    Box::new(group_by_required::Rule),
    Box::new(not_in_subquery::Rule),
    Box::new(prefer_alias::Rule),
    Box::new(missing_trigger_return::Rule),
    Box::new(exit_outside_loop::Rule),
    Box::new(bare_return_typed::Rule),
    Box::new(sql_lang_uses_new_old::Rule),
    Box::new(raise_arg_count::Rule),
    Box::new(unreachable_after_return::Rule),
    Box::new(delete_no_where_in_fn::Rule),
    Box::new(select_into_shape::Rule),
    Box::new(update_set_unknown_col::Rule),
    Box::new(immutable_calls_volatile::Rule),
    Box::new(insert_col_value_count::Rule),
    Box::new(return_type_literal::Rule),
    Box::new(insert_type_literal::Rule),
    Box::new(missing_primary_key::Rule),
    Box::new(insert_no_columns::Rule),
    Box::new(reserved_word_identifier::Rule),
    Box::new(limit_without_order::Rule),
    Box::new(bool_compare_equals::Rule),
    Box::new(like_without_wildcard::Rule),
    Box::new(union_vs_all::Rule),
    Box::new(redundant_parens::Rule),
    Box::new(case_single_when::Rule),
    Box::new(savepoint_no_release::Rule),
    Box::new(join_no_on::Rule),
    Box::new(group_by_position::Rule),
    Box::new(null_in_values::Rule),
    Box::new(single_stmt_transaction::Rule),
    Box::new(null_default_not_null::Rule),
    Box::new(select_for_update_no_where::Rule),
    Box::new(long_in_list::Rule),
    Box::new(time_with_timezone::Rule),
    Box::new(negative_limit_offset::Rule),
    Box::new(order_by_random::Rule),
    Box::new(insert_no_on_conflict::Rule),
    Box::new(nullif_same_args::Rule),
    Box::new(between_reversed::Rule),
    Box::new(like_leading_wildcard::Rule),
    Box::new(empty_comment::Rule),
    Box::new(distinct_with_aggregate::Rule),
    Box::new(deep_case_nesting::Rule),
    Box::new(count_one_vs_star::Rule),
    Box::new(trailing_comma_values::Rule),
    Box::new(select_no_from_no_agg::Rule),
    Box::new(multi_raise_exception::Rule),
    Box::new(group_by_all::Rule),
    Box::new(is_distinct_null::Rule),
    Box::new(multi_where::Rule),
    Box::new(order_by_position::Rule),
    Box::new(distinct_on_no_order::Rule),
    Box::new(char_n_type::Rule),
    Box::new(truncate_no_cascade::Rule),
    Box::new(char_length_vs_length::Rule),
    Box::new(lock_table_no_tx::Rule),
    Box::new(generate_series_no_alias::Rule),
    Box::new(jsonb_no_cast::Rule),
    Box::new(timestamp_without_tz::Rule),
    Box::new(jsonb_set_no_create::Rule),
    Box::new(numeric_no_precision::Rule),
    Box::new(distinct_after_group_by::Rule),
    Box::new(cast_text_to_int_in_where::Rule),
    Box::new(backslash_in_string::Rule),
    Box::new(boolean_in_text_column::Rule),
    Box::new(like_with_no_collation::Rule),
    Box::new(select_into_outside_plpgsql::Rule),
    Box::new(cte_missing_recursive::Rule),
    Box::new(explain_analyze_in_dml::Rule),
    Box::new(grant_to_public::Rule),
    Box::new(update_from_no_pk_filter::Rule),
    Box::new(transaction_isolation_no_set::Rule),
    Box::new(raise_message_no_args::Rule),
    Box::new(vacuum_in_transaction::Rule),
    Box::new(multiple_truncate_in_tx::Rule),
    // sql129 alter_table_no_owner unregistered -- too noisy for
    // teams that set ownership at DB level. Module retained for
    // future opt-in via config.
    // Box::new(alter_table_no_owner::Rule),
    Box::new(copy_no_format::Rule),
    Box::new(select_for_update_in_recursive_cte::Rule),
    Box::new(listen_unbounded::Rule),
    Box::new(set_role_no_reset::Rule),
    Box::new(trigger_when_uses_old_in_insert::Rule),
    Box::new(alter_type_add_value_in_tx::Rule),
    Box::new(grant_with_grant_option::Rule),
    Box::new(ddl_in_immutable::Rule),
    Box::new(column_default_volatile::Rule),
    Box::new(character_varying_no_limit::Rule),
    Box::new(array_subscript_zero::Rule),
    Box::new(trigger_when_uses_new_in_delete::Rule),
    Box::new(case_no_else::Rule),
    Box::new(update_set_no_change::Rule),
    Box::new(returning_no_assign::Rule),
    Box::new(row_count_after_dml::Rule),
    Box::new(unique_on_nullable::Rule),
    Box::new(returning_with_truncate::Rule),
    Box::new(cast_text_in_distinct::Rule),
    Box::new(select_into_strict_no_exception::Rule),
    Box::new(timestamp_int_arithmetic::Rule),
    Box::new(advisory_lock_no_unlock::Rule),
    Box::new(raise_using_errcode::Rule),
    Box::new(perform_for_pure_select::Rule),
    Box::new(trigger_stmt_uses_new::Rule),
    Box::new(count_star_returns_one::Rule),
    Box::new(text_int_arithmetic::Rule),
    Box::new(missing_lateral::Rule),
    Box::new(row_constructor_single::Rule),
    Box::new(redundant_index_on_pk::Rule),
    Box::new(begin_no_lock_mode::Rule),
    Box::new(redundant_unique_index::Rule),
    // sql169 owner_to_unknown_role: re-registered per user request.
    // Only fires when catalog.roles is populated AND the role is
    // neither in the catalog nor in the postgres/pg_* whitelist nor
    // a CURRENT_USER / SESSION_USER / CURRENT_ROLE built-in.
    Box::new(owner_to_unknown_role::Rule),
    Box::new(plpgsql_assign_type::Rule),
    Box::new(update_set_type_literal::Rule),
    Box::new(where_type_literal::Rule),
    Box::new(schema_drift::Rule),
    Box::new(count_nullable::Rule),
    Box::new(for_update_on_view::Rule),
    Box::new(is_null_on_not_null::Rule),
    Box::new(null_into_not_null::Rule),
    Box::new(insert_into_generated::Rule),
    Box::new(savepoint_outside_tx::Rule),
    Box::new(truncate_in_trigger::Rule),
    Box::new(varchar_length::Rule),
    Box::new(date_literal_format::Rule),
    Box::new(uuid_literal_format::Rule),
    Box::new(using_clause_columns::Rule),
    Box::new(int_range::Rule),
    Box::new(fk_unknown_column::Rule),
    Box::new(constraint_unknown_column::Rule),
    Box::new(drop_column_fk::Rule),
    Box::new(comment_on_unknown::Rule),
    Box::new(alter_column_type::Rule),
    Box::new(on_conflict_no_unique::Rule),
    Box::new(window_frame_reversed::Rule),
    Box::new(for_update_of_unknown::Rule),
    Box::new(generated_uses_volatile::Rule),
    Box::new(truncate_with_fk::Rule),
    Box::new(cast_literal_invalid::Rule),
    Box::new(fk_target_not_unique::Rule),
    Box::new(array_fn_on_scalar::Rule),
    Box::new(inline_check_other_col::Rule),
    Box::new(default_references_column::Rule),
    Box::new(lateral_no_ref::Rule),
    Box::new(secdef_no_search_path::Rule),
    Box::new(trigger_wrong_row_alias::Rule),
    Box::new(raise_no_level::Rule),
    Box::new(update_set_alias_mismatch::Rule),
    Box::new(notify_unlistened::Rule),
    Box::new(insert_subquery_col_count::Rule),
    Box::new(coalesce_single_arg::Rule),
    Box::new(extract_unknown_field::Rule),
    Box::new(copy_file_path::Rule),
    Box::new(reindex_system::Rule),
    Box::new(rollback_outside_tx::Rule),
    Box::new(select_into_existing::Rule),
    Box::new(index_expr_volatile::Rule),
    Box::new(index_concurrently_in_tx::Rule),
    Box::new(rollup_cube_single::Rule),
    Box::new(values_row_width::Rule),
    Box::new(for_update_left_join::Rule),
    Box::new(case_branch_types::Rule),
    Box::new(commit_in_function::Rule),
    Box::new(recursive_cte_no_union::Rule),
    Box::new(array_mixed_types::Rule),
    Box::new(limit_for_update_subq::Rule),
    Box::new(jsonb_set_path_format::Rule),
    Box::new(set_constraints_outside_tx::Rule),
    Box::new(comment_clears_existing::Rule),
    Box::new(drop_cascade_chain::Rule),
    Box::new(exists_select_star::Rule),
    Box::new(any_all_multicol::Rule),
    Box::new(cte_dml_no_returning::Rule),
    Box::new(gin_on_scalar::Rule),
    Box::new(nulls_first_last_no_order::Rule),
    Box::new(jsonb_contains_no_cast::Rule),
    Box::new(mv_no_data_query::Rule),
    Box::new(empty_in_list::Rule),
    Box::new(pg_sleep_in_tx::Rule),
    Box::new(after_trigger_return_row::Rule),
    Box::new(shell_command_in_sql::Rule),
    Box::new(array_eq_with_null::Rule),
    Box::new(alter_drop_just_created::Rule),
    Box::new(savepoint_name_reuse::Rule),
    Box::new(view_select_star::Rule),
    Box::new(drop_schema_no_cascade::Rule),
    Box::new(values_subq_no_alias::Rule),
    Box::new(check_always_true::Rule),
    Box::new(pg_catalog_no_schema::Rule),
    Box::new(on_conflict_do_nothing::Rule),
    Box::new(advisory_lock_literal_key::Rule),
    Box::new(add_column_notnull_no_default::Rule),
    Box::new(default_values_no_default_col::Rule),
    Box::new(for_update_aggregate::Rule),
    Box::new(star_with_order_by_position::Rule),
    Box::new(order_by_in_subquery::Rule),
    Box::new(not_in_nullable::Rule),
    Box::new(alter_set_tablespace::Rule),
    Box::new(window_no_order::Rule),
    Box::new(current_setting_no_missing_ok::Rule),
    Box::new(do_block_bare_select::Rule),
    Box::new(set_local_outside_tx::Rule),
    Box::new(set_role_in_function::Rule),
    Box::new(drop_function_no_args::Rule),
    Box::new(merge_missing_when::Rule),
    Box::new(extension_no_if_not_exists::Rule),
    Box::new(distinct_on_subq_no_order::Rule),
    Box::new(system_catalog_dml::Rule),
    Box::new(now_default_on_timestamp::Rule),
    Box::new(jsonb_build_odd_args::Rule),
    Box::new(chained_comparison::Rule),
    Box::new(union_inner_order_by::Rule),
    Box::new(extract_on_indexable::Rule),
    Box::new(format_no_placeholders::Rule),
    Box::new(cursor_with_hold_no_tx::Rule),
    Box::new(gist_on_scalar::Rule),
    Box::new(check_always_false::Rule),
    Box::new(select_into_temp_shadows::Rule),
    Box::new(set_transaction_in_function::Rule),
    Box::new(mysql_interval_syntax::Rule),
    Box::new(comment_fn_no_args::Rule),
    Box::new(literal_div_zero::Rule),
    Box::new(comment_constraint_no_on::Rule),
    Box::new(alter_add_check_no_not_valid::Rule),
    Box::new(alter_set_not_null_scan::Rule),
    Box::new(where_true_placeholder::Rule),
    Box::new(analyze_in_tx::Rule),
    Box::new(tg_var_in_non_trigger::Rule),
    Box::new(drop_role_no_reassign::Rule),
    Box::new(alter_type_label_unknown::Rule),
    Box::new(revoke_cascade::Rule),
    Box::new(index_no_name::Rule),
    Box::new(table_inherits::Rule),
    Box::new(percentile_no_within::Rule),
    Box::new(grant_all_too_broad::Rule),
    Box::new(limit_zero::Rule),
    Box::new(nullif_type_mismatch::Rule),
    Box::new(nested_begin::Rule),
    Box::new(copy_header_no_csv::Rule),
    Box::new(reindex_in_tx::Rule),
    Box::new(notify_payload_too_large::Rule),
    Box::new(identifier_too_long::Rule),
    Box::new(pk_duplicate_col::Rule),
    Box::new(select_trailing_comma::Rule),
    Box::new(copy_program_exec::Rule),
    Box::new(drop_table_no_if_exists::Rule),
    Box::new(empty_array_no_cast::Rule),
    Box::new(self_fk_no_deferrable::Rule),
    Box::new(information_schema_perf::Rule),
    Box::new(having_count_always_true::Rule),
    Box::new(between_equal_bounds::Rule),
    Box::new(boolean_literal_dominates::Rule),
    Box::new(now_cast_to_date::Rule),
    Box::new(greatest_least_dup_arg::Rule),
    Box::new(distinct_looks_like_function::Rule),
    Box::new(length_compare_zero::Rule),
    Box::new(group_by_aggregate::Rule),
    Box::new(extract_value_out_of_range::Rule),
    Box::new(array_length_zero_check::Rule),
    Box::new(table_self_alias::Rule),
    Box::new(redundant_nested_function::Rule),
    Box::new(like_operator_form::Rule),
    Box::new(is_true_redundant::Rule),
    Box::new(create_table_dup_column::Rule),
    Box::new(index_dup_column::Rule),
    Box::new(limit_all_redundant::Rule),
    Box::new(any_array_duplicate::Rule),
    Box::new(self_arithmetic::Rule),
    Box::new(builtin_too_few_args::Rule),
    Box::new(exists_order_by::Rule),
    Box::new(plaintext_password::Rule),
    Box::new(role_bypassrls::Rule),
    Box::new(policy_using_true::Rule),
    Box::new(view_order_by::Rule),
    Box::new(autovacuum_disabled::Rule),
    Box::new(json_prefer_jsonb::Rule),
    Box::new(exists_group_by::Rule),
    Box::new(cluster_locks_table::Rule),
    Box::new(add_column_volatile_default::Rule),
    Box::new(add_fk_not_valid::Rule),
    Box::new(values_inconsistent_length::Rule),
    Box::new(mysql_limit_comma::Rule),
    Box::new(mysql_replace_into::Rule),
    Box::new(mysql_regexp_operator::Rule),
    Box::new(use_statement::Rule),
    Box::new(mysql_unsigned::Rule),
    Box::new(backtick_identifier::Rule),
    Box::new(oracle_varchar2::Rule),
    Box::new(oracle_decode::Rule),
    Box::new(oracle_minus::Rule),
    Box::new(oracle_lob_types::Rule),
    Box::new(fk_set_null_not_null::Rule),
    Box::new(check_subquery::Rule),
    Box::new(type_no_modifier::Rule),
    Box::new(unique_index_non_btree::Rule),
    Box::new(for_update_distinct::Rule),
    Box::new(for_update_window::Rule),
    Box::new(update_delete_order_by::Rule),
    Box::new(returning_aggregate::Rule),
    Box::new(generated_column_not_stored::Rule),
    Box::new(mysql_inline_index::Rule),
    Box::new(with_oids::Rule),
    Box::new(mysql_character_set::Rule),
    Box::new(natural_join::Rule),
    Box::new(with_ties_no_order::Rule),
    Box::new(date_trunc_invalid_unit::Rule),
    Box::new(non_pg_date_diff_fns::Rule),
    Box::new(mysql_if_function::Rule),
    Box::new(mysql_string_functions::Rule),
    Box::new(mysql_enum_inline::Rule),
    Box::new(mysql_on_update_timestamp::Rule),
    Box::new(mysql_zerofill::Rule),
    Box::new(mysql_query_modifiers::Rule),
    Box::new(mysql_xor_div::Rule),
    Box::new(cross_dialect_scalar_fns::Rule),
    Box::new(tsql_types::Rule),
    Box::new(tsql_identity_fns::Rule),
    Box::new(last_value_default_frame::Rule),
    Box::new(large_object_file_access::Rule),
    Box::new(server_file_read_fns::Rule),
    Box::new(weak_gen_salt::Rule),
    Box::new(sqlite_pragma::Rule),
    Box::new(sqlite_autoincrement::Rule),
    Box::new(sqlite_glob::Rule),
    Box::new(sqlite_functions::Rule),
    Box::new(cross_dialect_string_fns2::Rule),
    Box::new(mysql_datepart_fns::Rule),
    Box::new(default_relative_datetime_string::Rule),
    Box::new(mysql_file_io::Rule),
    Box::new(oracle_date_misc_fns::Rule),
    Box::new(mysql_date_arith_fns::Rule),
    Box::new(srf_in_where::Rule),
    Box::new(aggregate_distinct_star::Rule),
    Box::new(in_subquery_multi_column::Rule),
    Box::new(tablesample_out_of_range::Rule),
    Box::new(on_conflict_do_update_no_target::Rule),
    Box::new(row_comparison_arity::Rule),
    Box::new(srf_in_group_order::Rule),
    Box::new(duplicate_cte_name::Rule),
    Box::new(aggregate_in_check::Rule),
    Box::new(aggregate_in_index::Rule),
    Box::new(update_set_arity::Rule),
    Box::new(truncate_with_where::Rule),
    Box::new(order_by_after_limit::Rule),
    Box::new(limit_and_fetch::Rule),
    Box::new(where_after_group_by::Rule),
    Box::new(cross_join_with_on::Rule),
    Box::new(window_fn_without_over::Rule),
    Box::new(distinct_on_no_parens::Rule),
    Box::new(tail_clause_before_setop::Rule),
    Box::new(having_before_group_by::Rule),
    Box::new(update_where_before_set::Rule),
    Box::new(mysql_insert_ignore::Rule),
    Box::new(mysql_insert_set::Rule),
    Box::new(delete_alias_before_from::Rule),
    Box::new(mysql_lock_in_share_mode::Rule),
    Box::new(mysql_show_statement::Rule),
    Box::new(mysql_describe::Rule),
    Box::new(mysql_alter_change_modify::Rule),
    Box::new(mysql_functions::Rule),
    Box::new(mysql_on_duplicate_key::Rule),
    Box::new(where_bare_integer::Rule),
    Box::new(between_null_bound::Rule),
    Box::new(ranking_fn_with_frame::Rule),
    Box::new(union_branch_distinct::Rule),
    Box::new(count_distinct_constant::Rule),
    Box::new(modulo_by_one::Rule),
    Box::new(zero_date_literal::Rule),
    Box::new(left_right_zero::Rule),
    Box::new(substring_zero_length::Rule),
    Box::new(multiply_by_zero::Rule),
    Box::new(coalesce_count_redundant::Rule),
    Box::new(case_constant_when::Rule),
    Box::new(greatest_least_null_arg::Rule),
    Box::new(power_base_one::Rule),
    Box::new(not_not_double_negation::Rule),
    Box::new(coalesce_constant_first::Rule),
    Box::new(modulo_self::Rule),
    Box::new(sqrt_negative_literal::Rule),
    Box::new(min_max_distinct::Rule),
    Box::new(ln_log_nonpositive_literal::Rule),
    Box::new(log_base_one::Rule),
    Box::new(acos_asin_domain::Rule),
    Box::new(nested_aggregate::Rule),
    Box::new(count_of_coalesce::Rule),
    Box::new(degrees_radians_roundtrip::Rule),
    Box::new(chr_zero::Rule),
    Box::new(lpad_rpad_zero::Rule),
    Box::new(setseed_out_of_range::Rule),
    Box::new(nullif_distinct_literals::Rule),
    Box::new(coalesce_is_null_always_false::Rule),
    Box::new(ntile_nonpositive::Rule),
    Box::new(nth_value_nonpositive::Rule),
    Box::new(width_bucket_nonpositive_count::Rule),
    Box::new(lag_lead_zero_offset::Rule),
    Box::new(jsonb_typeof_invalid_literal::Rule),
    Box::new(coalesce_is_not_null_always_true::Rule),
    Box::new(bitand_zero::Rule),
    Box::new(bitwise_self::Rule),
    Box::new(starts_with_empty_string::Rule),
    Box::new(translate_empty_from::Rule),
    Box::new(to_char_empty_format::Rule),
    Box::new(repeat_one::Rule),
    Box::new(power_zero_negative_exponent::Rule),
    Box::new(factorial_negative::Rule),
    Box::new(random_compare_out_of_range::Rule),
    Box::new(ascii_empty_string::Rule),
    Box::new(exp_ln_roundtrip::Rule),
    Box::new(bitor_zero::Rule),
    Box::new(bitshift_zero::Rule),
    Box::new(chr_above_max::Rule),
    Box::new(ln_log_one::Rule),
    Box::new(acosh_atanh_domain::Rule),
    Box::new(exists_aggregate::Rule),
    Box::new(width_bucket_equal_bounds::Rule),
    Box::new(nonneg_func_negative_compare2::Rule),
    Box::new(not_boolean_literal::Rule),
    Box::new(modulo_by_negative_one::Rule),
    Box::new(percentile_fraction_out_of_range::Rule),
    Box::new(reindex_not_concurrent::Rule),
    Box::new(alter_add_key_lock::Rule),
    Box::new(vacuum_full_locks::Rule),
    Box::new(internal_type_alias::Rule),
    Box::new(money_type::Rule),
    Box::new(unlogged_table::Rule),
    Box::new(create_rule_legacy::Rule),
    Box::new(disable_trigger_all::Rule),
    Box::new(disable_row_level_security::Rule),
    Box::new(role_superuser::Rule),
    Box::new(exists_distinct::Rule),
    Box::new(regex_literal_could_be_like::Rule),
    Box::new(col_eq_col_offset::Rule),
    Box::new(null_not_null_conflict::Rule),
    Box::new(default_subquery::Rule),
    Box::new(fk_column_count_mismatch::Rule),
    Box::new(multiple_primary_keys::Rule),
    Box::new(eq_any_array_to_in::Rule),
    Box::new(default_null_redundant::Rule),
    Box::new(nonneg_func_negative_compare::Rule),
    Box::new(redundant_range_bound::Rule),
    Box::new(neq_all_array_to_not_in::Rule),
    Box::new(modulo_out_of_range::Rule),
    Box::new(range_is_equality::Rule),
    Box::new(neq_chain_to_not_in::Rule),
    Box::new(not_paren_comparison::Rule),
    Box::new(on_conflict_self_assignment::Rule),
    Box::new(round_trunc_zero_scale::Rule),
    Box::new(coalesce_nested::Rule),
    Box::new(redundant_column_alias::Rule),
    Box::new(replace_same_from_to::Rule),
    Box::new(setop_identical_branches::Rule),
    Box::new(in_list_duplicates::Rule),
    Box::new(eq_contradiction::Rule),
    Box::new(impossible_range::Rule),
    Box::new(like_all_wildcard::Rule),
    Box::new(exists_with_limit::Rule),
    Box::new(left_join_defeated_by_where::Rule),
    Box::new(is_null_or_is_not_null::Rule),
    Box::new(case_fold_impossible_compare::Rule),
    Box::new(any_all_single_element_array::Rule),
    Box::new(case_boolean_redundant::Rule),
    Box::new(or_chain_to_in::Rule),
    Box::new(in_list_single_value::Rule),
    Box::new(update_self_assignment::Rule),
    Box::new(join_on_constant_tautology::Rule),
    Box::new(update_delete_limit::Rule),
    Box::new(timestamp_precision_over::Rule),
    Box::new(revoke_missing_from::Rule),
    Box::new(psql_backslash::Rule),
    Box::new(string_agg_no_order::Rule),
    Box::new(serial_vs_identity::Rule),
    Box::new(mysql_table_comment::Rule),
    Box::new(mysql_auto_increment::Rule),
    Box::new(mysql_engine::Rule),
    Box::new(mysql_types::Rule),
    Box::new(mssql_bracket_quote::Rule),
    Box::new(mssql_top::Rule),
    Box::new(non_pg_null_fns::Rule),
    Box::new(non_pg_date_fns::Rule),
    Box::new(mssql_go::Rule),
    Box::new(mssql_begin_tran::Rule),
    Box::new(oracle_dual::Rule),
    Box::new(oracle_rownum::Rule),
    Box::new(oracle_connect_by::Rule),
    Box::new(oracle_outer_join::Rule),
    Box::new(create_table_no_schema::Rule),
    Box::new(revoke_without_grant::Rule),
    Box::new(substring_from_no_for::Rule),
    Box::new(drop_index_concurrently_in_tx::Rule),
    Box::new(pg_terminate_backend::Rule),
    Box::new(on_update_cascade_pk::Rule),
    Box::new(setseed_no_determinism_guard::Rule),
    Box::new(tablespace_specified::Rule),
    Box::new(bytea_literal_no_escape::Rule),
    Box::new(group_by_alias::Rule),
    Box::new(like_include_indexes_partition::Rule),
    Box::new(truncate_in_plpgsql_exception::Rule),
    Box::new(new_assign_pk_in_before_insert::Rule),
    Box::new(array_elem_vs_col::Rule),
    Box::new(bool_agg_nullable::Rule),
    Box::new(percentile_non_numeric_order::Rule),
    Box::new(order_by_using_noncomparable::Rule),
    Box::new(rename_column_breaks_view::Rule),
    Box::new(brin_small_table::Rule),
    Box::new(alter_trigger_lock::Rule),
    Box::new(unknown_function::Rule),
    Box::new(insert_unknown_column::Rule),
    Box::new(returning_unknown_column::Rule),
    Box::new(dml_where_unknown_column::Rule),
    Box::new(function_arg_validation::Rule),
    Box::new(empty_expression_paren::Rule),
  ]
}
