use terminal_chess::lichess::{parse_event_line, parse_game_line, parse_playing, Event, Side};

#[test]
fn parses_game_start_event() {
    let line = r#"{"type":"gameStart","game":{"gameId":"abc123","fullId":"abc123xyz","color":"black","fen":"x","isMyTurn":false,"opponent":{"id":"bot","username":"BOT maia1"}}}"#;
    assert_eq!(
        parse_event_line(line),
        Some(Event::GameStart { game_id: "abc123".into(), color: Side::Black })
    );
}

#[test]
fn parses_game_finish_event() {
    let line = r#"{"type":"gameFinish","game":{"gameId":"abc123","color":"white"}}"#;
    assert_eq!(parse_event_line(line), Some(Event::GameFinish { game_id: "abc123".into() }));
}

#[test]
fn ignores_blank_and_unknown_events() {
    assert_eq!(parse_event_line(""), None);
    assert_eq!(parse_event_line(r#"{"type":"challenge","challenge":{}}"#), None);
    assert_eq!(parse_event_line("not json"), None);
}

#[test]
fn parses_game_full() {
    let line = r#"{"type":"gameFull","id":"abc123","rated":false,"speed":"correspondence","white":{"id":"me","name":"me","rating":1500},"black":{"aiLevel":3},"initialFen":"startpos","state":{"type":"gameState","moves":"e2e4 e7e5","wtime":1000,"btime":2000,"winc":0,"binc":0,"status":"started"}}"#;
    assert_eq!(
        parse_game_line("abc123", line),
        Some(Event::GameFull {
            game_id: "abc123".into(),
            white: "me".into(),
            black: "Stockfish level 3".into(),
            moves: "e2e4 e7e5".into(),
            status: "started".into(),
            wtime: 1000,
            btime: 2000,
        })
    );
}

#[test]
fn parses_game_state() {
    let line = r#"{"type":"gameState","moves":"e2e4 e7e5 g1f3","wtime":900,"btime":800,"winc":0,"binc":0,"status":"mate","winner":"white","wdraw":false,"bdraw":true}"#;
    assert_eq!(
        parse_game_line("abc123", line),
        Some(Event::GameState {
            game_id: "abc123".into(),
            moves: "e2e4 e7e5 g1f3".into(),
            status: "mate".into(),
            winner: Some(Side::White),
            wtime: 900,
            btime: 800,
            draw_offer: Some(Side::Black),
        })
    );
}

#[test]
fn parses_chat_line() {
    let line = r#"{"type":"chatLine","username":"lichess","text":"Takeback sent","room":"player"}"#;
    assert_eq!(
        parse_game_line("g1", line),
        Some(Event::Chat { game_id: "g1".into(), username: "lichess".into(), text: "Takeback sent".into() })
    );
}

#[test]
fn parses_now_playing_list() {
    let body = r#"{"nowPlaying":[{"gameId":"g1","fullId":"g1full","color":"white","isMyTurn":true,"lastMove":"e2e4","speed":"correspondence","opponent":{"id":"x","username":"Someone","rating":1400}},{"gameId":"g2","fullId":"g2full","color":"black","isMyTurn":false,"lastMove":"","speed":"blitz","opponent":{"ai":2,"username":"AI level 2"}}]}"#;
    let games = parse_playing(body).unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].id, "g1");
    assert_eq!(games[0].color, Side::White);
    assert!(games[0].my_turn);
    assert_eq!(games[0].opponent, "Someone");
    assert_eq!(games[0].speed, "correspondence");
    assert_eq!(games[1].opponent, "AI level 2");
    assert!(!games[1].my_turn);
}

use terminal_chess::lichess::{parse_cloud_eval, parse_export_analysis, Eval};

#[test]
fn parses_export_with_lichess_analysis() {
    let body = r#"{"id":"abc","moves":"e4 e5 Qh5","analysis":[{"eval":30},{"eval":25},{"eval":-40,"best":"g1f3","variation":"Nf3 Nc6","judgment":{"name":"Mistake","comment":"Mistake. Nf3 was best."}}]}"#;
    let evals = parse_export_analysis(body).expect("analysis present");
    assert_eq!(evals.len(), 3);
    assert_eq!(evals[0].eval, Some(Eval::Cp(30)));
    assert_eq!(evals[2].eval, Some(Eval::Cp(-40)));
    assert_eq!(evals[2].best.as_deref(), Some("g1f3"));
    assert_eq!(evals[2].judgment.as_deref(), Some("Mistake"));
    assert!(evals[0].judgment.is_none());
}

#[test]
fn export_without_analysis_is_none() {
    assert!(parse_export_analysis(r#"{"id":"abc","moves":"e4 e5"}"#).is_none());
    assert!(parse_export_analysis("garbage").is_none());
}

#[test]
fn export_mate_scores() {
    let body = r#"{"analysis":[{"mate":3},{"mate":-2}]}"#;
    let evals = parse_export_analysis(body).unwrap();
    assert_eq!(evals[0].eval, Some(Eval::Mate(3)));
    assert_eq!(evals[1].eval, Some(Eval::Mate(-2)));
}

#[test]
fn parses_cloud_eval() {
    let body = r#"{"fen":"x","knodes":100,"depth":30,"pvs":[{"moves":"e2e4 c7c5","cp":24},{"moves":"d2d4","cp":20}]}"#;
    let e = parse_cloud_eval(body).unwrap();
    assert_eq!(e.eval, Some(Eval::Cp(24)));
    assert_eq!(e.best.as_deref(), Some("e2e4"));
    let mate = parse_cloud_eval(r#"{"pvs":[{"moves":"d8h4","mate":-1}]}"#).unwrap();
    assert_eq!(mate.eval, Some(Eval::Mate(-1)));
    assert!(parse_cloud_eval(r#"{"error":"Not found"}"#).is_none());
}

#[test]
fn eval_formats_from_white_perspective() {
    assert_eq!(Eval::Cp(80).to_string(), "+0.8");
    assert_eq!(Eval::Cp(-125).to_string(), "-1.2");
    assert_eq!(Eval::Cp(0).to_string(), "0.0");
    assert_eq!(Eval::Mate(3).to_string(), "#3");
    assert_eq!(Eval::Mate(-2).to_string(), "#-2");
}
