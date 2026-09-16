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
   notify = "toast"      # or "bell" or "off"
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
| arrows | move the cursor |
| Enter or Space | select a piece, then move it (legal squares show as dots) |
| Esc | cancel selection, clear the prompt |
| `m` | mark the cursor square: green, red, blue, yellow, then off |
| `v` | arrow: press on the start square, then again on the end square (same arrow again removes it) |
| `x` | clear all marks and arrows |
| Tab | toggle camouflage: the board becomes a numbered file read with letters |
| F12 | panic: the whole screen becomes a canned agent session that types itself out; any key returns |
| Ctrl+T | switch theme (Claude / Codex) |
| Ctrl+C | quit |

The annotation keys only fire while the prompt is empty, so typing `mv` or
`x` inside a command still works.

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
| `/review` | step through the current game with evaluations (automatic when a game ends) |
| `/analyze` | open the game on lichess.org in your browser |
| `/say hello` | send a chat message to the opponent (their messages show in the transcript) |
| `/pgn` | print the game as a fake `Write(...)` tool call so it can be copied |
| `/pgn save [path]` | write the PGN to a file, default `Documents\chess\<game id>.pgn` |
| `/puzzle` | fetch your next Lichess puzzle (or the daily one) and solve it on the board |
| `/panic` | same as F12 |
| `/flip`, `/hide`, `/theme`, `/help`, `/quit` | as named |

## Clock, notifications, puzzles

Clocks show in the board's detail line for timed games and count down locally
between server updates. Correspondence games and unlimited games show none.

When it becomes your move while the terminal window is not focused, the app
rings the terminal bell (Windows Terminal flashes the taskbar) and shows a
Windows toast titled "Build finished". Set `notify = "bell"` or `"off"` in the
config to tone that down.

Puzzles load the game so far, put you on the side to move, and check each move
against the solution. The opponent's replies play automatically. A wrong move
says "test failed" and lets you retry. Puzzles played this way do not count
toward your Lichess puzzle rating; Lichess has no API for that.

Every move you make shows up in the transcript as a fake `Edit(...)` tool
call; the opponent's moves show as `Read(...)`. The move itself is in the dim
detail line, in parentheses.

## After the game

When a game ends the app drops into review at the final position. Left and
Right step one move, Up and Down jump to the start or end, Esc returns to the
live view. The footer shows the ply and the evaluation.

Evaluations come from Lichess. If the game has server analysis (you or your
opponent requested it on lichess.org), every move gets an eval and Lichess's
own blunder / mistake / inaccuracy verdicts with the best move. Otherwise the
app asks Lichess's cloud eval for each position, which covers openings and
well-known positions, and derives verdicts from the swings it can see. The
result is printed as fake `cargo clippy` warnings:

```
warning: blunder in your move 2. g4??, best was Nh3
  --> src/board/movegen.rs:3:4
```

For a full engine analysis, `/analyze` opens the game in the browser, click
"Request a computer analysis" there, then run `/review` again.

## Development

```
cargo test
cargo run --example preview   # dumps both themes as text
```

Layout: `game.rs` (rules, on shakmaty), `commands.rs` (prompt parser),
`lichess.rs` (Board API), `app.rs` (state and keys, no I/O), `render/`
(pretty and camouflage boards), `ui.rs` (screen layout), `main.rs` (runtime).
