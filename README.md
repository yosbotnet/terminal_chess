# terminal_chess

Play chess on Lichess from a terminal that looks like a Claude Code or Codex
session. Built for a second monitor at work.

## Setup

1. Create a personal API token at https://lichess.org/account/oauth/token
   with the "Play games with the board API" (board:play) scope.
2. Put it in the environment as `LICHESS_TOKEN`, or write a config file at
   `%APPDATA%\terminal_chess\config.toml` (Linux/macOS: `~/.config/terminal_chess/config.toml`):

   ```toml
   token = "lip_xxxxxxxxxxxx"
   theme = "claude"      # or "codex"
   camouflage = false    # start with the board hidden as file output
   ```

3. Build and run:

   ```
   cargo build --release
   target/release/tc
   ```

Rename the binary to whatever you want your task manager to show.

## Playing

The board sits above the prompt. Your side is at the bottom.

| Key | Action |
| --- | --- |
| arrows or `hjkl` | move the cursor (`hjkl` only while the prompt is empty) |
| Enter or Space | select a piece, then move it (legal squares show as dots) |
| Esc | cancel selection, clear the prompt |
| Tab | toggle camouflage: the board becomes a numbered file read with letters |
| Ctrl+T | switch theme (Claude / Codex) |
| Ctrl+C | quit |

You can also type moves into the prompt: `e2 e4`, `e2e4`, or `Nf3`.

Commands:

| Command | Action |
| --- | --- |
| `/games` | list games in progress on your account |
| `/game N` or `/game <id>` | open a game from that list |
| `/new ai 3` | new game against Stockfish level 1-8 |
| `/seek 15+10` | seek a human, realtime |
| `/seek corr 2` | seek a human, correspondence with 2 days per move (default for bare `/seek`) |
| `/resign`, `/draw` | resign or offer/accept a draw |
| `/flip`, `/hide`, `/theme`, `/help`, `/quit` | as named |

Every move you make shows up in the transcript as a fake `Edit(...)` tool
call; the opponent's moves show as `Read(...)`. The move itself is in the dim
detail line, in parentheses.

## Development

```
cargo test
cargo run --example preview   # dumps both themes as text
```

Layout: `game.rs` (rules, on shakmaty), `commands.rs` (prompt parser),
`lichess.rs` (Board API), `app.rs` (state and keys, no I/O), `render/`
(pretty and camouflage boards), `ui.rs` (screen layout), `main.rs` (runtime).
