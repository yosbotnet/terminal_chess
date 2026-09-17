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

// ----- review mode -----

use terminal_chess::app::judge;
use terminal_chess::lichess::{Eval, PlyEval};

fn finished_app() -> App {
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
    app
}

#[test]
fn game_over_enters_review_and_requests_analysis() {
    let mut app = finished_app();
    assert_eq!(app.review_ply(), Some(4));
    let actions = app.take_actions();
    match &actions[..] {
        [Action::FetchAnalysis { game_id, fens }] => {
            assert_eq!(game_id, "g1");
            assert_eq!(fens.len(), 5);
            assert!(fens[0].starts_with("rnbqkbnr/pppppppp"));
        }
        other => panic!("unexpected actions {other:?}"),
    }
}

#[test]
fn review_keys_step_through_positions() {
    let mut app = finished_app();
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.review_ply(), Some(3));
    assert_eq!(app.board_game().move_count(), 3);
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.review_ply(), Some(2));
    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.review_ply(), Some(0));
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.review_ply(), Some(0));
    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.review_ply(), Some(4));
    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.review_ply(), Some(4));
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.review_ply(), None);
    assert_eq!(app.board_game().move_count(), 4);
}

#[test]
fn review_view_highlights_the_move_at_that_ply() {
    let mut app = finished_app();
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.view().last_move.map(|(f, t)| format!("{f}{t}")), Some("g2g4".into()));
    assert!(app.view().cursor.is_none());
}

#[test]
fn analysis_event_shows_warnings_like_a_linter() {
    let mut app = finished_app();
    app.take_actions();
    let mut evals = vec![PlyEval::default(); 5];
    evals[1].eval = Some(Eval::Cp(-20));
    evals[3] = PlyEval { eval: Some(Eval::Mate(-1)), best: Some("g1h3".into()), judgment: Some("Blunder".into()) };
    evals[4].eval = Some(Eval::Mate(0));
    app.handle_event(Event::Analysis { game_id: "g1".into(), evals, from_lichess: true });
    let text = app
        .transcript()
        .iter()
        .filter_map(|e| match e {
            Entry::Text(t) => Some(t.clone()),
            _ => None,
        })
        .next_back()
        .unwrap();
    assert!(text.contains("warning"), "{text}");
    assert!(text.contains("g4"), "{text}");
    assert!(text.to_lowercase().contains("blunder"), "{text}");
    assert!(text.contains("Nh3"), "best move should be shown as SAN: {text}");
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.current_eval(), Some(Eval::Mate(-1)));
}

#[test]
fn analysis_without_judgments_derives_them_from_swings() {
    let mut app = finished_app();
    app.take_actions();
    let mut evals = vec![PlyEval::default(); 5];
    evals[0].eval = Some(Eval::Cp(20));
    evals[1].eval = Some(Eval::Cp(-40));
    evals[2].eval = Some(Eval::Cp(-50));
    evals[3].eval = Some(Eval::Cp(-900));
    evals[4].eval = Some(Eval::Mate(0));
    app.handle_event(Event::Analysis { game_id: "g1".into(), evals, from_lichess: false });
    let text = app
        .transcript()
        .iter()
        .filter_map(|e| match e {
            Entry::Text(t) => Some(t.clone()),
            _ => None,
        })
        .next_back()
        .unwrap();
    assert!(text.to_lowercase().contains("blunder"), "{text}");
    assert!(text.contains("g4"), "{text}");
}

