//! The camouflage board: looks like the numbered output of a file read.
//! Pieces are FEN letters, empty squares are dots. Ranks are file lines 1-8.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use shakmaty::Square;

use super::{files, ranks, BoardView};
use crate::game::Game;
use crate::theme::Theme;

pub fn render(game: &Game, view: &BoardView, theme: &Theme) -> Vec<Line<'static>> {
    let number = Style::default().fg(theme.dim);
    let mut lines = Vec::with_capacity(8);
    for (i, rank) in ranks(view.flipped).into_iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(17);
        spans.push(Span::styled(format!("{:>6}  ", i + 1), number));
        for (j, file) in files(view.flipped).into_iter().enumerate() {
            let sq = Square::from_coords(file, rank);
            let piece = game.piece_char_at(sq);
            let is_target = view.targets.contains(&sq);
            let ch = piece.unwrap_or('.');
            let mut style = Style::default().fg(match piece {
                Some(_) => theme.fg,
                None if is_target => theme.accent,
                None => theme.dim,
            });
            if view.cursor == Some(sq) {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if view.selected == Some(sq) {
                style = style.fg(theme.accent).add_modifier(Modifier::BOLD);
            }
            if is_target && piece.is_some() {
                style = style.fg(theme.accent);
            }
            if let Some((from, to)) = view.last_move {
                if sq == from || sq == to {
                    style = style.add_modifier(Modifier::BOLD);
                }
            }
            if view.check_square == Some(sq) {
                style = style.fg(theme.bad);
            }
            if let Some(m) = view.mark_at(sq) {
                style = style.bg(m.color());
            }
            if view.arrowhead_at(sq).is_some() {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            if view.is_arrow_origin(sq) {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if j > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(ch.to_string(), style));
        }
        lines.push(Line::from(spans));
    }
    lines
}
