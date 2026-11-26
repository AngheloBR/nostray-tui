//! Arranque y restauración de la terminal y bucle de eventos.
//!
//! - Salida normal: `ratatui::restore()` tras el bucle.
//! - Panic: `ratatui::init()` instala un hook que restaura antes de imprimir.
//! - Ctrl+C: en raw mode llega como tecla y sale del bucle.
//! - SIGTERM / SIGHUP / SIGINT: activan `stop` y el bucle sale limpio.

mod app;
mod digits;
mod layout;
mod panels;
#[cfg(test)]
mod snapshot;
mod theme;
mod ui;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyEventKind},
};

use crate::{
    clock::SystemClock,
    notify::NotifySend,
    session::SessionKind,
    store::{Loaded, Paths},
};
use app::App;

const FRAME: Duration = Duration::from_millis(100);

/// `start`: sesión ya elegida (sin selector); `category`: índice en la config.
pub fn run(
    paths: Paths,
    loaded: Loaded,
    start: Option<SessionKind>,
    category: usize,
) -> anyhow::Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
    ] {
        signal_hook::flag::register(sig, Arc::clone(&stop))?;
    }

    let mut app = App::new(
        Box::new(SystemClock::new()),
        Box::new(NotifySend::new()),
        Some(paths),
        loaded,
        start,
        category,
    );
    let mut terminal = ratatui::init();
    let res = event_loop(&mut terminal, &mut app, &stop);
    ratatui::restore();
    res
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    stop: &AtomicBool,
) -> anyhow::Result<()> {
    let mut dirty = true;
    while !app.quit && !stop.load(Ordering::Relaxed) {
        dirty |= app.update();
        if dirty {
            terminal.draw(|f| ui::draw(f, app))?;
            dirty = false;
        }
        if event::poll(FRAME)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    app.on_key(k);
                    dirty = true;
                }
                Event::Resize(..) => dirty = true,
                _ => {}
            }
        }
    }
    Ok(())
}
