//! The pretty board: unicode pieces on shaded squares, styled like a code block.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use shakmaty::Square;

use super::{files, ranks, BoardView};
use crate::game::Game;
use crate::theme::Theme;

const LABEL_WIDTH: usize = 4;

pub fn glyph(piece: char) -> char {
    match piece {
        'K' => '\u{2654}',
        'Q' => '\u{2655}',
        'R' => '\u{2656}',
        'B' => '\u{2657}',
        'N' => '\u{2658}',
        'P' => '\u{2659}',
        'k' => '\u{265a}',
        'q' => '\u{265b}',
        'r' => '\u{265c}',
        'b' => '\u{265d}',
        'n' => '\u{265e}',
        'p' => '\u{265f}',
        other => other,
    }
}

fn square_bg(sq: Square, view: &BoardView, theme: &Theme) -> ratatui::style::Color {
    if view.check_square == Some(sq) {
        return theme.check;
    }
    if view.selected == Some(sq) {
        return theme.selected;
    }
    if let Some(m) = view.mark_at(sq) {
        return m.color();
    }
    if let Some((from, to)) = view.last_move {
        if sq == from || sq == to {
            return theme.last_move;
        }
    }
    if sq.is_light() {
        theme.light_square
    } else {
        theme.dark_square
    }
}

pub fn render(game: &Game, view: &BoardView, theme: &Theme) -> Vec<Line<'static>> {
    let label = Style::default().fg(theme.dim);
    let mut lines = Vec::with_capacity(9);
    for rank in ranks(view.flipped) {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(10);
        spans.push(Span::styled(format!("{:>width$} ", rank.char(), width = LABEL_WIDTH - 1), label));
        for file in files(view.flipped) {
            let sq = Square::from_coords(file, rank);
            let piece = game.piece_char_at(sq);
            let is_target = view.targets.contains(&sq);
            let is_cursor = view.cursor == Some(sq);
            let bg = square_bg(sq, view, theme);
            let (left, right) = if is_cursor {
                ('[', ']')
            } else {
                (
                    view.arrowhead_at(sq).unwrap_or(' '),
                    if view.is_arrow_origin(sq) { '\u{2022}' } else { ' ' },
                )
            };
            let middle = match piece {
                Some(p) => glyph(p),
                None if is_target => '\u{00b7}',
                None => ' ',
            };
            let fg = match piece {
                Some(p) if p.is_ascii_uppercase() => theme.white_piece,
                Some(_) => theme.black_piece,
                None if is_target => theme.cursor,
                None => theme.dim,
            };
            let mut style = Style::default().bg(bg).fg(fg);
            if is_target && piece.is_some() {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            let bracket = Style::default().bg(bg).fg(theme.cursor).add_modifier(Modifier::BOLD);
            spans.push(Span::styled(left.to_string(), bracket));
            spans.push(Span::styled(middle.to_string(), style));
            spans.push(Span::styled(right.to_string(), bracket));
        }
        lines.push(Line::from(spans));
    }
    let mut footer = " ".repeat(LABEL_WIDTH);
    for file in files(view.flipped) {
        footer.push(' ');
        footer.push(file.char());
        footer.push(' ');
    }
    lines.push(Line::from(Span::styled(footer, label)));
    lines
}
