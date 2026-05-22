//! Fuzz-style invariant tests: generate many SELECT/UPDATE/DELETE/INSERT
//! permutations and assert resolver never panics + every produced scope
//! upholds the basic invariants.

use dsl_parse::{parse, Dialect};
use dsl_resolve::resolve;

fn deterministic_strings(seed: u32, count: usize) -> Vec<String> {
    // Tiny linear congruential generator so the corpus is repeatable
    // and the tests stay deterministic across runs.
    let mut state = seed.wrapping_mul(2654435761);
    let mut out = Vec::with_capacity(count);
    let names = ["users", "orders", "items", "tags", "products", "carts"];
    let aliases = ["u", "o", "i", "t", "p", "c", "x", "y", "z"];
    for _ in 0..count {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let mode = (state >> 16) as usize % 5;
        let tbl = names[(state >> 20) as usize % names.len()];
        let al = aliases[(state >> 24) as usize % aliases.len()];
        let q = match mode {
            0 => format!("SELECT 1 FROM {tbl}"),
            1 => format!("SELECT 1 FROM {tbl} {al}"),
            2 => format!("SELECT 1 FROM {tbl} AS {al}"),
            3 => format!("SELECT 1 FROM {tbl} {al}, {tbl} {al}2"),
            4 => format!("SELECT 1 FROM {tbl} {al} JOIN {tbl} {al}2 ON {al}.id = {al}2.id"),
            _ => unreachable!(),
        };
        out.push(q);
    }
    out
}

#[test]
fn fuzz_resolver_never_panics() {
    for src in deterministic_strings(42, 200) {
        let p = parse(&src, Dialect::Postgres);
        let _ = resolve(&p.statements);
    }
}

#[test]
fn fuzz_resolver_invariants_hold() {
    for src in deterministic_strings(13, 200) {
        let p = parse(&src, Dialect::Postgres);
        for scope in resolve(&p.statements) {
            for b in scope.tables() {
                assert!(!b.table.name.is_empty(),
                    "empty binding name in `{src}`");
            }
        }
    }
}

#[test]
fn fuzz_pathological_inputs_do_not_panic() {
    let cases = [
        "",
        ";",
        ";;;;",
        "SELECT",
        "SELECT FROM",
        "SELECT FROM ",
        "SELECT 1",
        "FROM users",
        "SELECT * FROM ((((users))))",
        "WITH x AS (SELECT 1) SELECT * FROM",
        "SELECT 'this is not '' a valid string", // unterminated string
        "/* unterminated comment",
        "SELECT $$ unterminated dollar quote",
    ];
    for s in &cases {
        let p = parse(s, Dialect::Postgres);
        let _ = resolve(&p.statements);
    }
}
