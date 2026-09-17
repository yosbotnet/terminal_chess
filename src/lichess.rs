//! Lichess Board API: NDJSON parsers (pure, tested) and a thin async HTTP client.

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;

pub use crate::game::Side;

const BASE: &str = "https://lichess.org";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    GameStart {
        game_id: String,
        color: Side,
    },
    GameFinish {
        game_id: String,
    },
    GameFull {
        game_id: String,
        white: String,
        black: String,
        moves: String,
        status: String,
        wtime: u64,
        btime: u64,
    },
    GameState {
        game_id: String,
        moves: String,
        status: String,
        winner: Option<Side>,
        wtime: u64,
        btime: u64,
        draw_offer: Option<Side>,
    },
    Chat {
        game_id: String,
        username: String,
        text: String,
    },
    /// A list of games in progress, from /api/account/playing.
    Playing(Vec<PlayingGame>),
    Info(String),
    Error(String),
    /// A game stream ended (network drop or game over).
    StreamEnded {
        game_id: String,
    },
    /// Evaluations for a finished game. `evals[i]` is the position after `i` half-moves.
    /// `from_lichess` is true when the game had server-side analysis with judgments.
    Analysis {
        game_id: String,
        evals: Vec<PlyEval>,
        from_lichess: bool,
    },
    Puzzle(Puzzle),
}

/// Engine evaluation from White's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eval {
    /// Centipawns.
    Cp(i32),
    /// Mate in N moves; negative means Black mates.
    Mate(i32),
}

impl std::fmt::Display for Eval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Eval::Cp(cp) => {
                let pawns = cp as f64 / 100.0;
                if cp > 0 {
                    write!(f, "+{pawns:.1}")
                } else {
                    write!(f, "{pawns:.1}")
                }
            }
            Eval::Mate(n) => write!(f, "#{n}"),
        }
    }
}

/// Evaluation attached to one position, plus what Lichess said about the move
/// that led to it (only when the game was analysed on Lichess).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlyEval {
    pub eval: Option<Eval>,
    pub best: Option<String>,
    pub judgment: Option<String>,
}

/// A tactics puzzle. Play every move of `pgn` to reach the starting position;
/// the side to move then plays `solution[0]`, the other side `solution[1]`, and so on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puzzle {
    pub id: String,
    pub rating: u32,
    pub pgn: String,
    pub solution: Vec<String>,
    pub themes: Vec<String>,
}

/// `puzzle` unless it was already shown this session.
pub fn pick_unseen(puzzle: Puzzle, seen: &[String]) -> Option<Puzzle> {
    (!seen.contains(&puzzle.id)).then_some(puzzle)
}

