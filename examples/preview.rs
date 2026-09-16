//! Prints both themes (and the camouflage view) as plain text so the layout
//! can be checked without a real terminal. Colors are not shown.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use terminal_chess::app::App;
use terminal_chess::lichess::{Eval, Event, PlyEval};
use terminal_chess::theme::ThemeKind;
use terminal_chess::ui;

fn dump(app: &mut App, w: u16, h: u16) {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    for y in 0..h {
        let mut line = String::new();
        for x in 0..w {
            line.push_str(buf[(x, y)].symbol());
        }
        println!("{}", line.trim_end());
    }
}

fn main() {
    for (kind, camo) in [(ThemeKind::Claude, false), (ThemeKind::Codex, false), (ThemeKind::Claude, true)] {
        let mut app = App::new(kind, camo, "me".into());
        app.handle_event(Event::GameFull {
            game_id: "g1".into(),
            white: "me".into(),
            black: "Stockfish level 3".into(),
            moves: "e2e4 c7c5 g1f3".into(),
            status: "started".into(),
            wtime: 0,
            btime: 0,
        });
        app.handle_event(Event::GameState {
            game_id: "g1".into(),
            moves: "e2e4 c7c5 g1f3 d7d6".into(),
            status: "started".into(),
            winner: None,
            wtime: 0,
            btime: 0,
            draw_offer: None,
        });
        println!("==== {:?} camouflage={camo} ====", kind);
        dump(&mut app, 90, 30);
        println!();
    }

    // Review mode after a lost game, with cloud evals only.
    let mut app = App::new(ThemeKind::Claude, false, "me".into());
    app.handle_event(Event::GameFull {
        game_id: "g1".into(),
        white: "me".into(),
        black: "Stockfish level 3".into(),
        moves: String::new(),
        status: "started".into(),
        wtime: 0,
        btime: 0,
    });
    app.handle_event(Event::GameState {
        game_id: "g1".into(),
        moves: "f2f3 e7e5 g2g4 d8h4".into(),
        status: "mate".into(),
        winner: None,
        wtime: 0,
        btime: 0,
        draw_offer: None,
    });
    let mut evals = vec![PlyEval::default(); 5];
    evals[0].eval = Some(Eval::Cp(20));
    evals[1].eval = Some(Eval::Cp(-60));
    evals[2].eval = Some(Eval::Cp(-70));
    evals[3] = PlyEval { eval: Some(Eval::Mate(-1)), best: Some("g1h3".into()), judgment: None };
    evals[4].eval = Some(Eval::Mate(0));
    app.handle_event(Event::Analysis { game_id: "g1".into(), evals, from_lichess: false });
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Left,
        crossterm::event::KeyModifiers::NONE,
    ));
    println!("==== Claude review ====");
    dump(&mut app, 90, 40);
}
