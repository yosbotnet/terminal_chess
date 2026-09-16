//! Board renderers. Both produce ratatui lines from a `Game` and a `BoardView`.

pub mod board;
pub mod camo;

use shakmaty::{File, Rank, Square};

/// Everything the renderers need beyond the position itself.
#[derive(Debug, Clone, Default)]
pub struct BoardView {
    pub flipped: bool,
    pub cursor: Option<Square>,
    pub selected: Option<Square>,
    pub targets: Vec<Square>,
    pub last_move: Option<(Square, Square)>,
    pub check_square: Option<Square>,
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
