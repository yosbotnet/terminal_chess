//! Visual themes that imitate agent CLIs. Colors use the terminal's own background.

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Claude,
    Codex,
}

impl ThemeKind {
    pub fn from_name(name: &str) -> Option<ThemeKind> {
        match name.to_ascii_lowercase().as_str() {
            "claude" => Some(ThemeKind::Claude),
            "codex" => Some(ThemeKind::Codex),
            _ => None,
        }
    }

    pub fn next(self) -> ThemeKind {
        match self {
            ThemeKind::Claude => ThemeKind::Codex,
            ThemeKind::Codex => ThemeKind::Claude,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub kind: ThemeKind,
    pub name: &'static str,
    /// Header banner text, e.g. "Claude Code".
    pub banner: &'static str,
    pub banner_glyph: &'static str,
    pub model: &'static str,
    pub prompt: &'static str,
    /// Bullet used before fake tool calls.
    pub tool_bullet: &'static str,
    /// Bullet used before assistant prose.
    pub text_bullet: &'static str,
    pub footer_left: &'static str,
    pub footer_right: &'static str,
    pub boxed_prompt: bool,
    pub accent: Color,
    pub fg: Color,
    pub dim: Color,
    pub light_square: Color,
    pub dark_square: Color,
    pub white_piece: Color,
    pub black_piece: Color,
    pub cursor: Color,
    pub selected: Color,
    pub last_move: Color,
    pub check: Color,
    pub good: Color,
    pub bad: Color,
}

impl Theme {
    pub fn get(kind: ThemeKind) -> Theme {
        match kind {
            ThemeKind::Claude => Theme {
                kind,
                name: "claude",
                banner: "Claude Code",
                banner_glyph: "\u{273b}",
                model: "claude-fable-5-1",
                prompt: ">",
                tool_bullet: "\u{23fa}",
                text_bullet: "\u{23fa}",
                footer_left: "? for shortcuts",
                footer_right: "auto-accept edits on (shift+tab to cycle)",
                boxed_prompt: true,
                accent: Color::Rgb(215, 119, 87),
                fg: Color::Reset,
                dim: Color::Rgb(128, 128, 128),
                light_square: Color::Rgb(58, 58, 62),
                dark_square: Color::Rgb(38, 38, 42),
                white_piece: Color::Rgb(235, 219, 178),
                black_piece: Color::Rgb(215, 119, 87),
                cursor: Color::Rgb(250, 189, 47),
                selected: Color::Rgb(69, 90, 120),
                last_move: Color::Rgb(72, 72, 96),
                check: Color::Rgb(160, 50, 50),
                good: Color::Rgb(142, 192, 124),
                bad: Color::Rgb(251, 73, 52),
            },
            ThemeKind::Codex => Theme {
                kind,
                name: "codex",
                banner: "OpenAI Codex",
                banner_glyph: ">_",
                model: "gpt-5.4-codex",
                prompt: "\u{203a}",
                tool_bullet: "\u{2022}",
                text_bullet: "\u{2022}",
                footer_left: "? for shortcuts",
                footer_right: "100% context left",
                boxed_prompt: false,
                accent: Color::Rgb(160, 160, 160),
                fg: Color::Reset,
                dim: Color::Rgb(110, 110, 110),
                light_square: Color::Rgb(52, 52, 52),
                dark_square: Color::Rgb(34, 34, 34),
                white_piece: Color::Rgb(230, 230, 230),
                black_piece: Color::Rgb(140, 170, 200),
                cursor: Color::Rgb(255, 255, 255),
                selected: Color::Rgb(70, 80, 95),
                last_move: Color::Rgb(66, 66, 66),
                check: Color::Rgb(140, 50, 50),
                good: Color::Rgb(120, 190, 130),
                bad: Color::Rgb(220, 90, 90),
            },
        }
    }
}
