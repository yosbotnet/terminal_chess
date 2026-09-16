use terminal_chess::theme::{Theme, ThemeKind};

#[test]
fn themes_can_be_looked_up_by_name() {
    assert_eq!(ThemeKind::from_name("claude"), Some(ThemeKind::Claude));
    assert_eq!(ThemeKind::from_name("CODEX"), Some(ThemeKind::Codex));
    assert_eq!(ThemeKind::from_name("nope"), None);
}

#[test]
fn themes_cycle() {
    assert_eq!(ThemeKind::Claude.next(), ThemeKind::Codex);
    assert_eq!(ThemeKind::Codex.next(), ThemeKind::Claude);
}

#[test]
fn each_theme_has_distinct_prompt_glyph() {
    let claude = Theme::get(ThemeKind::Claude);
    let codex = Theme::get(ThemeKind::Codex);
    assert_eq!(claude.prompt, ">");
    assert_eq!(codex.prompt, "\u{203a}");
    assert_ne!(claude.name, codex.name);
}
