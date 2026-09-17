use std::io::{self, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{DisableFocusChange, EnableFocusChange, Event as TermEvent, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::style::Print;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use terminal_chess::app::{Action, App};
use terminal_chess::config::Config;
use terminal_chess::lichess::{Client, Event, PlyEval};
use terminal_chess::theme::ThemeKind;
use terminal_chess::ui;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::load()?;
    let Some(token) = cfg.token.clone() else {
        print_setup_help();
        std::process::exit(2);
    };
    let client = Client::new(token)?;
    let username = match client.username().await {
        Ok(u) => u,
        Err(e) => {
            eprintln!("could not reach lichess: {e}");
            eprintln!("check the token (needs board:play scope) and your network.");
            std::process::exit(1);
        }
    };
    let theme = ThemeKind::from_name(&cfg.theme).unwrap_or(ThemeKind::Claude);
    let mut app = App::new(theme, cfg.camouflage, username);
    app.set_puzzle_difficulty(&cfg.puzzle);

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, app, client, cfg.notify.clone()).await;
    restore_terminal(&mut terminal)?;
    result
}

fn print_setup_help() {
    let path = Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<config dir>/terminal_chess/config.toml".into());
    println!("No Lichess token found.");
    println!();
    println!("1. Create a personal API token at https://lichess.org/account/oauth/token");
    println!("   with the 'Play games with the board API' (board:play) scope.");
    println!("2. Either set the LICHESS_TOKEN environment variable, or write:");
    println!();
    println!("   token = \"lip_xxxxxxxxxxxx\"");
    println!("   theme = \"claude\"      # or \"codex\"");
    println!("   camouflage = false");
    println!();
    println!("   to {path}");
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableFocusChange).context("entering alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).context("creating terminal")?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableFocusChange, LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    io::stdout().flush()?;
    Ok(())
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
    client: Client,
    notify: String,
) -> Result<()> {
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<Event>();
    let (act_tx, act_rx) = mpsc::unbounded_channel::<Action>();

    spawn_event_stream(client.clone(), ev_tx.clone());
    let worker = tokio::spawn(worker(client.clone(), act_rx, ev_tx.clone()));
    let _ = act_tx.send(Action::ListGames);

    let mut keys = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;
        tokio::select! {
            maybe = keys.next() => {
                match maybe {
                    Some(Ok(TermEvent::Key(k))) if k.kind != KeyEventKind::Release => app.handle_key(k),
                    Some(Ok(TermEvent::FocusGained)) => app.set_focused(true),
                    Some(Ok(TermEvent::FocusLost)) => app.set_focused(false),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e.into()),
                    None => break,
                }
            }
            Some(ev) = ev_rx.recv() => app.handle_event(ev),
            _ = tick.tick() => {}
        }
        for a in app.take_actions() {
            match a {
                Action::Notify { title, body } => notify_user(terminal, &notify, &title, &body),
                other => {
                    let _ = act_tx.send(other);
                }
            }
        }
        if app.should_quit() {
            break;
        }
    }
    worker.abort();
    Ok(())
}

/// Keeps the account event stream alive, reconnecting with a backoff.
fn spawn_event_stream(client: Client, tx: mpsc::UnboundedSender<Event>) {
    tokio::spawn(async move {
        let mut delay = 2u64;
        loop {
            match client.stream_events(tx.clone()).await {
                Ok(()) => delay = 2,
                Err(e) => {
                    let _ = tx.send(Event::Error(format!("event stream: {e}")));
                    delay = (delay * 2).min(60);
                }
            }
            if tx.is_closed() {
                return;
            }
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }
    });
}

