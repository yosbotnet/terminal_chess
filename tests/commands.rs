use terminal_chess::commands::{parse, Command};

#[test]
fn plain_text_is_a_move() {
    assert_eq!(parse("e2 e4"), Command::Move("e2 e4".into()));
    assert_eq!(parse("  Nf3 "), Command::Move("Nf3".into()));
}

#[test]
fn empty_is_noop() {
    assert_eq!(parse(""), Command::Noop);
    assert_eq!(parse("   "), Command::Noop);
}

#[test]
fn new_ai_game_with_level() {
    assert_eq!(parse("/new ai 3"), Command::NewAi { level: 3 });
    assert_eq!(parse("/new ai"), Command::NewAi { level: 1 });
    assert_eq!(parse("/new ai 9"), Command::Error("level must be 1-8".into()));
}

#[test]
fn seek_with_time_control() {
    assert_eq!(parse("/seek 15+10"), Command::Seek { minutes: 15, increment: 10, days: None });
    assert_eq!(parse("/seek"), Command::Seek { minutes: 0, increment: 0, days: Some(2) });
    assert_eq!(parse("/seek corr 3"), Command::Seek { minutes: 0, increment: 0, days: Some(3) });
    assert_eq!(parse("/seek abc"), Command::Error("usage: /seek 15+10 or /seek corr 2".into()));
}

#[test]
fn simple_commands() {
    assert_eq!(parse("/games"), Command::Games);
    assert_eq!(parse("/resign"), Command::Resign);
    assert_eq!(parse("/draw"), Command::Draw);
    assert_eq!(parse("/theme"), Command::Theme);
    assert_eq!(parse("/flip"), Command::Flip);
    assert_eq!(parse("/help"), Command::Help);
    assert_eq!(parse("/quit"), Command::Quit);
    assert_eq!(parse("/exit"), Command::Quit);
    assert_eq!(parse("/hide"), Command::Camouflage);
}

#[test]
fn switch_game_by_index_or_id() {
    assert_eq!(parse("/game 2"), Command::SwitchGame("2".into()));
    assert_eq!(parse("/game abcd1234"), Command::SwitchGame("abcd1234".into()));
}

#[test]
fn unknown_command_is_error() {
    assert_eq!(parse("/bogus"), Command::Error("unknown command: /bogus".into()));
}

#[test]
fn review_and_analyze_commands() {
    assert_eq!(parse("/review"), Command::Review);
    assert_eq!(parse("/analyze"), Command::Analyze);
    assert_eq!(parse("/analyse"), Command::Analyze);
}
