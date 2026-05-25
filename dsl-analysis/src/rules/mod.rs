//! Registered lint rules.
//!
//! New rules: add the module, then push an instance into [`all`].

pub mod add_column_notnull_no_default;
pub mod advisory_lock_literal_key;
pub mod advisory_lock_no_unlock;
pub mod after_trigger_return_row;
pub mod alter_table_no_owner;
pub mod alter_type_add_value_in_tx;
pub mod ambiguous_column;
pub mod any_all_multicol;
pub mod array_subscript_zero;
pub mod backslash_in_string;
pub mod bare_return_typed;
pub mod begin_no_lock_mode;
pub mod between_reversed;
pub mod bool_compare_equals;
pub mod boolean_in_text_column;
pub mod case_branch_types;
pub mod case_no_else;
pub mod case_single_when;
pub mod cast_text_in_distinct;
pub mod cast_text_to_int_in_where;
pub mod char_length_vs_length;
pub mod check_always_true;
pub mod char_n_type;
pub mod coalesce_single_arg;
pub mod character_varying_no_limit;
pub mod column_default_volatile;
pub mod commit_in_function;
pub mod copy_file_path;
pub mod copy_no_format;
pub mod count_one_vs_star;
pub mod count_star_returns_one;
pub mod cte_dml_no_returning;
pub mod cte_missing_recursive;
pub mod current_setting_no_missing_ok;
pub mod ddl_in_immutable;
pub mod deep_case_nesting;
pub mod delete_no_where_in_fn;
pub mod deprecated_function;
pub mod distinct_after_group_by;
pub mod distinct_on_no_order;
pub mod distinct_with_aggregate;
pub mod do_block_bare_select;
pub mod empty_comment;
pub mod empty_in_list;
pub mod exit_outside_loop;
pub mod exists_select_star;
pub mod explain_analyze_in_dml;
pub mod extract_unknown_field;
pub mod generate_series_no_alias;
pub mod gin_on_scalar;
pub mod grant_to_public;
pub mod grant_with_grant_option;
pub mod group_by_all;
pub mod group_by_position;
pub mod group_by_required;
pub mod immutable_calls_volatile;
pub mod implicit_cross_join;
pub mod index_concurrently_in_tx;
pub mod index_expr_volatile;
pub mod insert_col_value_count;
pub mod insert_no_columns;
pub mod insert_no_on_conflict;
pub mod insert_subquery_col_count;
pub mod insert_type_literal;
pub mod is_distinct_null;
pub mod join_no_on;
pub mod jsonb_contains_no_cast;
pub mod jsonb_no_cast;
pub mod jsonb_set_no_create;
pub mod jsonb_set_path_format;
pub mod like_leading_wildcard;
pub mod like_with_no_collation;
pub mod limit_for_update_subq;
pub mod like_without_wildcard;
pub mod limit_without_order;
pub mod listen_unbounded;
pub mod lock_table_no_tx;
pub mod long_in_list;
pub mod merge_missing_when;
pub mod missing_lateral;
pub mod missing_primary_key;
pub mod missing_trigger_return;
pub mod multi_raise_exception;
pub mod multi_where;
pub mod multiple_truncate_in_tx;
pub mod mv_no_data_query;
pub mod mutating_without_where;
pub mod negative_limit_offset;
pub mod notify_unlistened;
pub mod not_in_nullable;
pub mod not_in_subquery;
pub mod null_comparison;
pub mod null_default_not_null;
pub mod null_in_values;
pub mod nulls_first_last_no_order;
pub mod nullif_same_args;
pub mod numeric_no_precision;
pub mod order_by_in_subquery;
pub mod order_by_position;
pub mod order_by_random;
pub mod owner_to_unknown_role;
pub mod plpgsql_assign_type;
pub mod perform_for_pure_select;
pub mod pg_catalog_no_schema;
pub mod pg_sleep_in_tx;
pub mod prefer_alias;
pub mod raise_arg_count;
pub mod raise_no_level;
pub mod recursive_cte_no_union;
pub mod raise_message_no_args;
pub mod raise_using_errcode;
pub mod redundant_index_on_pk;
pub mod redundant_parens;
pub mod redundant_unique_index;
pub mod reindex_system;
pub mod reserved_word_identifier;
pub mod return_type_literal;
pub mod rollback_outside_tx;
pub mod rollup_cube_single;
pub mod returning_no_assign;
pub mod returning_with_truncate;
pub mod row_constructor_single;
pub mod row_count_after_dml;
pub mod savepoint_name_reuse;
pub mod savepoint_no_release;
pub mod select_for_update_in_recursive_cte;
pub mod select_for_update_no_where;
pub mod select_into_existing;
pub mod select_into_outside_plpgsql;
pub mod select_into_shape;
pub mod select_into_strict_no_exception;
pub mod select_no_from_no_agg;
pub mod select_star_insert;
pub mod set_constraints_outside_tx;
pub mod set_local_outside_tx;
pub mod set_role_in_function;
pub mod set_role_no_reset;
pub mod shell_command_in_sql;
pub mod single_stmt_transaction;
pub mod star_with_order_by_position;
pub mod sql_lang_uses_new_old;
pub mod text_int_arithmetic;
pub mod time_with_timezone;
pub mod timestamp_int_arithmetic;
pub mod timestamp_without_tz;
pub mod trailing_comma_values;
pub mod transaction_isolation_no_set;
pub mod trigger_stmt_uses_new;
pub mod trigger_when_uses_new_in_delete;
pub mod trigger_when_uses_old_in_insert;
pub mod truncate_no_cascade;
pub mod union_column_count;
pub mod union_vs_all;
pub mod unique_on_nullable;
pub mod unknown_column;
pub mod unreachable_after_return;
pub mod unresolved_table;
pub mod update_from_no_pk_filter;
pub mod update_set_no_change;
pub mod update_set_alias_mismatch;
pub mod update_set_type_literal;
pub mod update_set_unknown_col;
pub mod count_nullable;
pub mod for_update_on_view;
pub mod is_null_on_not_null;
pub mod insert_into_generated;
pub mod savepoint_outside_tx;
pub mod secdef_no_search_path;
pub mod trigger_wrong_row_alias;
pub mod truncate_in_trigger;
pub mod varchar_length;
pub mod date_literal_format;
pub mod alter_column_type;
pub mod alter_set_tablespace;
pub mod alter_drop_just_created;
pub mod array_eq_with_null;
pub mod array_fn_on_scalar;
pub mod array_mixed_types;
pub mod cast_literal_invalid;
pub mod comment_clears_existing;
pub mod comment_on_unknown;
pub mod default_references_column;
pub mod default_values_no_default_col;
pub mod fk_target_not_unique;
pub mod for_update_aggregate;
pub mod for_update_left_join;
pub mod for_update_of_unknown;
pub mod inline_check_other_col;
pub mod lateral_no_ref;
pub mod generated_uses_volatile;
pub mod on_conflict_do_nothing;
pub mod on_conflict_no_unique;
pub mod truncate_with_fk;
pub mod window_frame_reversed;
pub mod window_no_order;
pub mod drop_cascade_chain;
pub mod drop_column_fk;
pub mod drop_function_no_args;
pub mod drop_schema_no_cascade;
pub mod fk_unknown_column;
pub mod int_range;
pub mod using_clause_columns;
pub mod uuid_literal_format;
pub mod null_into_not_null;
pub mod schema_drift;
pub mod vacuum_in_transaction;
pub mod values_row_width;
pub mod values_subq_no_alias;
pub mod view_select_star;
pub mod where_type_literal;

use crate::LintRule;

pub fn all() -> Vec<Box<dyn LintRule>> {
  vec![
    Box::new(unresolved_table::Rule),
    Box::new(unknown_column::Rule),
    Box::new(ambiguous_column::Rule),
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
  ]
}