/// Executes UI actions against Lichess. One game stream at a time.
async fn worker(client: Client, mut rx: mpsc::UnboundedReceiver<Action>, tx: mpsc::UnboundedSender<Event>) {
    let mut game_task: Option<(String, JoinHandle<()>)> = None;
    let mut seek_task: Option<JoinHandle<()>> = None;
    let mut analysis_task: Option<JoinHandle<()>> = None;

    while let Some(action) = rx.recv().await {
        match action {
            Action::OpenGame(id) => {
                if let Some((cur, _)) = &game_task {
                    if *cur == id {
                        continue;
                    }
                }
                if let Some((_, h)) = game_task.take() {
                    h.abort();
                }
                let c = client.clone();
                let t = tx.clone();
                let gid = id.clone();
                let handle = tokio::spawn(async move {
                    if let Err(e) = c.stream_game(gid, t.clone()).await {
                        let _ = t.send(Event::Error(format!("game stream: {e}")));
                    }
                });
                game_task = Some((id, handle));
            }
            Action::Move { game_id, uci } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = c.make_move(&game_id, &uci).await {
                        let _ = t.send(Event::Error(format!("move rejected: {e}")));
                    }
                });
            }
            Action::NewAi { level } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    match c.challenge_ai(level, None).await {
                        Ok(id) => {
                            let _ = t.send(Event::GameStart { game_id: id, color: terminal_chess::game::Side::White });
                        }
                        Err(e) => {
                            let _ = t.send(Event::Error(format!("could not start game: {e}")));
                        }
                    }
                });
            }
            Action::Seek { minutes, increment, days } => {
                if let Some(h) = seek_task.take() {
                    h.abort();
                }
                let c = client.clone();
                let t = tx.clone();
                seek_task = Some(tokio::spawn(async move {
                    match c.seek(minutes, increment, days).await {
                        Ok(()) if days.is_some() => {
                            let _ = t.send(Event::Info("Seek posted. The game opens when someone accepts.".into()));
                        }
                        Ok(()) => {}
                        Err(e) => {
                            let _ = t.send(Event::Error(format!("seek failed: {e}")));
                        }
                    }
                }));
            }
            Action::ListGames => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    match c.playing().await {
                        Ok(list) => {
                            let _ = t.send(Event::Playing(list));
                        }
                        Err(e) => {
                            let _ = t.send(Event::Error(format!("could not list games: {e}")));
                        }
                    }
                });
            }
            Action::Resign(id) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = c.resign(&id).await {
                        let _ = t.send(Event::Error(format!("resign failed: {e}")));
                    }
                });
            }
            Action::Draw(id) => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = c.offer_draw(&id).await {
                        let _ = t.send(Event::Error(format!("draw failed: {e}")));
                    }
                });
            }
            Action::FetchAnalysis { game_id, fens } => {
                if let Some(h) = analysis_task.take() {
                    h.abort();
                }
                let c = client.clone();
                let t = tx.clone();
                analysis_task = Some(tokio::spawn(async move {
                    match fetch_analysis(&c, &game_id, &fens).await {
                        Ok((evals, from_lichess)) => {
                            let _ = t.send(Event::Analysis { game_id, evals, from_lichess });
                        }
                        Err(e) => {
                            let _ = t.send(Event::Error(format!("analysis failed: {e}")));
                        }
                    }
                }));
            }
            Action::OpenBrowser(url) => {
                if let Err(e) = open_browser(&url) {
                    let _ = tx.send(Event::Error(format!("could not open browser: {e}")));
                }
            }
            Action::SendChat { game_id, text } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = c.send_chat(&game_id, &text).await {
                        let _ = t.send(Event::Error(format!("chat failed: {e}")));
                    }
                });
            }
            Action::WriteFile { path, contents } => {
                let result = std::path::Path::new(&path)
                    .parent()
                    .map(std::fs::create_dir_all)
                    .unwrap_or(Ok(()))
                    .and_then(|_| std::fs::write(&path, contents));
                let _ = match result {
                    Ok(()) => tx.send(Event::Info(format!("Saved to {path}"))),
                    Err(e) => tx.send(Event::Error(format!("could not write {path}: {e}"))),
                };
            }
            Action::FetchPuzzle { difficulty, seen } => {
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    match c.next_puzzle(&difficulty, &seen).await {
                        Ok(p) => {
                            let _ = t.send(Event::Puzzle(p));
                        }
                        Err(e) => {
                            let _ = t.send(Event::Error(format!("could not fetch a puzzle: {e}")));
                        }
                    }
                });
            }
            // Handled in the UI loop; it never reaches the worker.
            Action::Notify { .. } => {}
        }
    }
}

/// Terminal bell (Windows Terminal flashes the taskbar) and, if configured, a toast
/// that looks like a build notification.
fn notify_user(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, mode: &str, title: &str, body: &str) {
    if mode == "off" {
        return;
    }
    let _ = execute!(terminal.backend_mut(), Print("\x07"));
    if mode != "toast" {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
             $n = $t.GetElementsByTagName('text'); $n.Item(0).AppendChild($t.CreateTextNode('{}')) | Out-Null; \
             $n.Item(1).AppendChild($t.CreateTextNode('{}')) | Out-Null; \
             $toast = [Windows.UI.Notifications.ToastNotification]::new($t); \
             [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe').Show($toast)",
            title.replace('\'', ""),
            body.replace('\'', "")
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!("display notification \"{}\" with title \"{}\"", body.replace('"', ""), title.replace('"', ""));
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::process::Command::new("notify-send").args([title, body]).spawn();
    }
}

/// Server analysis when Lichess has it, otherwise cloud evals position by position.
/// Returns one entry per fen and whether the data came with Lichess judgments.
async fn fetch_analysis(client: &Client, game_id: &str, fens: &[String]) -> Result<(Vec<PlyEval>, bool)> {
    if let Some(list) = client.export_analysis(game_id).await? {
        // Lichess entries describe the position after each move; index 0 is the start.
        let mut evals = vec![PlyEval::default()];
        evals.extend(list);
        evals.resize(fens.len().max(evals.len()), PlyEval::default());
        return Ok((evals, true));
    }
    let mut evals = Vec::with_capacity(fens.len());
    for fen in fens {
        let e = client.cloud_eval(fen).await.unwrap_or(None).unwrap_or_default();
        evals.push(e);
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    Ok((evals, false))
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd").args(["/C", "start", "", url]).status()?;
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status()?;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).status()?;
    if !status.success() {
        anyhow::bail!("launcher exited with {status}");
    }
    Ok(())
}
