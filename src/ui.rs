//! Screen layout: header, scrolling transcript, board block, prompt, footer.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{App, Entry};
use crate::render::{board, camo};
use crate::theme::{Theme, ThemeKind};

const FAKE_DIR: &str = "~/Git-projects/engine";

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if area.height < 4 || area.width < 12 {
        return;
    }
    let theme = app.theme().clone();
    if app.panic_active() {
        draw_panic(f, area, app, &theme);
        return;
    }
    let board_lines = board_block(app, &theme);
    let prompt_height: u16 = if theme.boxed_prompt { 3 } else { 2 };
    let board_height = (board_lines.len() as u16).min(area.height.saturating_sub(6));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(board_height),
            Constraint::Length(prompt_height),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(f, chunks[0], &theme);
    draw_transcript(f, chunks[1], app, &theme);
    f.render_widget(Paragraph::new(board_lines), chunks[2]);
    draw_prompt(f, chunks[3], app, &theme);
    draw_footer(f, chunks[4], app, &theme);
}

fn draw_header(f: &mut Frame, area: Rect, theme: &Theme) {
    let bold = Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(theme.dim);
    let line = match theme.kind {
        ThemeKind::Claude => Line::from(vec![
            Span::styled(format!(" {} {} ", theme.banner_glyph, theme.banner), bold),
            Span::styled("v2.1.14", dim),
            Span::styled(format!("   {}  \u{00b7}  {}", theme.model, FAKE_DIR), dim),
        ]),
        ThemeKind::Codex => Line::from(vec![
            Span::styled(format!(" {} {} ", theme.banner_glyph, theme.banner), bold),
            Span::styled("(v0.52.0)", dim),
            Span::styled(format!("   model: {}   directory: {}", theme.model, FAKE_DIR), dim),
        ]),
    };
    f.render_widget(Paragraph::new(vec![line, Line::raw("")]), area);
}

