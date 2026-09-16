use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use terminal_chess::app::App;
use terminal_chess::theme::ThemeKind;
use terminal_chess::ui;

fn screen(app: &mut App, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn claude_theme_shows_banner_prompt_and_board() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    let s = screen(&mut app, 100, 40);
    assert!(s.contains("Claude Code"), "{s}");
    assert!(s.contains("? for shortcuts"), "{s}");
    assert!(s.contains('\u{265c}'), "{s}");
    assert!(s.contains("a  b  c  d  e  f  g  h"), "{s}");
}

#[test]
fn codex_theme_shows_its_banner() {
    let mut app = App::new(ThemeKind::Codex, false, "me".into());
    let s = screen(&mut app, 100, 40);
    assert!(s.contains("Codex"), "{s}");
    assert!(!s.contains("Claude Code"), "{s}");
}

#[test]
fn camouflage_hides_glyphs_behind_a_fake_read() {
    let mut app = App::new(ThemeKind::Claude, true, "me".into());
    let s = screen(&mut app, 100, 40);
    assert!(!s.contains('\u{265c}'), "{s}");
    assert!(s.contains("Read("), "{s}");
    assert!(s.contains("r n b q k b n r"), "{s}");
}

#[test]
fn typed_input_appears_in_prompt() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    for c in "e2 e4".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    let s = screen(&mut app, 100, 40);
    assert!(s.contains("> e2 e4"), "{s}");
}

#[test]
fn small_terminal_does_not_panic() {
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    let _ = screen(&mut app, 30, 8);
    let _ = screen(&mut app, 10, 2);
}

#[test]
fn review_draws_the_past_position_and_eval() {
    use terminal_chess::lichess::{Eval, Event, PlyEval};
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::GameFull {
        game_id: "g1".into(), white: "me".into(), black: "x".into(),
        moves: String::new(), status: "started".into(), wtime: 0, btime: 0,
    });
    app.handle_event(Event::GameState {
        game_id: "g1".into(), moves: "f2f3 e7e5 g2g4 d8h4".into(), status: "mate".into(),
        winner: None, wtime: 0, btime: 0, draw_offer: None,
    });
    let mut evals = vec![PlyEval::default(); 5];
    evals[3].eval = Some(Eval::Mate(-1));
    app.handle_event(Event::Analysis { game_id: "g1".into(), evals, from_lichess: false });
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    let s = screen(&mut app, 100, 44);
    // queen still on d8 at ply 3
    let rank8 = s.lines().find(|l| l.trim_start().starts_with("8 ")).unwrap().to_string();
    assert!(rank8.contains('\u{265b}'), "{rank8}");
    assert!(s.contains("#-1"), "{s}");
    assert!(s.contains("3/4"), "{s}");
}
