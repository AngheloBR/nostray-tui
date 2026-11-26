use clap::{Parser, Subcommand, ValueEnum};

use crate::{APP_NAME, session::SessionKind};

#[derive(Parser, Debug)]
#[command(
    name = APP_NAME,
    version,
    about = "Temporizador Pomodoro en terminal para mantener el enfoque",
    after_help = "Sin argumentos abre la interfaz interactiva (TUI) con el selector de sesión."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Cmd {
    /// Abre la TUI directamente con una sesión y una categoría
    Start {
        /// Sesión tipo
        #[arg(long, value_enum, default_value_t = SessionArg::Clasico)]
        session: SessionArg,
        /// Categoría de enfoque (una de las de config.toml; sin distinguir mayúsculas)
        #[arg(long, value_name = "NOMBRE")]
        tag: Option<String>,
    },
    /// Imprime un resumen de texto (hoy, 7 días, racha) y sale
    Stats,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionArg {
    /// 25/5, descanso largo de 15 tras 4 ciclos
    Clasico,
    /// 50/10
    Deep,
    /// 90/20
    Flow,
    /// Valores de [custom] en config.toml
    Personalizado,
}

impl From<SessionArg> for SessionKind {
    fn from(s: SessionArg) -> Self {
        match s {
            SessionArg::Clasico => Self::Clasico,
            SessionArg::Deep => Self::Deep,
            SessionArg::Flow => Self::Flow,
            SessionArg::Personalizado => Self::Personalizado,
        }
    }
}

/// Índice de `tag` en `categories`, sin distinguir mayúsculas.
pub fn find_category(categories: &[String], tag: &str) -> Option<usize> {
    let t = tag.trim().to_lowercase();
    categories.iter().position(|c| c.to_lowercase() == t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once(APP_NAME).chain(args.iter().copied()))
    }

    #[test]
    fn sin_argumentos_abre_tui() {
        assert_eq!(parse(&[]).ok().map(|c| c.command), Some(None));
    }

    #[test]
    fn start_con_sesion_y_tag() {
        let c = parse(&["start", "--session", "deep", "--tag", "Inglés"]).ok();
        assert_eq!(
            c.map(|c| c.command),
            Some(Some(Cmd::Start {
                session: SessionArg::Deep,
                tag: Some("Inglés".into())
            }))
        );
        let c = parse(&["start"]).ok().and_then(|c| c.command);
        assert_eq!(
            c,
            Some(Cmd::Start {
                session: SessionArg::Clasico,
                tag: None
            })
        );
    }

    #[test]
    fn sesion_invalida_es_error() {
        assert!(parse(&["start", "--session", "eterna"]).is_err());
        assert!(parse(&["stats", "extra"]).is_err());
    }

    #[test]
    fn stats() {
        assert_eq!(
            parse(&["stats"]).ok().and_then(|c| c.command),
            Some(Cmd::Stats)
        );
    }

    #[test]
    fn busca_categoria_sin_mayusculas() {
        let cats = vec!["Programar".to_owned(), "Inglés".to_owned()];
        assert_eq!(find_category(&cats, "inglés"), Some(1));
        assert_eq!(find_category(&cats, " PROGRAMAR "), Some(0));
        assert_eq!(find_category(&cats, "Leer"), None);
    }
}
