use ratatui::text::Line;
use terminal_chess::game::Game;
use terminal_chess::render::{board, camo, BoardView};
use terminal_chess::theme::{Theme, ThemeKind};

fn text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn squeeze(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn camo_lines_show_pieces_as_letters() {
    let g = Game::new();
    let lines = camo::render(&g, &BoardView::default(), &Theme::get(ThemeKind::Claude));
    assert_eq!(lines.len(), 8);
    assert!(text(&lines[0]).contains("r n b q k b n r"), "{}", text(&lines[0]));
    assert!(text(&lines[2]).contains(". . . . . . . ."));
    assert!(text(&lines[7]).contains("R N B Q K B N R"));
}

#[test]
fn camo_lines_are_numbered_like_a_file_read() {
    let g = Game::new();
    let lines = camo::render(&g, &BoardView::default(), &Theme::get(ThemeKind::Claude));
    assert!(text(&lines[0]).trim_start().starts_with('1'), "{}", text(&lines[0]));
    assert!(text(&lines[7]).trim_start().starts_with('8'));
}

#[test]
fn camo_flipped_puts_white_first() {
    let g = Game::new();
    let view = BoardView { flipped: true, ..Default::default() };
    let lines = camo::render(&g, &view, &Theme::get(ThemeKind::Claude));
    // files run h..a from black's side, so the king sits left of the queen
    assert!(text(&lines[0]).contains("R N B K Q B N R"), "{}", text(&lines[0]));
}

#[test]
fn board_has_rank_labels_and_file_row() {
    let g = Game::new();
    let lines = board::render(&g, &BoardView::default(), &Theme::get(ThemeKind::Claude));
    assert_eq!(lines.len(), 9);
    assert!(text(&lines[0]).trim_start().starts_with('8'));
    assert!(text(&lines[7]).trim_start().starts_with('1'));
    assert_eq!(squeeze(&text(&lines[8])), "abcdefgh");
}

#[test]
fn board_flipped_reverses_labels() {
    let g = Game::new();
    let view = BoardView { flipped: true, ..Default::default() };
    let lines = board::render(&g, &view, &Theme::get(ThemeKind::Claude));
    assert!(text(&lines[0]).trim_start().starts_with('1'));
    assert_eq!(squeeze(&text(&lines[8])), "hgfedcba");
}

#[test]
fn board_uses_unicode_pieces() {
    let g = Game::new();
    let lines = board::render(&g, &BoardView::default(), &Theme::get(ThemeKind::Claude));
    let top = text(&lines[0]);
    assert!(top.contains('\u{265c}'), "black rook expected in {top}");
    assert!(top.contains('\u{265a}'), "black king expected in {top}");
    let bottom = text(&lines[7]);
    assert!(bottom.contains('\u{2654}'), "white king expected in {bottom}");
}

#[test]
fn board_marks_legal_targets_with_dots() {
    let g = Game::new();
    let view = BoardView {
        selected: Some("e2".parse().unwrap()),
        targets: vec!["e3".parse().unwrap(), "e4".parse().unwrap()],
        ..Default::default()
    };
    let lines = board::render(&g, &view, &Theme::get(ThemeKind::Claude));
    // rank 4 is line index 4 from the top when not flipped
    assert!(text(&lines[4]).contains('\u{00b7}'), "{}", text(&lines[4]));
    assert!(!text(&lines[3]).contains('\u{00b7}'));
}