#[test]
fn judge_uses_the_movers_perspective() {
    assert_eq!(judge(Eval::Cp(50), Eval::Cp(-250), Side::White), Some("Blunder"));
    assert_eq!(judge(Eval::Cp(-30), Eval::Cp(-150), Side::White), Some("Mistake"));
    assert_eq!(judge(Eval::Cp(0), Eval::Cp(-60), Side::White), Some("Inaccuracy"));
    assert_eq!(judge(Eval::Cp(0), Eval::Cp(-20), Side::White), None);
    assert_eq!(judge(Eval::Cp(0), Eval::Cp(80), Side::White), None);
    assert_eq!(judge(Eval::Cp(-100), Eval::Cp(200), Side::Black), Some("Blunder"));
    assert_eq!(judge(Eval::Cp(100), Eval::Mate(2), Side::Black), Some("Blunder"));
    assert_eq!(judge(Eval::Mate(-3), Eval::Cp(-900), Side::Black), None);
}

#[test]
fn analyze_command_opens_the_lichess_page() {
    let mut app = finished_app();
    app.take_actions();
    type_str(&mut app, "/analyze");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::OpenBrowser("https://lichess.org/g1".into())]);
}

#[test]
fn review_command_needs_a_game() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    type_str(&mut app, "/review");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.take_actions().is_empty());
    assert!(matches!(app.transcript().last(), Some(Entry::Error(_))));
}

#[test]
fn review_status_line_shows_progress() {
    let mut app = finished_app();
    app.handle_key(key(KeyCode::Left));
    assert!(app.status_line().contains("review"), "{}", app.status_line());
    assert!(app.status_line().contains("3/4"), "{}", app.status_line());
}

// ----- annotations, clock, chat, pgn, notifications, puzzles, panic -----

use terminal_chess::lichess::Puzzle;
use terminal_chess::render::Mark;

#[test]
fn letters_always_go_to_the_prompt() {
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "h4");
    assert_eq!(app.cursor().to_string(), "e2");
    assert_eq!(app.input(), "h4");
}

#[test]
fn m_cycles_a_mark_on_the_cursor_square_and_x_clears() {
    let mut app = app_in_game(Side::White);
    app.handle_key(key(KeyCode::Char('m')));
    assert_eq!(app.view().marks, vec![("e2".parse().unwrap(), Mark::Green)]);
    app.handle_key(key(KeyCode::Char('m')));
    assert_eq!(app.view().marks[0].1, Mark::Red);
    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Char('m')));
    assert_eq!(app.view().marks[0].1, Mark::Yellow);
    app.handle_key(key(KeyCode::Char('m')));
    assert!(app.view().marks.is_empty());
    app.handle_key(key(KeyCode::Char('m')));
    app.handle_key(key(KeyCode::Char('x')));
    assert!(app.view().marks.is_empty());
    assert_eq!(app.input(), "");
}

#[test]
fn v_twice_draws_an_arrow() {
    let mut app = app_in_game(Side::White);
    app.handle_key(key(KeyCode::Char('v')));
    app.handle_key(key(KeyCode::Up));
    app.handle_key(key(KeyCode::Up));
    app.handle_key(key(KeyCode::Char('v')));
    assert_eq!(app.view().arrows, vec![("e2".parse().unwrap(), "e4".parse().unwrap())]);
    // same arrow again removes it
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Char('v')));
    app.handle_key(key(KeyCode::Up));
    app.handle_key(key(KeyCode::Up));
    app.handle_key(key(KeyCode::Char('v')));
    assert!(app.view().arrows.is_empty());
}

#[test]
fn annotation_keys_type_normally_once_the_prompt_has_text() {
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "/say mv x");
    assert_eq!(app.input(), "/say mv x");
    assert!(app.view().marks.is_empty());
}

#[test]
fn clocks_count_down_for_the_side_to_move() {
    let mut app = app_in_game(Side::White);
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "e2e4 e7e5".into(),
        status: "started".into(),
        winner: None,
        wtime: 600_000,
        btime: 500_000,
        draw_offer: None,
    });
    let (w, b) = app.clocks().expect("clock shown");
    assert!(w <= 600_000 && w > 598_000, "{w}");
    assert_eq!(b, 500_000);
    assert_eq!(App::format_clock(600_000), "10:00");
    assert_eq!(App::format_clock(59_500), "0:59");
    assert_eq!(App::format_clock(3_725_000), "1:02:05");
}

