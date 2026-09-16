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
