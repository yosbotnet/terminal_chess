//! Parses text typed into the prompt into a command.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Noop,
    Move(String),
    NewAi { level: u8 },
    Seek { minutes: u32, increment: u32, days: Option<u32> },
    Games,
    SwitchGame(String),
    Resign,
    Draw,
    Theme,
    Flip,
    Camouflage,
    Help,
    Quit,
    Review,
    Analyze,
    Say(String),
    /// `save` is `None` to print the PGN, `Some(path)` to write it (empty path = default).
    Pgn { save: Option<String> },
    Puzzle,
    Panic,
    Error(String),
}

pub fn parse(input: &str) -> Command {
    let text = input.trim();
    if text.is_empty() {
        return Command::Noop;
    }
    if !text.starts_with('/') {
        return Command::Move(text.to_string());
    }
    let mut parts = text.split_whitespace();
    let name = parts.next().unwrap_or_default();
    let args: Vec<&str> = parts.collect();
    match name {
        "/new" => parse_new(&args),
        "/seek" => parse_seek(&args),
        "/games" => Command::Games,
        "/game" => match args.first() {
            Some(id) => Command::SwitchGame((*id).to_string()),
            None => Command::Error("usage: /game <number or id>".into()),
        },
        "/resign" => Command::Resign,
        "/draw" => Command::Draw,
        "/theme" => Command::Theme,
        "/flip" => Command::Flip,
        "/hide" => Command::Camouflage,
        "/help" => Command::Help,
        "/quit" | "/exit" => Command::Quit,
        "/review" => Command::Review,
        "/analyze" | "/analyse" => Command::Analyze,
        "/say" => {
            let msg = text[4..].trim();
            if msg.is_empty() {
                Command::Error("usage: /say <message>".into())
            } else {
                Command::Say(msg.to_string())
            }
        }
        "/pgn" => match args.first() {
            None => Command::Pgn { save: None },
            Some(&"save") => Command::Pgn { save: Some(args[1..].join(" ")) },
            _ => Command::Error("usage: /pgn or /pgn save [path]".into()),
        },
        "/puzzle" => Command::Puzzle,
        "/panic" => Command::Panic,
        other => Command::Error(format!("unknown command: {other}")),
    }
}

fn parse_new(args: &[&str]) -> Command {
    match args.first() {
        Some(&"ai") | None => {
            let level = args.get(1).map(|s| s.parse::<u8>()).unwrap_or(Ok(1));
            match level {
                Ok(l) if (1..=8).contains(&l) => Command::NewAi { level: l },
                _ => Command::Error("level must be 1-8".into()),
            }
        }
        _ => Command::Error("usage: /new ai [1-8]".into()),
    }
}

fn parse_seek(args: &[&str]) -> Command {
    const USAGE: &str = "usage: /seek 15+10 or /seek corr 2";
    match args.first() {
        None => Command::Seek { minutes: 0, increment: 0, days: Some(2) },
        Some(&"corr") => {
            let days = args.get(1).map(|s| s.parse::<u32>()).unwrap_or(Ok(2));
            match days {
                Ok(d) if (1..=14).contains(&d) => {
                    Command::Seek { minutes: 0, increment: 0, days: Some(d) }
                }
                _ => Command::Error(USAGE.into()),
            }
        }
        Some(tc) => {
            let Some((m, i)) = tc.split_once('+') else {
                return Command::Error(USAGE.into());
            };
            match (m.parse::<u32>(), i.parse::<u32>()) {
                (Ok(minutes), Ok(increment)) if minutes > 0 => {
                    Command::Seek { minutes, increment, days: None }
                }
                _ => Command::Error(USAGE.into()),
            }
        }
    }
}