/// Body of /api/puzzle/next.
pub fn parse_puzzle(body: &str) -> Option<Puzzle> {
    let v: Value = serde_json::from_str(body).ok()?;
    let game = v.get("game")?;
    let puzzle = v.get("puzzle")?;
    let strings = |key: &str| -> Vec<String> {
        puzzle
            .get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
            .unwrap_or_default()
    };
    let solution = strings("solution");
    if solution.is_empty() {
        return None;
    }
    Some(Puzzle {
        id: str_of(puzzle, "id"),
        rating: puzzle.get("rating").and_then(Value::as_u64).unwrap_or(0) as u32,
        pgn: str_of(game, "pgn"),
        solution,
        themes: strings("themes"),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayingGame {
    pub id: String,
    pub color: Side,
    pub my_turn: bool,
    pub opponent: String,
    pub speed: String,
}

fn side_from(v: &Value) -> Option<Side> {
    match v.as_str()? {
        "white" => Some(Side::White),
        "black" => Some(Side::Black),
        _ => None,
    }
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn u64_of(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or_default()
}

/// One line from /api/stream/event.
pub fn parse_event_line(line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let game = v.get("game")?;
    match v.get("type")?.as_str()? {
        "gameStart" => Some(Event::GameStart {
            game_id: str_of(game, "gameId"),
            color: side_from(game.get("color")?)?,
        }),
        "gameFinish" => Some(Event::GameFinish {
            game_id: str_of(game, "gameId"),
        }),
        _ => None,
    }
}

fn player_name(p: &Value) -> String {
    if let Some(level) = p.get("aiLevel").and_then(Value::as_u64) {
        return format!("Stockfish level {level}");
    }
    let name = str_of(p, "name");
    if name.is_empty() {
        "anonymous".to_string()
    } else {
        name
    }
}

/// One line from /api/board/game/stream/{id}.
pub fn parse_game_line(game_id: &str, line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let game_id = game_id.to_string();
    match v.get("type")?.as_str()? {
        "gameFull" => {
            let state = v.get("state")?;
            Some(Event::GameFull {
                game_id,
                white: player_name(v.get("white")?),
                black: player_name(v.get("black")?),
                moves: str_of(state, "moves"),
                status: str_of(state, "status"),
                wtime: u64_of(state, "wtime"),
                btime: u64_of(state, "btime"),
            })
        }
        "gameState" => {
            let draw_offer = if v.get("wdraw").and_then(Value::as_bool) == Some(true) {
                Some(Side::White)
            } else if v.get("bdraw").and_then(Value::as_bool) == Some(true) {
                Some(Side::Black)
            } else {
                None
            };
            Some(Event::GameState {
                game_id,
                moves: str_of(&v, "moves"),
                status: str_of(&v, "status"),
                winner: v.get("winner").and_then(side_from),
                wtime: u64_of(&v, "wtime"),
                btime: u64_of(&v, "btime"),
                draw_offer,
            })
        }
        "chatLine" => Some(Event::Chat {
            game_id,
            username: str_of(&v, "username"),
            text: str_of(&v, "text"),
        }),
        _ => None,
    }
}

fn eval_from(v: &Value) -> Option<Eval> {
    if let Some(m) = v.get("mate").and_then(Value::as_i64) {
        return Some(Eval::Mate(m as i32));
    }
    v.get("eval")
        .or_else(|| v.get("cp"))
        .and_then(Value::as_i64)
        .map(|cp| Eval::Cp(cp as i32))
}

/// Body of /game/export/{id}?evals=true as JSON. `None` when the game has no
/// server analysis. Entry `i` describes the position after move `i + 1`.
pub fn parse_export_analysis(body: &str) -> Option<Vec<PlyEval>> {
    let v: Value = serde_json::from_str(body).ok()?;
    let list = v.get("analysis")?.as_array()?;
    Some(
        list.iter()
            .map(|e| PlyEval {
                eval: eval_from(e),
                best: e.get("best").and_then(Value::as_str).map(String::from),
                judgment: e
                    .get("judgment")
                    .and_then(|j| j.get("name"))
                    .and_then(Value::as_str)
                    .map(String::from),
            })
            .collect(),
    )
}

/// Body of /api/cloud-eval. `None` when the position is not in the cloud.
pub fn parse_cloud_eval(body: &str) -> Option<PlyEval> {
    let v: Value = serde_json::from_str(body).ok()?;
    let pv = v.get("pvs")?.as_array()?.first()?;
    let best = pv
        .get("moves")
        .and_then(Value::as_str)
        .and_then(|m| m.split_whitespace().next())
        .map(String::from);
    Some(PlyEval {
        eval: eval_from(pv),
        best,
        judgment: None,
    })
}

/// Body of /api/account/playing.
pub fn parse_playing(body: &str) -> Result<Vec<PlayingGame>> {
    let v: Value = serde_json::from_str(body).context("playing list is not json")?;
    let list = v
        .get("nowPlaying")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing nowPlaying"))?;
    Ok(list
        .iter()
        .filter_map(|g| {
            Some(PlayingGame {
                id: str_of(g, "gameId"),
                color: side_from(g.get("color")?)?,
                my_turn: g.get("isMyTurn").and_then(Value::as_bool).unwrap_or(false),
                opponent: g
                    .get("opponent")
                    .map(|o| str_of(o, "username"))
                    .unwrap_or_default(),
                speed: str_of(g, "speed"),
            })
        })
        .collect())
}

/// One line for a failed request. Lichess answers some errors with a full HTML
/// page, which must never reach the transcript.
pub fn error_message(status: u16, body: &str) -> String {
    let json_error = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(String::from));
    if let Some(msg) = json_error {
        return format!("lichess {status}: {msg}");
    }
    if status == 429 {
        return format!("lichess {status}: too many requests, wait a minute and try again");
    }
    let text = body.trim();
    if text.is_empty() || text.starts_with('<') || text.len() > 200 {
        return format!("lichess {status}");
    }
    format!("lichess {status}: {text}")
}

/// Async client. Every method is safe to call from a spawned task.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    token: String,
}

impl Client {
    pub fn new(token: String) -> Result<Client> {
        let http = reqwest::Client::builder()
            .user_agent("terminal_chess/0.1")
            .build()
            .context("building http client")?;
        Ok(Client { http, token })
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http.get(format!("{BASE}{path}")).bearer_auth(&self.token)
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.http.post(format!("{BASE}{path}")).bearer_auth(&self.token)
    }

    async fn check(resp: reqwest::Response) -> Result<reqwest::Response> {
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(anyhow!(error_message(status, &body)))
    }

    pub async fn username(&self) -> Result<String> {
        let resp = Self::check(self.get("/api/account").send().await?).await?;
        let v: Value = resp.json().await?;
        Ok(str_of(&v, "username"))
    }

    pub async fn playing(&self) -> Result<Vec<PlayingGame>> {
        let resp = Self::check(self.get("/api/account/playing").send().await?).await?;
        parse_playing(&resp.text().await?)
    }

    /// Starts a game against Stockfish. Returns the new game id.
    pub async fn challenge_ai(&self, level: u8, days: Option<u32>) -> Result<String> {
        let mut form: Vec<(&str, String)> = vec![("level", level.to_string()), ("color", "random".into())];
        if let Some(d) = days {
            form.push(("days", d.to_string()));
        }
        let resp = Self::check(self.post("/api/challenge/ai").form(&form).send().await?).await?;
        let v: Value = resp.json().await?;
        let id = str_of(&v, "id");
        if id.is_empty() {
            return Err(anyhow!("challenge response had no game id"));
        }
        Ok(id)
    }

    /// Creates a public seek. For realtime seeks the request stays open while the seek
    /// is active, so run this in its own task; the event stream reports the game start.
    pub async fn seek(&self, minutes: u32, increment: u32, days: Option<u32>) -> Result<()> {
        let mut form: Vec<(&str, String)> = vec![("rated", "false".into()), ("color", "random".into())];
        match days {
            Some(d) => form.push(("days", d.to_string())),
            None => {
                form.push(("time", minutes.to_string()));
                form.push(("increment", increment.to_string()));
            }
        }
        let resp = Self::check(self.post("/api/board/seek").form(&form).send().await?).await?;
        // Drain the body so a realtime seek stays open until matched or dropped.
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            chunk?;
        }
        Ok(())
    }

    /// Server analysis for a finished game, if Lichess has analysed it.
    pub async fn export_analysis(&self, game_id: &str) -> Result<Option<Vec<PlyEval>>> {
        let resp = Self::check(
            self.get(&format!("/game/export/{game_id}?evals=true&clocks=false"))
                .header("Accept", "application/json")
                .send()
                .await?,
        )
        .await?;
        Ok(parse_export_analysis(&resp.text().await?))
    }

    /// Cloud evaluation for one position. `None` when unknown to the cloud.
    pub async fn cloud_eval(&self, fen: &str) -> Result<Option<PlyEval>> {
        let resp = self
            .get("/api/cloud-eval")
            .query(&[("fen", fen), ("multiPv", "1")])
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let resp = Self::check(resp).await?;
        Ok(parse_cloud_eval(&resp.text().await?))
    }

    /// A random puzzle that is not in `seen`. Asked without the token on purpose:
    /// with it, Lichess keeps returning the account's current puzzle until it is
    /// solved on the site, and the endpoint needs a scope the board token lacks.
    pub async fn next_puzzle(&self, difficulty: &str, seen: &[String]) -> Result<Puzzle> {
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
            let resp = self
                .http
                .get(format!("{BASE}/api/puzzle/next"))
                .query(&[("difficulty", difficulty)])
                .header("Accept", "application/json")
                .send()
                .await?;
            let resp = Self::check(resp).await?;
            let puzzle = parse_puzzle(&resp.text().await?)
                .ok_or_else(|| anyhow!("puzzle response was not understood"))?;
            if let Some(p) = pick_unseen(puzzle, seen) {
                return Ok(p);
            }
        }
        Err(anyhow!("lichess kept returning puzzles already shown, try again"))
    }

    pub async fn send_chat(&self, game_id: &str, text: &str) -> Result<()> {
        let form = [("room", "player"), ("text", text)];
        Self::check(
            self.post(&format!("/api/board/game/{game_id}/chat")).form(&form).send().await?,
        )
        .await?;
        Ok(())
    }

    pub async fn make_move(&self, game_id: &str, uci: &str) -> Result<()> {
        Self::check(self.post(&format!("/api/board/game/{game_id}/move/{uci}")).send().await?).await?;
        Ok(())
    }

    pub async fn resign(&self, game_id: &str) -> Result<()> {
        Self::check(self.post(&format!("/api/board/game/{game_id}/resign")).send().await?).await?;
        Ok(())
    }

    pub async fn offer_draw(&self, game_id: &str) -> Result<()> {
        Self::check(self.post(&format!("/api/board/game/{game_id}/draw/yes")).send().await?).await?;
        Ok(())
    }

    /// Streams account events (game start/finish) into `tx` until the connection drops.
    pub async fn stream_events(&self, tx: mpsc::UnboundedSender<Event>) -> Result<()> {
        let resp = Self::check(self.get("/api/stream/event").send().await?).await?;
        self.pump(resp, tx, parse_event_line).await
    }

    /// Streams one game's state into `tx` until the game ends or the connection drops.
    pub async fn stream_game(&self, game_id: String, tx: mpsc::UnboundedSender<Event>) -> Result<()> {
        let resp = Self::check(
            self.get(&format!("/api/board/game/stream/{game_id}")).send().await?,
        )
        .await?;
        let id = game_id.clone();
        let result = self.pump(resp, tx.clone(), move |line| parse_game_line(&id, line)).await;
        let _ = tx.send(Event::StreamEnded { game_id });
        result
    }

    async fn pump<F>(&self, resp: reqwest::Response, tx: mpsc::UnboundedSender<Event>, parse: F) -> Result<()>
    where
        F: Fn(&str) -> Option<Event>,
    {
        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = stream.next().await {
            buf.extend_from_slice(&chunk?);
            while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let text = String::from_utf8_lossy(&line);
                if let Some(ev) = parse(&text) {
                    if tx.send(ev).is_err() {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
