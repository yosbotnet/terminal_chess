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
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let msg = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(String::from))
            .unwrap_or(body);
        Err(anyhow!("lichess {status}: {msg}"))
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
