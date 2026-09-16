use terminal_chess::game::{Game, Outcome, Side};

#[test]
fn new_game_starts_with_white_to_move() {
    let g = Game::new();
    assert_eq!(g.turn(), Side::White);
    assert_eq!(g.move_count(), 0);
}

#[test]
fn parses_space_separated_squares() {
    let g = Game::new();
    let m = g.parse_input("e2 e4").expect("valid");
    assert_eq!(m.to_uci(), "e2e4");
}

#[test]
fn parses_joined_squares() {
    let g = Game::new();
    let m = g.parse_input("e2e4").expect("valid");
    assert_eq!(m.to_uci(), "e2e4");
}

#[test]
fn parses_san() {
    let g = Game::new();
    let m = g.parse_input("Nf3").expect("valid");
    assert_eq!(m.to_uci(), "g1f3");
}

#[test]
fn rejects_illegal_move() {
    let g = Game::new();
    assert!(g.parse_input("e2 e5").is_err());
    assert!(g.parse_input("nonsense").is_err());
}

#[test]
fn applies_lichess_move_list() {
    let mut g = Game::new();
    g.set_moves("e2e4 e7e5 g1f3").expect("valid");
    assert_eq!(g.move_count(), 3);
    assert_eq!(g.turn(), Side::Black);
    assert_eq!(g.last_move().map(|m| m.to_uci()), Some("g1f3".to_string()));
}

#[test]
fn set_moves_resets_from_start() {
    let mut g = Game::new();
    g.set_moves("e2e4 e7e5").unwrap();
    g.set_moves("d2d4").unwrap();
    assert_eq!(g.move_count(), 1);
    assert_eq!(g.turn(), Side::Black);
}

#[test]
fn legal_targets_from_square() {
    let g = Game::new();
    let targets = g.legal_targets("e2".parse().unwrap());
    let mut names: Vec<String> = targets.iter().map(|s| s.to_string()).collect();
    names.sort();
    assert_eq!(names, vec!["e3", "e4"]);
    assert!(g.legal_targets("e4".parse().unwrap()).is_empty());
}

#[test]
fn san_history_is_recorded() {
    let mut g = Game::new();
    g.set_moves("e2e4 e7e5 g1f3").unwrap();
    assert_eq!(g.san_history(), vec!["e4", "e5", "Nf3"]);
}

#[test]
fn detects_check_and_mate() {
    let mut g = Game::new();
    g.set_moves("f2f3 e7e5 g2g4 d8h4").unwrap();
    assert!(g.in_check());
    assert_eq!(g.outcome(), Some(Outcome::Checkmate { winner: Side::Black }));
    let mut g2 = Game::new();
    g2.set_moves("e2e4 f7f5").unwrap();
    assert_eq!(g2.outcome(), None);
}

#[test]
fn piece_at_returns_piece_char() {
    let g = Game::new();
    assert_eq!(g.piece_char_at("e1".parse().unwrap()), Some('K'));
    assert_eq!(g.piece_char_at("e8".parse().unwrap()), Some('k'));
    assert_eq!(g.piece_char_at("e4".parse().unwrap()), None);
}

#[test]
fn promotion_defaults_to_queen_for_coordinate_input() {
    let mut g = Game::new();
    // white pawn on a7 promotes; construct via moves
    g.set_moves("a2a4 h7h5 a4a5 h5h4 a5a6 h4h3 a6b7 h3g2 b7a8q g2h1q").unwrap();
    let mut g2 = Game::new();
    g2.set_moves("a2a4 h7h5 a4a5 h5h4 a5a6 h4h3 a6b7 h3g2").unwrap();
    let m = g2.parse_input("b7 a8").expect("promotion");
    assert_eq!(m.to_uci(), "b7a8q");
    assert_eq!(g.move_count(), 10);
}

#[test]
fn position_at_replays_a_prefix_of_the_game() {
    let mut g = Game::new();
    g.set_moves("e2e4 e7e5 g1f3").unwrap();
    let start = g.position_at(0);
    assert_eq!(start.move_count(), 0);
    assert_eq!(start.piece_char_at("e2".parse().unwrap()), Some('P'));
    let one = g.position_at(1);
    assert_eq!(one.move_count(), 1);
    assert_eq!(one.turn(), Side::Black);
    assert_eq!(one.piece_char_at("e4".parse().unwrap()), Some('P'));
    let clamped = g.position_at(99);
    assert_eq!(clamped.move_count(), 3);
}

#[test]
fn fen_at_reports_the_position() {
    let mut g = Game::new();
    g.set_moves("e2e4").unwrap();
    assert!(g.fen_at(0).starts_with("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w"));
    assert!(g.fen_at(1).starts_with("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b"));
}

#[test]
fn san_of_converts_uci_without_changing_the_game() {
    let g = Game::new();
    assert_eq!(g.san_of("g1f3"), Some("Nf3".to_string()));
    assert_eq!(g.san_of("e2e5"), None);
    assert_eq!(g.move_count(), 0);
}

#[test]
fn set_san_moves_loads_a_pgn_move_list() {
    let mut g = Game::new();
    g.set_san_moves("e4 e5 Nf3 Nc6 Bb5").unwrap();
    assert_eq!(g.move_count(), 5);
    assert_eq!(g.turn(), Side::Black);
    assert_eq!(g.piece_char_at("b5".parse().unwrap()), Some('B'));
    assert!(Game::new().set_san_moves("e4 e4").is_err());
}

#[test]
fn pgn_has_headers_numbered_moves_and_result() {
    let mut g = Game::new();
    g.set_moves("f2f3 e7e5 g2g4 d8h4").unwrap();
    let pgn = g.pgn(&[("White", "me"), ("Black", "Stockfish level 3")], "0-1");
    assert!(pgn.starts_with("[White \"me\"]\n[Black \"Stockfish level 3\"]\n[Result \"0-1\"]\n\n"), "{pgn}");
    assert!(pgn.ends_with("1. f3 e5 2. g4 Qh4# 0-1\n"), "{pgn}");
}

#[test]
fn pgn_wraps_long_move_lists() {
    let mut g = Game::new();
    g.set_san_moves("e4 e5 Nf3 Nc6 Bb5 a6 Ba4 Nf6 O-O Be7 Re1 b5 Bb3 d6 c3 O-O h3 Nb8 d4 Nbd7 c4 c6 cxb5 axb5 Nc3 Bb7").unwrap();
    let pgn = g.pgn(&[], "*");
    assert!(pgn.lines().all(|l| l.len() <= 80), "{pgn}");
    assert!(pgn.contains("13. Nc3 Bb7 *"), "{pgn}");
}
