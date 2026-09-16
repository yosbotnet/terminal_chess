//! Application state and input handling. No terminal or network code lives here,
//! which keeps the whole thing testable with plain key events.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use shakmaty::{File, Rank, Square};

use crate::commands::{self, Command};
use crate::game::{Game, Outcome, Side};
use crate::lichess::{Eval, Event, PlayingGame, PlyEval};
use crate::render::BoardView;
use crate::theme::{Theme, ThemeKind};

/// Requests the UI makes of the Lichess worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Move { game_id: String, uci: String },
    NewAi { level: u8 },
    Seek { minutes: u32, increment: u32, days: Option<u32> },
    ListGames,
    OpenGame(String),
    Resign(String),
    Draw(String),
    /// Evaluate every position of a game. `fens[i]` is the position after `i` half-moves.
    FetchAnalysis { game_id: String, fens: Vec<String> },
    OpenBrowser(String),
}

/// One entry of the fake agent transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// What the user typed, echoed like an agent CLI does.
    User(String),
    /// A fake tool call: title like `Edit(src/board.rs)` and a dim detail line.
    Tool { title: String, detail: String },
    /// Assistant prose.
    Text(String),
    Error(String),
}

const FILES: &[&str] = &[
    "src/engine/search.rs",
    "src/engine/eval.rs",
    "src/board/bitboard.rs",
    "src/board/movegen.rs",
    "src/uci/protocol.rs",
    "src/book/opening.rs",
    "tests/perft.rs",
    "src/tt/table.rs",
    "src/time/manager.rs",
    "src/main.rs",
];

/// Winning chances in [-1, 1] from White's view, the same curve Lichess uses.
fn winning_chances(e: Eval) -> f64 {
    match e {
        Eval::Cp(cp) => 2.0 / (1.0 + (-0.003_682_08 * cp as f64).exp()) - 1.0,
        Eval::Mate(n) if n > 0 => 1.0,
        Eval::Mate(_) => -1.0,
    }
}

/// Judge the move that took the position from `before` to `after`, played by `mover`.
/// Thresholds follow Lichess: 0.3 blunder, 0.2 mistake, 0.1 inaccuracy.
pub fn judge(before: Eval, after: Eval, mover: Side) -> Option<&'static str> {
    if after == Eval::Mate(0) {
        return None; // the mover delivered mate
    }
    let sign = if mover == Side::White { 1.0 } else { -1.0 };
    let drop = (winning_chances(before) - winning_chances(after)) * sign;
    if drop >= 0.3 {
        Some("Blunder")
    } else if drop >= 0.2 {
        Some("Mistake")
    } else if drop >= 0.1 {
        Some("Inaccuracy")
    } else {
        None
    }
}

pub struct App {
    theme: Theme,
    camouflage: bool,
    username: String,
    game: Game,
    game_id: Option<String>,
    my_side: Option<Side>,
    opponent: String,
    status: String,
    flipped: bool,
    cursor: Square,
    selected: Option<Square>,
    targets: Vec<Square>,
    input: String,
    transcript: Vec<Entry>,
    playing: Vec<PlayingGame>,
    actions: Vec<Action>,
    should_quit: bool,
    /// Ply being looked at in review mode, if any.
    review: Option<usize>,
    /// `evals[i]` is the evaluation after `i` half-moves. Empty until analysis arrives.
    evals: Vec<PlyEval>,
    pub wtime: u64,
    pub btime: u64,
}

impl App {
    pub fn new(theme: ThemeKind, camouflage: bool, username: String) -> App {
        let mut app = App {
            theme: Theme::get(theme),
            camouflage,
            username,
            game: Game::new(),
            game_id: None,
            my_side: None,
            opponent: String::new(),
            status: String::new(),
            flipped: false,
            cursor: Square::E2,
            selected: None,
            targets: Vec::new(),
            input: String::new(),
            transcript: Vec::new(),
            playing: Vec::new(),
            actions: Vec::new(),
            should_quit: false,
            review: None,
            evals: Vec::new(),
            wtime: 0,
            btime: 0,
        };
        app.push(Entry::Text(
            "Ready. Type /games to resume a game, /new ai 3 for Stockfish, /seek for a human, /help for keys.".into(),
        ));
        app
    }

