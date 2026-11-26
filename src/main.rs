mod cli;
mod clock;
mod model;
mod notify;
mod session;
mod stats;
mod store;
mod timer;
mod tui;

use std::{io::IsTerminal, process::ExitCode};

use anyhow::bail;
use clap::Parser;
use cli::{Cli, Cmd};

use crate::{
    clock::{Clock, SystemClock},
    store::Paths,
};

/// Nombre único del programa: rutas XDG, título y textos derivan de aquí.
/// Debe coincidir con `[[bin]] name` en `Cargo.toml` (lo comprueba un test).
pub const APP_NAME: &str = "nostray";

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let paths = Paths::from_env()?;
    let mode = load_mode(cli.command.as_ref());
    // La TUI escribe: una sola instancia a la vez. Se toma antes de cargar para que una
    // segunda instancia no llegue a tocar ningún archivo. Se libera al salir de `main`.
    let _lock = match mode {
        store::Mode::ReadWrite => Some(store::InstanceLock::acquire(&paths)?),
        store::Mode::ReadOnly => None,
    };
    let loaded = store::load_all(&paths, mode)?;

    match cli.command {
        Some(Cmd::Stats) => {
            // Avisos de carga (corrupto → .bak, valores corregidos) por stderr.
            for n in &loaded.notices {
                eprintln!("aviso: {n}");
            }
            let today = SystemClock::new().today();
            print!(
                "{}",
                stats::text_report(&loaded.history, &loaded.config, today)
            );
            Ok(ExitCode::SUCCESS)
        }
        Some(Cmd::Start { session, tag }) => {
            let category = match tag {
                None => 0,
                Some(t) => match cli::find_category(&loaded.config.categories, &t) {
                    Some(i) => i,
                    None => {
                        eprintln!(
                            "{APP_NAME}: categoría «{t}» desconocida. Válidas: {}",
                            loaded.config.categories.join(", ")
                        );
                        return Ok(ExitCode::from(2));
                    }
                },
            };
            require_tty()?;
            tui::run(paths, loaded, Some(session.into()), category)?;
            Ok(ExitCode::SUCCESS)
        }
        None => {
            require_tty()?;
            tui::run(paths, loaded, None, 0)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// `stats` solo consulta: no debe crear ni renombrar archivos del usuario.
fn load_mode(cmd: Option<&Cmd>) -> store::Mode {
    match cmd {
        Some(Cmd::Stats) => store::Mode::ReadOnly,
        _ => store::Mode::ReadWrite,
    }
}

fn require_tty() -> anyhow::Result<()> {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        bail!("{APP_NAME} necesita una terminal interactiva (usa --help o `{APP_NAME} stats`)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_name_coincide_con_binario() {
        assert_eq!(super::APP_NAME, env!("CARGO_BIN_NAME"));
    }

    #[test]
    fn stats_carga_en_solo_lectura() {
        use super::{Cmd, load_mode, store::Mode};
        assert_eq!(load_mode(Some(&Cmd::Stats)), Mode::ReadOnly);
        assert_eq!(load_mode(None), Mode::ReadWrite);
    }
}
