//! Registered lint rules.
//!
//! New rules: add the module, then push an instance into [`all`].

pub mod alter_table_no_owner;
pub mod alter_type_add_value_in_tx;
pub mod advisory_lock_no_unlock;
pub mod ambiguous_column;
pub mod array_subscript_zero;
pub mod backslash_in_string;
pub mod bare_return_typed;
pub mod begin_no_lock_mode;
pub mod between_reversed;
pub mod bool_compare_equals;
pub mod boolean_in_text_column;
pub mod case_no_else;
pub mod case_single_when;
pub mod cast_text_in_distinct;
pub mod cast_text_to_int_in_where;
pub mod character_varying_no_limit;
pub mod char_length_vs_length;
pub mod char_n_type;
pub mod column_default_volatile;
pub mod copy_no_format;
pub mod count_one_vs_star;
pub mod count_star_returns_one;
pub mod cte_missing_recursive;
pub mod ddl_in_immutable;
pub mod deep_case_nesting;
pub mod distinct_after_group_by;
pub mod distinct_on_no_order;
pub mod delete_no_where_in_fn;
pub mod deprecated_function;
pub mod distinct_with_aggregate;
pub mod empty_comment;
pub mod exit_outside_loop;
pub mod explain_analyze_in_dml;
pub mod generate_series_no_alias;
pub mod grant_to_public;
pub mod grant_with_grant_option;
pub mod group_by_all;
pub mod group_by_position;
pub mod group_by_required;
pub mod immutable_calls_volatile;
pub mod implicit_cross_join;
pub mod insert_col_value_count;
pub mod is_distinct_null;
pub mod insert_no_columns;
pub mod insert_no_on_conflict;
pub mod insert_type_literal;
pub mod join_no_on;
pub mod jsonb_no_cast;
pub mod jsonb_set_no_create;
pub mod like_leading_wildcard;
pub mod like_with_no_collation;
pub mod like_without_wildcard;
pub mod listen_unbounded;
pub mod limit_without_order;
pub mod lock_table_no_tx;
pub mod long_in_list;
pub mod missing_lateral;
pub mod missing_primary_key;
pub mod missing_trigger_return;
pub mod multi_raise_exception;
pub mod multi_where;
pub mod multiple_truncate_in_tx;
pub mod mutating_without_where;
pub mod negative_limit_offset;
pub mod not_in_subquery;
pub mod null_comparison;
pub mod null_default_not_null;
pub mod null_in_values;
pub mod nullif_same_args;
pub mod numeric_no_precision;
pub mod order_by_position;
pub mod order_by_random;
pub mod perform_for_pure_select;
pub mod prefer_alias;
pub mod raise_arg_count;
pub mod raise_message_no_args;
pub mod raise_using_errcode;
pub mod redundant_index_on_pk;
pub mod redundant_parens;
pub mod redundant_unique_index;
pub mod reserved_word_identifier;
pub mod return_type_literal;
pub mod returning_no_assign;
pub mod returning_with_truncate;
pub mod row_constructor_single;
pub mod row_count_after_dml;
pub mod savepoint_no_release;
pub mod select_for_update_in_recursive_cte;
pub mod set_role_no_reset;
pub mod select_for_update_no_where;
pub mod select_into_outside_plpgsql;
pub mod select_into_strict_no_exception;
pub mod select_no_from_no_agg;
pub mod select_into_shape;
pub mod single_stmt_transaction;
pub mod select_star_insert;
pub mod sql_lang_uses_new_old;
pub mod text_int_arithmetic;
pub mod time_with_timezone;
pub mod timestamp_int_arithmetic;
pub mod timestamp_without_tz;
pub mod transaction_isolation_no_set;
pub mod trigger_stmt_uses_new;
pub mod trigger_when_uses_new_in_delete;
pub mod trigger_when_uses_old_in_insert;
pub mod trailing_comma_values;
pub mod truncate_no_cascade;
pub mod union_column_count;
pub mod union_vs_all;
pub mod unique_on_nullable;
pub mod unknown_column;
pub mod unreachable_after_return;
pub mod unresolved_table;
pub mod update_from_no_pk_filter;
pub mod update_set_no_change;
pub mod update_set_unknown_col;
pub mod vacuum_in_transaction;

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
    ]
}
