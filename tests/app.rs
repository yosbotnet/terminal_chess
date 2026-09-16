use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use terminal_chess::app::{Action, App, Entry};
use terminal_chess::game::Side;
use terminal_chess::lichess::{Event, PlayingGame};
use terminal_chess::theme::ThemeKind;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
}

fn app_in_game(side: Side) -> App {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    let (white, black) = match side {
        Side::White => ("me", "Someone"),
        Side::Black => ("Someone", "me"),
    };
    app.handle_event(Event::GameFull {
        game_id: "g1".into(),
        white: white.into(),
        black: black.into(),
        moves: String::new(),
        status: "started".into(),
        wtime: 0,
        btime: 0,
    });
    app.take_actions();
    app
}

#[test]
fn game_full_sets_side_and_flips_board_for_black() {
    let app = app_in_game(Side::Black);
    assert_eq!(app.my_side(), Some(Side::Black));
    assert!(app.view().flipped);
    assert_eq!(app.game_id(), Some("g1"));
    let white = app_in_game(Side::White);
    assert!(!white.view().flipped);
}

#[test]
fn cursor_select_and_move_emits_action() {
    let mut app = app_in_game(Side::White);
    // cursor starts on e2 for white
    assert_eq!(app.cursor().to_string(), "e2");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.view().selected.map(|s| s.to_string()), Some("e2".into()));
    assert_eq!(app.view().targets.len(), 2);
    app.handle_key(key(KeyCode::Up));
    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.cursor().to_string(), "e4");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::Move { game_id: "g1".into(), uci: "e2e4".into() }]);
    assert_eq!(app.game().move_count(), 1);
    assert!(app.view().selected.is_none());
}

#[test]
fn hjkl_moves_cursor_only_when_prompt_is_empty() {
    let mut app = app_in_game(Side::White);
    app.handle_key(key(KeyCode::Char('l')));
    assert_eq!(app.cursor().to_string(), "f2");
    app.handle_key(key(KeyCode::Char('N')));
    app.handle_key(key(KeyCode::Char('h')));
    assert_eq!(app.cursor().to_string(), "f2");
    assert_eq!(app.input(), "Nh");
}

#[test]
fn cursor_respects_flipped_board() {
    let mut app = app_in_game(Side::Black);
    assert_eq!(app.cursor().to_string(), "e7");
    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.cursor().to_string(), "e6");
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.cursor().to_string(), "f6");
}

#[test]
fn cannot_select_opponent_piece_or_move_out_of_turn() {
    let mut app = app_in_game(Side::Black);
    // white to move; black tries to select e7
    app.handle_key(key(KeyCode::Enter));
    assert!(app.view().selected.is_none());
    assert!(app.take_actions().is_empty());
}

#[test]
fn typed_move_emits_action_and_clears_prompt() {
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "e2 e4");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::Move { game_id: "g1".into(), uci: "e2e4".into() }]);
    assert_eq!(app.input(), "");
}

#[test]
fn illegal_typed_move_shows_error_in_transcript() {
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "e2 e5");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.take_actions().is_empty());
    assert!(matches!(app.transcript().last(), Some(Entry::Error(_))));
}

#[test]
fn escape_clears_selection_and_input() {
    let mut app = app_in_game(Side::White);
    app.handle_key(key(KeyCode::Enter));
    type_str(&mut app, "abc");
    app.handle_key(key(KeyCode::Esc));
    assert!(app.view().selected.is_none());
    assert_eq!(app.input(), "");
}

#[test]
fn tab_toggles_camouflage_and_ctrl_t_cycles_theme() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_key(key(KeyCode::Tab));
    assert!(app.camouflage());
    app.handle_key(ctrl('t'));
    assert_eq!(app.theme().kind, ThemeKind::Codex);
}

#[test]
fn ctrl_c_quits() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_key(ctrl('c'));
    assert!(app.should_quit());
}

#[test]
fn game_state_event_updates_position_and_logs_opponent_move() {
    let mut app = app_in_game(Side::White);
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "e2e4 e7e5".into(),
        status: "started".into(),
        winner: None,
        wtime: 0,
        btime: 0,
        draw_offer: None,
    });
    assert_eq!(app.game().move_count(), 2);
    assert_eq!(app.view().last_move.map(|(f, t)| format!("{f}{t}")), Some("e7e5".into()));
    assert!(matches!(app.transcript().last(), Some(Entry::Tool { .. })));
}

#[test]
fn game_over_is_reported() {
    let mut app = app_in_game(Side::White);
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "f2f3 e7e5 g2g4 d8h4".into(),
        status: "mate".into(),
        winner: Some(Side::Black),
        wtime: 0,
        btime: 0,
        draw_offer: None,
    });
    let last = app.transcript().last().cloned();
    match last {
        Some(Entry::Text(t)) => assert!(t.to_lowercase().contains("lost"), "{t}"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn events_for_other_games_are_ignored() {
    let mut app = app_in_game(Side::White);
    app.handle_event(Event::GameState {
        game_id: "other".into(),
        moves: "e2e4".into(),
        status: "started".into(),
        winner: None,
        wtime: 0,
        btime: 0,
        draw_offer: None,
    });
    assert_eq!(app.game().move_count(), 0);
}

#[test]
fn new_ai_command_emits_action() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    type_str(&mut app, "/new ai 3");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::NewAi { level: 3 }]);
}

#[test]
fn games_command_lists_and_game_command_opens() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    type_str(&mut app, "/games");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::ListGames]);
    app.handle_event(Event::Playing(vec![
        PlayingGame { id: "aaa".into(), color: Side::White, my_turn: true, opponent: "Bob".into(), speed: "correspondence".into() },
        PlayingGame { id: "bbb".into(), color: Side::Black, my_turn: false, opponent: "Ann".into(), speed: "blitz".into() },
    ]));
    type_str(&mut app, "/game 2");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::OpenGame("bbb".into())]);
    type_str(&mut app, "/game zzz");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::OpenGame("zzz".into())]);
}

#[test]
fn game_start_event_opens_the_game() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::GameStart { game_id: "new1".into(), color: Side::White });
    assert_eq!(app.take_actions(), vec![Action::OpenGame("new1".into())]);
}

#[test]
fn resign_and_draw_need_a_game() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    type_str(&mut app, "/resign");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.take_actions().is_empty());
    assert!(matches!(app.transcript().last(), Some(Entry::Error(_))));
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "/draw");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::Draw("g1".into())]);
}

#[test]
fn moves_show_as_fake_tool_calls() {
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "e4");
    app.handle_key(key(KeyCode::Enter));
    match app.transcript().last() {
        Some(Entry::Tool { title, .. }) => assert!(title.contains('('), "{title}"),
        other => panic!("expected tool entry, got {other:?}"),
    }
}
