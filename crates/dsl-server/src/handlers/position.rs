//! Convert between LSP `Position` (utf-16 line/col) and our `TextSize`
//! (utf-8 byte offset). Uses `ropey` so the lookups stay O(log n).

use ropey::Rope;
use text_size::TextSize;
use tower_lsp::lsp_types::Position;

pub fn to_offset(rope: &Rope, pos: Position) -> TextSize {
    let line = pos.line as usize;
    let col = pos.character as usize;
    if line >= rope.len_lines() {
        return TextSize::from(rope.len_bytes() as u32);
    }
    let line_start_char = rope.line_to_char(line);
    let line_str = rope.line(line);
    // pos.character is utf-16 code units per LSP. Walk char-by-char
    // counting utf-16 units; map back to utf-8 bytes.
    let mut utf16_seen = 0usize;
    let mut bytes_seen = 0usize;
    for c in line_str.chars() {
        if utf16_seen >= col { break; }
        utf16_seen += c.len_utf16();
        bytes_seen += c.len_utf8();
    }
    let line_start_byte = rope.char_to_byte(line_start_char);
    TextSize::from((line_start_byte + bytes_seen) as u32)
}
