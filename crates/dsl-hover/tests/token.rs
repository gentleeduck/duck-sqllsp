use dsl_hover::token::token_at;
use text_size::TextSize;

#[test]
fn cursor_inside_identifier() {
    let src = "SELECT id FROM users";
    let off = TextSize::from("SELECT id".len() as u32 - 1);
    assert_eq!(token_at(src, off).as_deref(), Some("id"));
}

#[test]
fn cursor_at_end_of_identifier() {
    let src = "SELECT id";
    let off = TextSize::from(src.len() as u32);
    assert_eq!(token_at(src, off).as_deref(), Some("id"));
}

#[test]
fn includes_dot() {
    let src = "SELECT users.id";
    let off = TextSize::from("SELECT user".len() as u32);
    assert_eq!(token_at(src, off).as_deref(), Some("users.id"));
}

#[test]
fn returns_none_on_whitespace() {
    let src = "SELECT  id";
    let off = TextSize::from(7);
    assert!(token_at(src, off).is_none());
}
