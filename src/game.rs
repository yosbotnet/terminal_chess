//! Chess position state built on shakmaty. Knows nothing about Lichess or the UI.

use anyhow::{anyhow, bail, Result};
use shakmaty::san::{San, SanPlus};
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, Color, Move, Position, Role, Square};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    White,
    Black,
}

impl From<Color> for Side {
    fn from(c: Color) -> Self {
        match c {
            Color::White => Side::White,
            Color::Black => Side::Black,
        }
    }
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Checkmate { winner: Side },
    Draw,
}

/// A move that is legal in the position it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMove {
    inner: Move,
    uci: String,
}

impl ParsedMove {
    pub fn to_uci(&self) -> String {
        self.uci.clone()
    }
    pub fn from_square(&self) -> Option<Square> {
        self.inner.from()
    }
    pub fn to_square(&self) -> Square {
        // For castling we want the king destination, which UCI standard mode encodes.
        self.uci[2..4].parse().expect("uci to square")
    }
}

#[derive(Debug, Clone)]
pub struct Game {
    pos: Chess,
    history: Vec<ParsedMove>,
    san_history: Vec<String>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        Game {
            pos: Chess::default(),
            history: Vec::new(),
            san_history: Vec::new(),
        }
    }

    pub fn turn(&self) -> Side {
        self.pos.turn().into()
    }

    pub fn move_count(&self) -> usize {
        self.history.len()
    }

    pub fn last_move(&self) -> Option<&ParsedMove> {
        self.history.last()
    }

    pub fn san_history(&self) -> Vec<String> {
        self.san_history.clone()
    }

    pub fn in_check(&self) -> bool {
        self.pos.is_check()
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.pos.outcome().map(|o| match o {
            shakmaty::Outcome::Decisive { winner } => Outcome::Checkmate {
                winner: winner.into(),
            },
            shakmaty::Outcome::Draw => Outcome::Draw,
        })
    }

    /// Piece letter in FEN style: uppercase white, lowercase black.
    pub fn piece_char_at(&self, sq: Square) -> Option<char> {
        self.pos.board().piece_at(sq).map(|p| p.char())
    }

    pub fn king_square(&self, side: Side) -> Option<Square> {
        let color = match side {
            Side::White => Color::White,
            Side::Black => Color::Black,
        };
        self.pos.board().king_of(color)
    }

    /// Destination squares of every legal move starting at `from`.
    pub fn legal_targets(&self, from: Square) -> Vec<Square> {
        self.pos
            .legal_moves()
            .iter()
            .filter(|m| m.from() == Some(from))
            .map(|m| {
                let uci = UciMove::from_move(m, CastlingMode::Standard).to_string();
                uci[2..4].parse::<Square>().expect("uci square")
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Replace the whole game with the given space-separated UCI move list.
    pub fn set_moves(&mut self, moves: &str) -> Result<()> {
        let mut fresh = Game::new();
        for tok in moves.split_whitespace() {
            let m = fresh.parse_uci(tok)?;
            fresh.play(&m);
        }
        *self = fresh;
        Ok(())
    }

    /// Play a move that was parsed against the current position.
    pub fn play(&mut self, m: &ParsedMove) {
        let san = SanPlus::from_move_and_play_unchecked(&mut self.pos, &m.inner);
        self.san_history.push(san.to_string());
        self.history.push(m.clone());
    }

    fn parse_uci(&self, text: &str) -> Result<ParsedMove> {
        let uci = UciMove::from_ascii(text.as_bytes())
            .map_err(|_| anyhow!("not a move: {text}"))?;
        let m = uci
            .to_move(&self.pos)
            .map_err(|_| anyhow!("illegal move: {text}"))?;
        Ok(self.wrap(m))
    }

    fn wrap(&self, m: Move) -> ParsedMove {
        let uci = UciMove::from_move(&m, CastlingMode::Standard).to_string();
        ParsedMove { inner: m, uci }
    }

    /// Accepts `e2 e4`, `e2e4`, `e7e8q`, or SAN such as `Nf3`, `O-O`, `exd5`.
    /// Coordinate input that lands a pawn on the last rank promotes to a queen.
    pub fn parse_input(&self, text: &str) -> Result<ParsedMove> {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.is_empty() {
            bail!("empty move");
        }
        if let Some(m) = self.try_coordinates(&compact) {
            return Ok(m);
        }
        let san = San::from_ascii(compact.trim_end_matches(['+', '#']).as_bytes())
            .map_err(|_| anyhow!("not a move: {text}"))?;
        let m = san
            .to_move(&self.pos)
            .map_err(|_| anyhow!("illegal move: {text}"))?;
        Ok(self.wrap(m))
    }

    /// Try to read `from`/`to` squares for the cursor and the coordinate prompt.
    pub fn move_between(&self, from: Square, to: Square) -> Option<ParsedMove> {
        self.try_coordinates(&format!("{from}{to}"))
    }

    fn try_coordinates(&self, compact: &str) -> Option<ParsedMove> {
        let lower = compact.to_ascii_lowercase();
        if !(lower.len() == 4 || lower.len() == 5) {
            return None;
        }
        let from: Square = lower.get(0..2)?.parse().ok()?;
        let to: Square = lower.get(2..4)?.parse().ok()?;
        if let Ok(m) = self.parse_uci(&lower) {
            return Some(m);
        }
        if lower.len() == 4
            && self.pos.board().role_at(from) == Some(Role::Pawn)
            && (to.rank() == shakmaty::Rank::First || to.rank() == shakmaty::Rank::Eighth)
        {
            return self.parse_uci(&format!("{lower}q")).ok();
        }
        None
    }
}
