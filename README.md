# nostray

A Pomodoro timer for the terminal, built to keep you focused on any activity:
coding, studying, practising a language, reading… A TUI with a big clock,
preset sessions, categories, a daily goal, time distribution, streaks and a
GitHub-style activity heatmap, plus a minimal CLI.

The interface is in Spanish. Full documentation: [README.es.md](README.es.md).

```
 nostray v0.1.0                        Clásico · Programar · ciclo 1/4  ? ayuda
╭ Timer ───────────────────────────────────────────────────────────────────────╮
│                        ██████ ██████    ██████ ██████                        │
│                            ██ ██     ██ ██  ██ ██  ██                        │
│                        ██████ ██████    ██  ██ ██  ██                        │
│                        ██         ██ ██ ██  ██ ██  ██                        │
│                        ██████ ██████    ██████ ██████                        │
│                                                                              │
│             ● ENFOQUE   Programar · Escribir parser de ejemplo 1/4           │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━   0%  │
╰──────────────────────────────────────────────────────────────────────────────╯
```

(Text capture rendered by the app's own drawing code with sample data; the real
terminal shows colours.)

## Install

Requires Rust and Cargo (`rust-version = "1.88"`; developed and tested with
1.100):

```sh
cargo install --path .
```

Desktop notifications use `notify-send` (usually the `libnotify` package). It is
optional: without it, nostray shows a notice in the TUI and keeps working.

## Usage

```sh
nostray                                   # TUI with the session picker
nostray start --session deep --tag Inglés # open directly with a session/category
nostray stats                             # plain-text summary (today, 7 days, streak)
nostray --help | --version
```

Sessions: `clasico` (25/5, 15-minute long break after 4), `deep` (50/10),
`flow` (90/20), `personalizado` (custom, editable in the TUI or in `config.toml`).

## Keys

`space` start/pause · `s` skip phase · `r` restart phase · `m` session ·
`c` category · `Tab`/`Shift+Tab` switch panel · `?` help · `q` quit (asks if a
cycle is running) · `Ctrl+C` quit now. Panel keys (tasks, log, distribution)
are listed in the in-app help (`?`).

The terminal is restored on normal exit, `Ctrl+C`, SIGTERM/SIGHUP/SIGINT and
panics.

## Data

- `$XDG_DATA_HOME/nostray/history.json` and `tasks.json` (default `~/.local/share/nostray/`)
- `$XDG_CONFIG_HOME/nostray/config.toml` (default `~/.config/nostray/`)

Writes are atomic. An unreadable file is never deleted: it is renamed to
`.bak` and nostray starts fresh, with a notice.

To rename the program, change `APP_NAME` in `src/main.rs` and the `[[bin]]`
name in `Cargo.toml` (a test checks they match).

## v1 limitations

Phase time uses the monotonic clock, which does not advance while the machine is
suspended. No locking between concurrent instances. Leaving mid-cycle discards
that phase. Saving the config from the TUI drops the comments in `config.toml`.
Developed and tested on Linux (Fedora, X11). Out of scope: sound, git
integration, sync, network, Wayland, themes, plugins, export.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
