//! Prints both themes (and the camouflage view) as plain text so the layout
//! can be checked without a real terminal. Colors are not shown.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use terminal_chess::app::App;
use terminal_chess::lichess::Event;
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
}