#[test]
fn clocks_hidden_for_correspondence_or_no_clock() {
    let mut app = app_in_game(Side::White);
    assert!(app.clocks().is_none());
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "e2e4".into(),
        status: "started".into(),
        winner: None,
        wtime: 172_800_000,
        btime: 172_800_000,
        draw_offer: None,
    });
    assert!(app.clocks().is_none());
}

#[test]
fn say_sends_chat_to_the_current_game() {
    let mut app = app_in_game(Side::White);
    type_str(&mut app, "/say gg");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.take_actions(), vec![Action::SendChat { game_id: "g1".into(), text: "gg".into() }]);
}

#[test]
fn pgn_command_prints_a_fake_write_tool_call() {
    let mut app = finished_app();
    app.take_actions();
    type_str(&mut app, "/pgn");
    app.handle_key(key(KeyCode::Enter));
    match app.transcript().last() {
        Some(Entry::Block { title, lines }) => {
            assert!(title.starts_with("Write("), "{title}");
            assert!(lines.iter().any(|l| l.contains("1. f3 e5 2. g4 Qh4# 0-1")), "{lines:?}");
            assert!(lines.iter().any(|l| l == "[White \"me\"]"), "{lines:?}");
            assert!(lines.iter().any(|l| l == "[Black \"Someone\"]"), "{lines:?}");
        }
        other => panic!("expected block, got {other:?}"),
    }
    assert!(app.take_actions().is_empty());
}

#[test]
fn pgn_save_emits_a_write_action() {
    let mut app = finished_app();
    app.take_actions();
    type_str(&mut app, "/pgn save C:\\tmp\\g.pgn");
    app.handle_key(key(KeyCode::Enter));
    match app.take_actions().as_slice() {
        [Action::WriteFile { path, contents }] => {
            assert_eq!(path, "C:\\tmp\\g.pgn");
            assert!(contents.contains("Qh4#"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn opponent_move_while_unfocused_notifies() {
    let mut app = app_in_game(Side::White);
    app.set_focused(false);
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "e2e4 e7e5".into(),
        status: "started".into(),
        winner: None,
        wtime: 0,
        btime: 0,
        draw_offer: None,
    });
    assert!(app.take_actions().iter().any(|a| matches!(a, Action::Notify { .. })));
    app.set_focused(true);
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "e2e4 e7e5 g1f3 b8c6".into(),
        status: "started".into(),
        winner: None,
        wtime: 0,
        btime: 0,
        draw_offer: None,
    });
    assert!(!app.take_actions().iter().any(|a| matches!(a, Action::Notify { .. })));
}

fn puzzle() -> Puzzle {
    Puzzle {
        id: "P1".into(),
        rating: 1200,
        // Scholar's mate setup: white to play Qxf7#
        pgn: "e4 e5 Bc4 Nc6 Qh5 Nf6".into(),
        solution: vec!["h5f7".into()],
        themes: vec!["mateIn1".into()],
    }
}

fn two_step_puzzle() -> Puzzle {
    Puzzle {
        id: "P2".into(),
        rating: 1300,
        pgn: "e4 e5 Nf3 Nc6 Bc4 Nf6 Ng5 d5 exd5 Nxd5 Nxf7 Kxf7 Qf3+ Ke6 Nc3 Ncb4".into(),
        solution: vec!["c3d5".into(), "b4d5".into(), "d2d4".into()],
        themes: vec![],
    }
}

#[test]
fn puzzle_command_fetches_and_event_sets_up_the_board() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    type_str(&mut app, "/puzzle");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.take_actions(),
        vec![Action::FetchPuzzle { difficulty: "easiest".into(), seen: vec![] }]
    );
    app.handle_event(Event::Puzzle(puzzle()));
    assert!(app.puzzle_active());
    assert_eq!(app.my_side(), Some(Side::White));
    assert_eq!(app.game().move_count(), 6);
    assert!(!app.view().flipped);
}

