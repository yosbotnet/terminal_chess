//! Board renderers. Both produce ratatui lines from a `Game` and a `BoardView`.

pub mod board;
pub mod camo;

use ratatui::style::Color;
use shakmaty::{File, Rank, Square};

/// A user highlight on a square, cycling in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Green,
    Red,
    Blue,
    Yellow,
}

impl Mark {
    /// The next colour in the cycle, or `None` after yellow (mark removed).
    pub fn next(self) -> Option<Mark> {
        match self {
            Mark::Green => Some(Mark::Red),
            Mark::Red => Some(Mark::Blue),
            Mark::Blue => Some(Mark::Yellow),
            Mark::Yellow => None,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Mark::Green => Color::Rgb(60, 110, 70),
            Mark::Red => Color::Rgb(130, 55, 55),
            Mark::Blue => Color::Rgb(55, 85, 130),
            Mark::Yellow => Color::Rgb(130, 115, 45),
        }
    }
}

/// Everything the renderers need beyond the position itself.
#[derive(Debug, Clone, Default)]
pub struct BoardView {
    pub flipped: bool,
    pub cursor: Option<Square>,
    pub selected: Option<Square>,
    pub targets: Vec<Square>,
    pub last_move: Option<(Square, Square)>,
    pub check_square: Option<Square>,
    pub marks: Vec<(Square, Mark)>,
    pub arrows: Vec<(Square, Square)>,
}

impl BoardView {
    pub fn mark_at(&self, sq: Square) -> Option<Mark> {
        self.marks.iter().find(|(s, _)| *s == sq).map(|(_, m)| *m)
    }

    pub fn is_arrow_origin(&self, sq: Square) -> bool {
        self.arrows.iter().any(|(f, _)| *f == sq)
    }

    /// Arrowhead glyph for an arrow ending on `sq`, pointing the way it travels on screen.
    pub fn arrowhead_at(&self, sq: Square) -> Option<char> {
        let (from, to) = *self.arrows.iter().find(|(_, t)| *t == sq)?;
        let mut dx = (to.file() as i32 - from.file() as i32).signum();
        let mut dy = (to.rank() as i32 - from.rank() as i32).signum();
        if self.flipped {
            dx = -dx;
            dy = -dy;
        }
        Some(match (dx, dy) {
            (0, 1) => '\u{2191}',
            (0, -1) => '\u{2193}',
            (1, 0) => '\u{2192}',
            (-1, 0) => '\u{2190}',
            (1, 1) => '\u{2197}',
            (1, -1) => '\u{2198}',
            (-1, 1) => '\u{2196}',
            _ => '\u{2199}',
        })
    }
}

/// Iterates ranks top to bottom as they should appear on screen.
pub fn ranks(flipped: bool) -> Vec<Rank> {
    let mut r = Rank::ALL.to_vec();
    if !flipped {
        r.reverse();
    }
    r
}

/// Iterates files left to right as they should appear on screen.
pub fn files(flipped: bool) -> Vec<File> {
    let mut f = File::ALL.to_vec();
    if flipped {
        f.reverse();
    }
    f
}