    // ----- accessors -----

    pub fn theme(&self) -> &Theme {
        &self.theme
    }
    pub fn camouflage(&self) -> bool {
        self.camouflage
    }
    /// The live game.
    pub fn game(&self) -> &Game {
        &self.game
    }
    /// The game as it should be drawn: a past position in review, else the live one.
    pub fn board_game(&self) -> Game {
        match self.review {
            Some(ply) => self.game.position_at(ply),
            None => self.game.clone(),
        }
    }
    pub fn game_id(&self) -> Option<&str> {
        self.game_id.as_deref()
    }
    pub fn my_side(&self) -> Option<Side> {
        self.my_side
    }
    pub fn opponent(&self) -> &str {
        &self.opponent
    }
    pub fn username(&self) -> &str {
        &self.username
    }
    pub fn cursor(&self) -> Square {
        self.cursor
    }
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn transcript(&self) -> &[Entry] {
        &self.transcript
    }
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }
    pub fn review_ply(&self) -> Option<usize> {
        self.review
    }
    pub fn current_eval(&self) -> Option<Eval> {
        let ply = self.review.unwrap_or(self.game.move_count());
        self.evals.get(ply).and_then(|e| e.eval)
    }
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    pub fn view(&self) -> BoardView {
        let shown = self.board_game();
        let check_square = if shown.in_check() {
            shown.king_square(shown.turn())
        } else {
            None
        };
        let last_move = shown
            .last_move()
            .and_then(|m| m.from_square().map(|f| (f, m.to_square())));
        if self.review.is_some() {
            return BoardView {
                flipped: self.flipped,
                cursor: None,
                selected: None,
                targets: Vec::new(),
                last_move,
                check_square,
            };
        }
        BoardView {
            flipped: self.flipped,
            cursor: Some(self.cursor),
            selected: self.selected,
            targets: self.targets.clone(),
            last_move,
            check_square,
        }
    }

    /// Short human status for the footer.
    pub fn status_line(&self) -> String {
        if let Some(ply) = self.review {
            let eval = self
                .current_eval()
                .map(|e| format!(" | {e}"))
                .unwrap_or_default();
            return format!("review | move {}/{}{}", ply, self.game.move_count(), eval);
        }
        match (&self.game_id, self.my_side) {
            (Some(_), Some(side)) => {
                let turn = if self.game.turn() == side { "your move" } else { "waiting" };
                let n = self.game.move_count() / 2 + 1;
                if self.is_over() {
                    format!("vs {} | game over", self.opponent)
                } else {
                    format!("vs {} | move {} | {}", self.opponent, n, turn)
                }
            }
            _ => "no game".to_string(),
        }
    }

    fn is_over(&self) -> bool {
        !matches!(self.status.as_str(), "" | "started" | "created")
    }

    fn my_turn(&self) -> bool {
        self.game_id.is_some()
            && self.my_side == Some(self.game.turn())
            && !self.is_over()
            && self.review.is_none()
    }

    fn push(&mut self, e: Entry) {
        self.transcript.push(e);
        if self.transcript.len() > 400 {
            self.transcript.drain(..100);
        }
    }

    fn fake_file(&self) -> &'static str {
        FILES[self.game.move_count() % FILES.len()]
    }

    // ----- key handling -----

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.should_quit = true,
                KeyCode::Char('t') => self.cycle_theme(),
                KeyCode::Char('u') => self.input.clear(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Tab => self.camouflage = !self.camouflage,
            KeyCode::Esc => {
                self.selected = None;
                self.targets.clear();
                self.input.clear();
                self.review = None;
            }
            KeyCode::Enter => {
                if self.input.trim().is_empty() {
                    self.cursor_action();
                } else {
                    let text = std::mem::take(&mut self.input);
                    self.submit(&text);
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Up => self.nav(0, 1),
            KeyCode::Down => self.nav(0, -1),
            KeyCode::Left => self.nav(-1, 0),
            KeyCode::Right => self.nav(1, 0),
            KeyCode::Char(c) => {
                if self.input.is_empty() {
                    match c {
                        'k' => return self.nav(0, 1),
                        'j' => return self.nav(0, -1),
                        'h' => return self.nav(-1, 0),
                        'l' => return self.nav(1, 0),
                        ' ' => return self.cursor_action(),
                        _ => {}
                    }
                }
                self.input.push(c);
            }
            _ => {}
        }
    }

    /// Directional input: steps through the game in review, moves the cursor otherwise.
    fn nav(&mut self, dx: i32, dy: i32) {
        if let Some(ply) = self.review {
            let last = self.game.move_count();
            self.review = Some(match (dx, dy) {
                (-1, _) => ply.saturating_sub(1),
                (1, _) => (ply + 1).min(last),
                (_, 1) => 0,
                _ => last,
            });
            return;
        }
        self.move_cursor(dx, dy);
    }

    /// `dx`/`dy` are screen directions: +x right, +y up.
    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let (dx, dy) = if self.flipped { (-dx, -dy) } else { (dx, dy) };
        let f = (self.cursor.file() as i32 + dx).clamp(0, 7);
        let r = (self.cursor.rank() as i32 + dy).clamp(0, 7);
        self.cursor = Square::from_coords(File::new(f as u32), Rank::new(r as u32));
    }

    fn cursor_action(&mut self) {
        if !self.my_turn() {
            return;
        }
        let sq = self.cursor;
        if let Some(from) = self.selected {
            if self.targets.contains(&sq) {
                if let Some(m) = self.game.move_between(from, sq) {
                    self.play_my_move(m);
                }
                self.selected = None;
                self.targets.clear();
                return;
            }
            if from == sq {
                self.selected = None;
                self.targets.clear();
                return;
            }
        }
        if self.is_my_piece(sq) {
            self.targets = self.game.legal_targets(sq);
            self.selected = if self.targets.is_empty() { None } else { Some(sq) };
        }
    }

    fn is_my_piece(&self, sq: Square) -> bool {
        match (self.game.piece_char_at(sq), self.my_side) {
            (Some(p), Some(Side::White)) => p.is_ascii_uppercase(),
            (Some(p), Some(Side::Black)) => p.is_ascii_lowercase(),
            _ => false,
        }
    }

    fn play_my_move(&mut self, m: crate::game::ParsedMove) {
        let Some(id) = self.game_id.clone() else { return };
        let uci = m.to_uci();
        self.game.play(&m);
        let san = self.game.san_history().last().cloned().unwrap_or_default();
        let file = self.fake_file();
        self.push(Entry::Tool {
            title: format!("Edit({file})"),
            detail: format!("Updated {file} with 1 addition and 1 removal ({san})"),
        });
        self.actions.push(Action::Move { game_id: id, uci });
    }

    fn submit(&mut self, text: &str) {
        let cmd = commands::parse(text);
        if !matches!(cmd, Command::Noop) {
            self.push(Entry::User(text.trim().to_string()));
        }
        match cmd {
            Command::Noop => {}
            Command::Move(m) => {
                if !self.my_turn() {
                    self.push(Entry::Error("not your move".into()));
                    return;
                }
                match self.game.parse_input(&m) {
                    Ok(parsed) => {
                        self.selected = None;
                        self.targets.clear();
                        self.play_my_move(parsed);
                    }
                    Err(e) => self.push(Entry::Error(e.to_string())),
                }
            }
            Command::NewAi { level } => {
                self.push(Entry::Text(format!("Starting a new session against Stockfish level {level}.")));
                self.actions.push(Action::NewAi { level });
            }
            Command::Seek { minutes, increment, days } => {
                let desc = match days {
                    Some(d) => format!("correspondence, {d} days per move"),
                    None => format!("{minutes}+{increment}"),
                };
                self.push(Entry::Text(format!("Looking for an opponent ({desc}). This can take a while.")));
                self.actions.push(Action::Seek { minutes, increment, days });
            }
            Command::Games => self.actions.push(Action::ListGames),
            Command::SwitchGame(arg) => {
                let id = arg
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| self.playing.get(n.wrapping_sub(1)).map(|g| g.id.clone()))
                    .unwrap_or(arg);
                self.actions.push(Action::OpenGame(id));
            }
            Command::Resign => match self.game_id.clone() {
                Some(id) if !self.is_over() => self.actions.push(Action::Resign(id)),
                _ => self.push(Entry::Error("no game in progress".into())),
            },
            Command::Draw => match self.game_id.clone() {
                Some(id) if !self.is_over() => self.actions.push(Action::Draw(id)),
                _ => self.push(Entry::Error("no game in progress".into())),
            },
            Command::Theme => self.cycle_theme(),
            Command::Flip => self.flipped = !self.flipped,
            Command::Camouflage => self.camouflage = !self.camouflage,
            Command::Help => self.push(Entry::Text(HELP.trim().to_string())),
            Command::Quit => self.should_quit = true,
            Command::Review => {
                if self.game_id.is_none() || self.game.move_count() == 0 {
                    self.push(Entry::Error("nothing to review yet".into()));
                } else {
                    self.push(Entry::Text("Stepping through the changes. Left/Right to move, Esc to stop.".into()));
                    self.enter_review();
                }
            }
            Command::Analyze => match self.game_id.clone() {
                Some(id) => {
                    self.push(Entry::Text("Opening the full report in the browser.".into()));
                    self.actions.push(Action::OpenBrowser(format!("https://lichess.org/{id}")));
                }
                None => self.push(Entry::Error("no game to analyze".into())),
            },
            Command::Error(e) => self.push(Entry::Error(e)),
        }
    }

    fn cycle_theme(&mut self) {
        self.theme = Theme::get(self.theme.kind.next());
    }

    /// Jump to the final position in review and ask the worker for evaluations.
    fn enter_review(&mut self) {
        let Some(id) = self.game_id.clone() else { return };
        let n = self.game.move_count();
        self.review = Some(n);
        self.selected = None;
        self.targets.clear();
        let fens = (0..=n).map(|p| self.game.fen_at(p)).collect();
        self.actions.push(Action::FetchAnalysis { game_id: id, fens });
    }

    // ----- lichess events -----

    pub fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::GameStart { game_id, .. } => {
                self.actions.push(Action::OpenGame(game_id));
            }
            Event::GameFinish { game_id } => {
                if self.game_id.as_deref() == Some(&game_id) && !self.is_over() {
                    self.status = "finished".into();
                    self.push(Entry::Text("Session finished.".into()));
                }
            }
            Event::GameFull { game_id, white, black, moves, status, wtime, btime } => {
                let side = if white.eq_ignore_ascii_case(&self.username) {
                    Side::White
                } else if black.eq_ignore_ascii_case(&self.username) {
                    Side::Black
                } else {
                    Side::White
                };
                self.game_id = Some(game_id);
                self.my_side = Some(side);
                self.opponent = if side == Side::White { black } else { white };
                self.flipped = side == Side::Black;
                self.cursor = if side == Side::White { Square::E2 } else { Square::E7 };
                self.selected = None;
                self.targets.clear();
                self.review = None;
                self.evals.clear();
                self.status = status.clone();
                self.wtime = wtime;
                self.btime = btime;
                if let Err(e) = self.game.set_moves(&moves) {
                    self.push(Entry::Error(format!("could not load game: {e}")));
                }
                let color = if side == Side::White { "white" } else { "black" };
                self.push(Entry::Tool {
                    title: format!("Read({})", self.fake_file()),
                    detail: format!("Read {} lines (vs {}, you are {})", 40 + self.game.move_count(), self.opponent, color),
                });
                if self.is_over() {
                    self.report_result(&status, None);
                }
            }
            Event::GameState { game_id, moves, status, winner, wtime, btime, draw_offer } => {
                if self.game_id.as_deref() != Some(&game_id) {
                    return;
                }
                let before = self.game.move_count();
                if let Err(e) = self.game.set_moves(&moves) {
                    self.push(Entry::Error(format!("desync: {e}")));
                    return;
                }
                self.wtime = wtime;
                self.btime = btime;
                let after = self.game.move_count();
                if after > before {
                    let opponent_moved = self.my_side != Some(self.game.turn().other());
                    if opponent_moved {
                        let san = self.game.san_history().last().cloned().unwrap_or_default();
                        let file = self.fake_file();
                        self.push(Entry::Tool {
                            title: format!("Read({file})"),
                            detail: format!("Read {} lines ({san})", 12 + after * 3),
                        });
                        if self.game.in_check() && self.game.outcome().is_none() {
                            self.push(Entry::Text("Heads up: the current branch fails a check.".into()));
                        }
                    }
                }
                if let Some(side) = draw_offer {
                    if Some(side) != self.my_side {
                        self.push(Entry::Text("The other side proposes a draw. /draw to accept.".into()));
                    }
                }
                let was_over = self.is_over();
                self.status = status.clone();
                if self.is_over() && !was_over {
                    self.report_result(&status, winner);
                }
            }
            Event::Chat { username, text, .. } => {
                self.push(Entry::Text(format!("{username}: {text}")));
            }
            Event::Playing(list) => {
                if list.is_empty() {
                    self.push(Entry::Text("No games in progress. /new ai 3 or /seek to start one.".into()));
                } else {
                    let mut text = String::from("Games in progress:");
                    for (i, g) in list.iter().enumerate() {
                        let color = if g.color == Side::White { "white" } else { "black" };
                        let turn = if g.my_turn { "your move" } else { "waiting" };
                        text.push_str(&format!("\n  {}. vs {} ({color}, {}, {turn})  /game {}", i + 1, g.opponent, g.speed, i + 1));
                    }
                    self.push(Entry::Text(text));
                }
                self.playing = list;
            }
            Event::Info(t) => self.push(Entry::Text(t)),
            Event::Error(e) => self.push(Entry::Error(e)),
            Event::StreamEnded { game_id } => {
                if self.game_id.as_deref() == Some(&game_id) && !self.is_over() {
                    self.push(Entry::Text("Connection to the session dropped. /game to reopen.".into()));
                }
            }
            Event::Analysis { game_id, evals, from_lichess } => {
                if self.game_id.as_deref() != Some(&game_id) {
                    return;
                }
                self.evals = evals;
                self.report_analysis(from_lichess);
            }
        }
    }

    fn report_result(&mut self, status: &str, winner: Option<Side>) {
        let winner = winner.or_else(|| match self.game.outcome() {
            Some(Outcome::Checkmate { winner }) => Some(winner),
            _ => None,
        });
        let how = match status {
            "mate" => "by checkmate",
            "resign" => "by resignation",
            "outoftime" | "timeout" => "on time",
            "stalemate" => "by stalemate",
            "draw" => "by agreement",
            "aborted" => "(aborted)",
            _ => "",
        };
        let verdict = match (winner, self.my_side) {
            (Some(w), Some(me)) if w == me => "you won",
            (Some(_), Some(_)) => "you lost",
            _ if status == "aborted" => "aborted",
            _ => "drawn",
        };
        let mut text = format!("Done. Summary: {verdict} {how} against {}.", self.opponent).replace("  ", " ");
        if self.game.move_count() > 0 && status != "aborted" {
            text.push_str(" Reviewing the changes now: Left/Right to step, Esc to stop, /analyze for the full report.");
            self.push(Entry::Text(text));
            self.enter_review();
        } else {
            self.push(Entry::Text(text));
        }
    }

    /// Summarise the evaluations as if a linter had run over the game.
    fn report_analysis(&mut self, from_lichess: bool) {
        let n = self.game.move_count();
        let evaluated = self.evals.iter().filter(|e| e.eval.is_some()).count();
        if evaluated == 0 {
            self.push(Entry::Text(
                "No evaluations are available for this game yet. /analyze opens the Lichess page where you can request computer analysis, then run /review again.".into(),
            ));
            return;
        }
        let sans = self.game.san_history();
        let mut warnings: Vec<String> = Vec::new();
        let mut counts = [0usize; 3];
        for ply in 1..=n {
            let mover = if ply % 2 == 1 { Side::White } else { Side::Black };
            let entry = self.evals.get(ply);
            let name: Option<String> = if from_lichess {
                entry.and_then(|e| e.judgment.clone())
            } else {
                let before = self.evals.get(ply - 1).and_then(|e| e.eval);
                let after = entry.and_then(|e| e.eval);
                match (before, after) {
                    (Some(b), Some(a)) => judge(b, a, mover).map(String::from),
                    _ => None,
                }
            };
            let Some(name) = name else { continue };
            let lower = name.to_ascii_lowercase();
            let (mark, slot) = match lower.as_str() {
                "blunder" => ("??", 0),
                "mistake" => ("?", 1),
                _ => ("?!", 2),
            };
            counts[slot] += 1;
            let number = if mover == Side::White {
                format!("{}.", ply.div_ceil(2))
            } else {
                format!("{}...", ply / 2)
            };
            let san = sans.get(ply - 1).cloned().unwrap_or_default();
            let best = entry
                .and_then(|e| e.best.as_deref())
                .and_then(|uci| self.game.position_at(ply - 1).san_of(uci))
                .map(|s| format!(", best was {s}"))
                .unwrap_or_default();
            let whose = if Some(mover) == self.my_side { "your" } else { "their" };
            warnings.push(format!(
                "warning: {lower} in {whose} move {number} {san}{mark}{best}\n  --> {}:{}:{}",
                FILES[ply % FILES.len()],
                ply,
                ply % 7 + 1
            ));
        }
        self.push(Entry::Tool {
            title: "Bash(cargo clippy --all-targets)".into(),
            detail: format!(
                "{} positions evaluated{}",
                evaluated,
                if from_lichess { " (server analysis)" } else { " (cloud, partial)" }
            ),
        });
        let mut text = format!(
            "{} blunder{}, {} mistake{}, {} inaccurac{} in {} moves.",
            counts[0],
            if counts[0] == 1 { "" } else { "s" },
            counts[1],
            if counts[1] == 1 { "" } else { "s" },
            counts[2],
            if counts[2] == 1 { "y" } else { "ies" },
            n
        );
        for w in &warnings {
            text.push('\n');
            text.push_str(w);
        }
        self.push(Entry::Text(text));
    }
}

const HELP: &str = "
Keys: arrows or hjkl move the cursor, Enter/Space selects and moves, Esc cancels.
Tab hides the board as file output, Ctrl+T switches theme, Ctrl+C quits.
Type moves as e2 e4, e2e4, or Nf3. Commands: /games, /game N, /new ai 1-8,
/seek 15+10, /seek corr 2, /resign, /draw, /flip, /hide, /theme, /quit.
After a game: Left/Right step through it, /review restarts that, /analyze opens Lichess.
";
