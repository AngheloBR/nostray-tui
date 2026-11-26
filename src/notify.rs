//! Aviso de escritorio con `notify-send`, sin shell y sin bloquear la UI.

use std::{
    ffi::OsString,
    io,
    process::{Command, Stdio},
    thread,
};

use crate::APP_NAME;

#[derive(Debug, PartialEq, Eq)]
pub enum NotifyError {
    /// `notify-send` no está instalado.
    Missing,
    Other(String),
}

pub trait Notifier {
    fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError>;
}

/// Lanza `notify-send --app-name=<APP_NAME> -- <título> <cuerpo>`.
/// Los textos van como argumentos (nunca por una shell) y `--` evita que un
/// título que empiece por `-` se lea como opción. El proceso se recoge en un
/// hilo aparte para no dejar zombis ni bloquear el bucle.
pub struct NotifySend {
    program: OsString,
}

impl NotifySend {
    pub fn new() -> Self {
        Self::with_program("notify-send")
    }

    /// Para tests: otro ejecutable en lugar de `notify-send`.
    pub fn with_program(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl Notifier for NotifySend {
    fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError> {
        let child = Command::new(&self.program)
            .arg(format!("--app-name={APP_NAME}"))
            .arg("--")
            .arg(title)
            .arg(body)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(mut c) => {
                thread::spawn(move || c.wait());
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(NotifyError::Missing),
            Err(e) => Err(NotifyError::Other(e.to_string())),
        }
    }
}

#[cfg(test)]
pub use fake::Recorder;

#[cfg(test)]
mod fake {
    use std::{cell::RefCell, rc::Rc};

    use super::{Notifier, NotifyError};

    /// Notificador de pruebas: guarda cada aviso; `missing` simula que no existe.
    #[derive(Clone, Default)]
    pub struct Recorder {
        pub sent: Rc<RefCell<Vec<(String, String)>>>,
        pub missing: bool,
    }

    impl Notifier for Recorder {
        fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError> {
            if self.missing {
                return Err(NotifyError::Missing);
            }
            self.sent
                .borrow_mut()
                .push((title.to_owned(), body.to_owned()));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, thread, time::Duration};

    use super::*;
    use crate::store::testdir::TestDir;

    #[test]
    fn pasa_argumentos_sin_shell() {
        let t = TestDir::new("notify");
        let out = t.path().join("args.txt");
        let stub = t.path().join("notify-send");
        // Escribe un argumento por línea: si hubiera shell, `$(…)` y `;` se expandirían.
        let script = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n",
            out.display()
        );
        let _ = fs::write(&stub, script);
        let _ = fs::set_permissions(&stub, fs::Permissions::from_mode(0o755));

        let n = NotifySend::with_program(&stub);
        let r = n.notify("nostray · Foco completado", "-x $(id); echo hola\nlínea 2");
        assert_eq!(r, Ok(()));
        let mut got = String::new();
        for _ in 0..50 {
            got = fs::read_to_string(&out).unwrap_or_default();
            if got.contains("línea 2") {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            got,
            "--app-name=nostray\n--\nnostray · Foco completado\n-x $(id); echo hola\nlínea 2\n"
        );
    }

    #[test]
    fn programa_inexistente_es_missing() {
        let n = NotifySend::with_program("/nonexistent/notify-send-de-prueba");
        assert_eq!(n.notify("a", "b"), Err(NotifyError::Missing));
    }
}