#[test]
fn next_puzzle_request_lists_the_ones_already_shown() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::Puzzle(puzzle()));
    type_str(&mut app, "Qxf7");
    app.handle_key(key(KeyCode::Enter));
    type_str(&mut app, "/puzzle");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.take_actions(),
        vec![Action::FetchPuzzle { difficulty: "easiest".into(), seen: vec!["P1".into()] }]
    );
}

#[test]
fn puzzle_difficulty_sticks_for_the_session() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.set_puzzle_difficulty("normal");
    type_str(&mut app, "/puzzle");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.take_actions(),
        vec![Action::FetchPuzzle { difficulty: "normal".into(), seen: vec![] }]
    );
    type_str(&mut app, "/puzzle hard");
    app.handle_key(key(KeyCode::Enter));
    type_str(&mut app, "/puzzle");
    app.handle_key(key(KeyCode::Enter));
    let actions = app.take_actions();
    assert_eq!(actions.len(), 2);
    assert!(actions
        .iter()
        .all(|a| matches!(a, Action::FetchPuzzle { difficulty, .. } if difficulty == "harder")));
    // An unknown value from the config is ignored.
    app.set_puzzle_difficulty("bogus");
    type_str(&mut app, "/puzzle");
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(
        app.take_actions().as_slice(),
        [Action::FetchPuzzle { difficulty, .. }] if difficulty == "harder"
    ));
}

#[test]
fn correct_puzzle_move_solves_it_without_network() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::Puzzle(puzzle()));
    type_str(&mut app, "Qxf7");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.take_actions().is_empty());
    assert!(!app.puzzle_active());
    let last = app.transcript().iter().rev().find_map(|e| match e {
        Entry::Text(t) => Some(t.clone()),
        _ => None,
    });
    assert!(last.unwrap().to_lowercase().contains("solved"));
}

#[test]
fn wrong_puzzle_move_is_rejected_and_can_retry() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::Puzzle(puzzle()));
    type_str(&mut app, "Nf3");
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.transcript().last(), Some(Entry::Error(_))));
    assert_eq!(app.game().move_count(), 6);
    assert!(app.puzzle_active());
    type_str(&mut app, "h5 f7");
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.puzzle_active());
}

#[test]
fn puzzle_plays_the_opponent_reply_automatically() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::Puzzle(two_step_puzzle()));
    assert_eq!(app.my_side(), Some(Side::White));
    type_str(&mut app, "c3 d5");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.puzzle_active());
    assert_eq!(app.game().move_count(), 18);
    assert_eq!(app.game().turn(), Side::White);
    type_str(&mut app, "d2 d4");
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.puzzle_active());
    assert_eq!(app.game().move_count(), 19);
}

#[test]
fn cursor_works_in_puzzles() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::Puzzle(puzzle()));
    assert!(app.status_line().contains("puzzle"), "{}", app.status_line());
    for _ in 0..3 {
        app.handle_key(key(KeyCode::Right));
    }
    for _ in 0..3 {
        app.handle_key(key(KeyCode::Up));
    }
    assert_eq!(app.cursor().to_string(), "h5");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.view().selected.is_some());
    for _ in 0..2 {
        app.handle_key(key(KeyCode::Left));
    }
    for _ in 0..2 {
        app.handle_key(key(KeyCode::Up));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.puzzle_active());
}

#[test]
fn panic_mode_swallows_keys_until_one_is_pressed() {
    let mut app = app_in_game(Side::White);
    app.handle_key(key(KeyCode::F(12)));
    assert!(app.panic_active());
    app.handle_key(key(KeyCode::Char('e')));
    assert!(!app.panic_active());
    assert_eq!(app.input(), "");
    type_str(&mut app, "/panic");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.panic_active());
    assert!(!app.panic_script().is_empty());
}
