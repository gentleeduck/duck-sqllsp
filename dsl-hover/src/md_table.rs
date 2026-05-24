//! ASCII tables for hover output, with single-row separators so the
//! markdown renderer can colour-style the grid uniformly.
//!
//! Hover floats often don't run a real markdown renderer -- raw `|`
//! pipes leak through. Wrapping in a fenced code block + ASCII rules
//! keeps the layout intact whether rendered as markdown or shown raw.

const MAX_CELL_WIDTH: usize = 60;

/// Render headers + rows as a box-drawing table, wrapped in a fenced
/// code block so monospace alignment survives any markdown reflow.
pub fn render(headers: &[&str], rows: &[Vec<String>]) -> String {
    let cols = headers.len();

    // Pre-truncate every cell so width math is honest.
    let header_cells: Vec<String> = headers.iter().map(|h| trunc(h)).collect();
    let row_cells: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let mut row: Vec<String> = r.iter().take(cols).map(|c| trunc(c)).collect();
            while row.len() < cols { row.push(String::new()); }
            row
        })
        .collect();

    let mut widths: Vec<usize> = header_cells.iter().map(|h| h.chars().count()).collect();
    for row in &row_cells {
        for (i, cell) in row.iter().enumerate() {
            let w = cell.chars().count();
            if w > widths[i] { widths[i] = w; }
        }
    }

    let top    = border(&widths, '+', '+', '+');
    let middle = border(&widths, '+', '+', '+');
    let bottom = border(&widths, '+', '+', '+');

    let mut out = String::new();
    out.push_str("```\n");
    out.push_str(&top); out.push('\n');
    out.push_str(&data_row(&header_cells, &widths)); out.push('\n');
    out.push_str(&middle); out.push('\n');
    for (i, row) in row_cells.iter().enumerate() {
        out.push_str(&data_row(row, &widths));
        out.push('\n');
        // Per-row separator -- helps the float read like a table even
        // when the markdown renderer collapses adjacent lines.
        if i + 1 < row_cells.len() {
            out.push_str(&middle);
            out.push('\n');
        }
    }
    out.push_str(&bottom); out.push('\n');
    out.push_str("```\n");
    out
}

fn border(widths: &[usize], left: char, mid: char, right: char) -> String {
    let mut s = String::new();
    s.push(left);
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..(*w + 2) { s.push('-'); }
        s.push(if i + 1 == widths.len() { right } else { mid });
    }
    s
}

fn data_row(cells: &[String], widths: &[usize]) -> String {
    let mut s = String::new();
    s.push('|');
    for (i, cell) in cells.iter().enumerate() {
        s.push(' ');
        s.push_str(&pad(cell, widths[i]));
        s.push(' ');
        s.push('|');
    }
    s
}

fn pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width { return s.to_string(); }
    let mut padded = String::with_capacity(s.len() + width - len);
    padded.push_str(s);
    for _ in 0..(width - len) { padded.push(' '); }
    padded
}

fn trunc(s: &str) -> String {
    let len = s.chars().count();
    if len <= MAX_CELL_WIDTH { return s.to_string(); }
    let mut taken: String = s.chars().take(MAX_CELL_WIDTH - 1).collect();
    taken.push('…');
    taken
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aligns_columns_inside_code_fence() {
        let out = render(
            &["a", "b"],
            &[vec!["x".into(), "yyy".into()], vec!["xxxx".into(), "z".into()]],
        );
        assert!(out.starts_with("```\n"));
        assert!(out.contains('|'));
        assert!(out.contains('+'));
    }
    #[test]
    fn truncates_huge_cells() {
        let big = "x".repeat(200);
        let out = render(&["c"], &[vec![big]]);
        for line in out.lines() {
            assert!(line.chars().count() < 80);
        }
    }
}
