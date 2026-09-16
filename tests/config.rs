use terminal_chess::config::Config;

#[test]
fn parses_full_config() {
    let c = Config::from_toml(
        r#"
token = "lip_abc"
theme = "codex"
camouflage = true
"#,
    )
    .unwrap();
    assert_eq!(c.token.as_deref(), Some("lip_abc"));
    assert_eq!(c.theme, "codex");
    assert!(c.camouflage);
}

#[test]
fn defaults_when_fields_missing() {
    let c = Config::from_toml("").unwrap();
    assert_eq!(c.token, None);
    assert_eq!(c.theme, "claude");
    assert!(!c.camouflage);
}

#[test]
fn env_token_overrides_file_token() {
    let mut c = Config::from_toml(r#"token = "file""#).unwrap();
    c.apply_env(Some("env".to_string()));
    assert_eq!(c.token.as_deref(), Some("env"));
    let mut c2 = Config::from_toml(r#"token = "file""#).unwrap();
    c2.apply_env(None);
    assert_eq!(c2.token.as_deref(), Some("file"));
}