fn entry_lines(entry: &Entry, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let dim = Style::default().fg(theme.dim);
    let mut out = Vec::new();
    match entry {
        Entry::User(t) => {
            out.push(Line::from(Span::styled(format!("{} {}", theme.prompt, t), dim)));
        }
        Entry::Tool { title, detail } => {
            out.push(Line::from(vec![
                Span::styled(format!("{} ", theme.tool_bullet), Style::default().fg(theme.good)),
                Span::styled(title.clone(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            ]));
            out.push(Line::from(Span::styled(format!("  \u{23bf}  {detail}"), dim)));
        }
        Entry::Block { title, lines } => {
            out.push(Line::from(vec![
                Span::styled(format!("{} ", theme.tool_bullet), Style::default().fg(theme.good)),
                Span::styled(title.clone(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            ]));
            out.push(Line::from(Span::styled(
                format!("  \u{23bf}  Wrote {} lines", lines.len()),
                dim,
            )));
            for (i, l) in lines.iter().enumerate() {
                out.push(Line::from(vec![
                    Span::styled(format!("{:>8}  ", i + 1), dim),
                    Span::styled(l.clone(), Style::default().fg(theme.fg)),
                ]));
            }
        }
        Entry::Text(t) => {
            for (i, para) in t.lines().enumerate() {
                let prefix = if i == 0 { format!("{} ", theme.text_bullet) } else { "  ".to_string() };
                for (j, chunk) in wrap(para, width.saturating_sub(2)).into_iter().enumerate() {
                    let p = if j == 0 { prefix.clone() } else { "  ".to_string() };
                    out.push(Line::from(vec![
                        Span::styled(p, Style::default().fg(theme.accent)),
                        Span::styled(chunk, Style::default().fg(theme.fg)),
                    ]));
                }
            }
        }
        Entry::Error(t) => {
            out.push(Line::from(vec![
                Span::styled(format!("{} ", theme.text_bullet), Style::default().fg(theme.bad)),
                Span::styled(t.clone(), Style::default().fg(theme.bad)),
            ]));
        }
    }
    out.push(Line::raw(""));
    out
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

fn draw_transcript(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let width = area.width as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    for e in app.transcript() {
        lines.extend(entry_lines(e, theme, width));
    }
    let h = area.height as usize;
    let start = lines.len().saturating_sub(h);
    let visible: Vec<Line<'static>> = lines.drain(start..).collect();
    f.render_widget(Paragraph::new(visible), area);
}

fn board_block(app: &App, theme: &Theme) -> Vec<Line<'static>> {
    let dim = Style::default().fg(theme.dim);
    let view = app.view();
    let shown = app.board_game();
    // In review the detail line carries the ply, the move, and the evaluation.
    let note = match (app.review_ply(), app.clocks()) {
        (Some(ply), _) => {
            let san = shown.san_history().last().cloned().unwrap_or_else(|| "start".into());
            let eval = app.current_eval().map(|e| format!("  {e}")).unwrap_or_default();
            format!("  ({ply}/{} {san}{eval})", app.game().move_count())
        }
        (None, Some((w, b))) => format!(
            "  white {}  black {}",
            App::format_clock(w),
            App::format_clock(b)
        ),
        (None, None) => String::new(),
    };
    let mut lines = Vec::new();
    if app.camouflage() {
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", theme.tool_bullet), Style::default().fg(theme.good)),
            Span::styled("Read(tests/fixtures/position.txt)", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(Span::styled(format!("  \u{23bf}  Read 8 lines{note}"), dim)));
        lines.extend(camo::render(&shown, &view, theme));
    } else {
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", theme.tool_bullet), Style::default().fg(theme.good)),
            Span::styled("Bash(cargo run --release -- --render)", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(Span::styled(format!("  \u{23bf}{note}"), dim)));
        lines.extend(board::render(&shown, &view, theme));
    }
    lines.push(Line::raw(""));
    lines
}

fn draw_prompt(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let lead = if theme.boxed_prompt { " " } else { "" };
    let text = Line::from(vec![
        Span::styled(format!("{lead}{} ", theme.prompt), Style::default().fg(theme.accent)),
        Span::styled(app.input().to_string(), Style::default().fg(theme.fg)),
        Span::styled("\u{2588}", Style::default().fg(theme.dim)),
    ]);
    if theme.boxed_prompt {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(theme.dim));
        f.render_widget(Paragraph::new(text).block(block), area);
    } else {
        let rule = Line::from(Span::styled(
            "\u{2500}".repeat(area.width as usize),
            Style::default().fg(theme.dim),
        ));
        f.render_widget(Paragraph::new(vec![rule, text]), area);
    }
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let dim = Style::default().fg(theme.dim);
    let left = format!("  {}", theme.footer_left);
    let mid = app.status_line();
    let mut right = format!("{}  ", theme.footer_right);
    let w = area.width as usize;
    let mut used = left.chars().count() + mid.chars().count() + right.chars().count();
    if used + 4 > w {
        // Drop the decorative right side before letting anything get clipped.
        right.clear();
        used = left.chars().count() + mid.chars().count();
    }
    let (gap1, gap2) = if used >= w {
        (1, 1)
    } else {
        let free = w - used;
        (free / 2, free - free / 2)
    };
    let line = Line::from(vec![
        Span::styled(left, dim),
        Span::raw(" ".repeat(gap1)),
        Span::styled(mid, dim),
        Span::raw(" ".repeat(gap2)),
        Span::styled(right, dim),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

/// The panic screen: a canned agent session that types itself out over time.
fn draw_panic(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let prompt_height: u16 = if theme.boxed_prompt { 3 } else { 2 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(prompt_height),
            Constraint::Length(1),
        ])
        .split(area);
    draw_header(f, chunks[0], theme);

    let dim = Style::default().fg(theme.dim);
    let script = app.panic_script();
    // One script line every 350ms, so it looks like the agent is still working.
    let visible = ((app.panic_elapsed_ms() / 350) as usize + 1).min(script.len());
    let width = chunks[1].width as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (kind, text) in &script[..visible] {
        match *kind {
            "user" => {
                lines.push(Line::from(Span::styled(format!("{} {text}", theme.prompt), dim)));
                lines.push(Line::raw(""));
            }
            "tool" => lines.push(Line::from(vec![
                Span::styled(format!("{} ", theme.tool_bullet), Style::default().fg(theme.good)),
                Span::styled(text.to_string(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            ])),
            "detail" => {
                lines.push(Line::from(Span::styled(format!("  \u{23bf}  {text}"), dim)));
                lines.push(Line::raw(""));
            }
            "spinner" => {
                let frames = ['\u{2733}', '\u{2736}', '\u{2731}', '\u{2735}'];
                let frame = frames[(app.panic_elapsed_ms() / 200) as usize % frames.len()];
                lines.push(Line::from(vec![
                    Span::styled(format!("{frame} "), Style::default().fg(theme.accent)),
                    Span::styled(format!("{text}\u{2026} "), Style::default().fg(theme.accent)),
                    Span::styled("(esc to interrupt)", dim),
                ]));
            }
            _ => {
                for (j, chunk) in wrap(text, width.saturating_sub(2)).into_iter().enumerate() {
                    let p = if j == 0 { format!("{} ", theme.text_bullet) } else { "  ".to_string() };
                    lines.push(Line::from(vec![
                        Span::styled(p, Style::default().fg(theme.accent)),
                        Span::styled(chunk, Style::default().fg(theme.fg)),
                    ]));
                }
                lines.push(Line::raw(""));
            }
        }
    }
    let h = chunks[1].height as usize;
    let start = lines.len().saturating_sub(h);
    let visible_lines: Vec<Line<'static>> = lines.drain(start..).collect();
    f.render_widget(Paragraph::new(visible_lines), chunks[1]);

    let empty = App::new(theme.kind, false, String::new());
    draw_prompt(f, chunks[2], &empty, theme);
    let footer = Line::from(vec![
        Span::styled(format!("  {}", theme.footer_left), dim),
        Span::raw(" ".repeat((chunks[3].width as usize).saturating_sub(theme.footer_left.len() + theme.footer_right.len() + 4))),
        Span::styled(format!("{}  ", theme.footer_right), dim),
    ]);
    f.render_widget(Paragraph::new(footer), chunks[3]);
}
