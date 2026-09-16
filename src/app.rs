//! Application state and input handling. No terminal or network code lives here,
//! which keeps the whole thing testable with plain key events.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use shakmaty::{File, Rank, Square};

use crate::commands::{self, Command};
use crate::game::{Game, Outcome, Side};
use crate::lichess::{Event, PlayingGame};
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
    ctrl_c_armed: bool,
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
            ctrl_c_armed: false,
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
    pub fn game(&self) -> &Game {
        &self.game
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
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    pub fn view(&self) -> BoardView {
        let check_square = if self.game.in_check() {
            self.game.king_square(self.game.turn())
        } else {
            None
        };
        BoardView {
            flipped: self.flipped,
            cursor: Some(self.cursor),
            selected: self.selected,
            targets: self.targets.clone(),
            last_move: self
                .game
                .last_move()
                .and_then(|m| m.from_square().map(|f| (f, m.to_square()))),
            check_square,
        }
    }

    /// Short human status for the footer.
    pub fn status_line(&self) -> String {
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
        self.game_id.is_some() && self.my_side == Some(self.game.turn()) && !self.is_over()
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
                KeyCode::Char('c') => {
                    self.should_quit = true;
                }
                KeyCode::Char('t') => self.cycle_theme(),
                KeyCode::Char('u') => self.input.clear(),
                _ => {}
            }
            return;
        }
        self.ctrl_c_armed = false;
        match key.code {
            KeyCode::Tab => self.camouflage = !self.camouflage,
            KeyCode::Esc => {
                self.selected = None;
                self.targets.clear();
                self.input.clear();
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
            KeyCode::Up => self.move_cursor(0, 1),
            KeyCode::Down => self.move_cursor(0, -1),
            KeyCode::Left => self.move_cursor(-1, 0),
            KeyCode::Right => self.move_cursor(1, 0),
            KeyCode::Char(c) => {
                if self.input.is_empty() {
                    match c {
                        'k' => return self.move_cursor(0, 1),
                        'j' => return self.move_cursor(0, -1),
                        'h' => return self.move_cursor(-1, 0),
                        'l' => return self.move_cursor(1, 0),
                        ' ' => return self.cursor_action(),
                        _ => {}
                    }
                }
                self.input.push(c);
            }
            _ => {}
        }
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
            Command::Error(e) => self.push(Entry::Error(e)),
        }
    }

    fn cycle_theme(&mut self) {
        self.theme = Theme::get(self.theme.kind.next());
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
                        if self.game.in_check() {
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
        self.push(Entry::Text(format!(
            "Done. Summary: {verdict} {how} against {}.",
            self.opponent
        ).replace("  ", " ")));
    }
}

const HELP: &str = "
Keys: arrows or hjkl move the cursor, Enter/Space selects and moves, Esc cancels.
Tab hides the board as file output, Ctrl+T switches theme, Ctrl+C quits.
Type moves as e2 e4, e2e4, or Nf3. Commands: /games, /game N, /new ai 1-8,
/seek 15+10, /seek corr 2, /resign, /draw, /flip, /hide, /theme, /quit.
";
