//! Persistencia: rutas XDG, carga tolerante a corrupción y escritura atómica.
//!
//! - Datos: `$XDG_DATA_HOME/<APP_NAME>/{history,tasks}.json`
//! - Config: `$XDG_CONFIG_HOME/<APP_NAME>/config.toml`
//! - Un archivo ilegible NO se borra: se renombra a `.bak` (o `.bak.N`) y se arranca limpio.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::{
    APP_NAME,
    model::{Config, FORMAT_VERSION, History, TaskList, default_config_toml},
};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("no se pudo determinar el directorio personal (HOME)")]
    NoHome,
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path}: no se pudo serializar: {msg}")]
    Encode { path: PathBuf, msg: String },
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_owned(),
        source,
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    data: PathBuf,
    config: PathBuf,
}

impl Paths {
    /// Rutas XDG reales (respeta `XDG_DATA_HOME` y `XDG_CONFIG_HOME`).
    pub fn from_env() -> Result<Self, StoreError> {
        let p = ProjectDirs::from("", "", APP_NAME).ok_or(StoreError::NoHome)?;
        Ok(Self::at(p.data_dir(), p.config_dir()))
    }

    pub fn at(data: impl Into<PathBuf>, config: impl Into<PathBuf>) -> Self {
        Self {
            data: data.into(),
            config: config.into(),
        }
    }

    pub fn history(&self) -> PathBuf {
        self.data.join("history.json")
    }

    pub fn tasks(&self) -> PathBuf {
        self.data.join("tasks.json")
    }

    pub fn config(&self) -> PathBuf {
        self.config.join("config.toml")
    }
}

/// Todo lo cargado al arrancar, más los avisos para mostrar en la TUI.
#[derive(Debug)]
pub struct Loaded {
    pub history: History,
    pub tasks: TaskList,
    pub config: Config,
    pub notices: Vec<String>,
}

pub fn load_all(paths: &Paths) -> Result<Loaded, StoreError> {
    let mut notices = Vec::new();
    let history = load_json::<History>(&paths.history(), |h| h.version, &mut notices)?;
    let tasks = load_json::<TaskList>(&paths.tasks(), |t| t.version, &mut notices)?;
    let config = load_config(&paths.config(), &mut notices)?;
    Ok(Loaded {
        history,
        tasks,
        config,
        notices,
    })
}

/// Lee un JSON. Ausente → valor por defecto. Ilegible o de versión desconocida → `.bak`.
pub fn load_json<T: DeserializeOwned + Default>(
    path: &Path,
    version: impl Fn(&T) -> u32,
    notices: &mut Vec<String>,
) -> Result<T, StoreError> {
    let Some(text) = read_optional(path)? else {
        return Ok(T::default());
    };
    let reason = match serde_json::from_str::<T>(&text) {
        Ok(v) if version(&v) == FORMAT_VERSION => return Ok(v),
        Ok(v) => format!("versión {} no soportada", version(&v)),
        Err(e) => e.to_string(),
    };
    let bak = backup(path)?;
    notices.push(format!(
        "{} ilegible ({reason}); guardado como {} y empiezo vacío",
        file_name(path),
        file_name(&bak)
    ));
    Ok(T::default())
}

/// Lee la configuración. Ausente → la crea con comentarios. Ilegible → `.bak`.
/// Valores fuera de rango se corrigen en memoria sin tocar el archivo.
pub fn load_config(path: &Path, notices: &mut Vec<String>) -> Result<Config, StoreError> {
    let Some(text) = read_optional(path)? else {
        write_atomic(path, default_config_toml().as_bytes())?;
        return Ok(Config::default());
    };
    let cfg = match toml::from_str::<Config>(&text) {
        Ok(c) => c,
        Err(e) => {
            let bak = backup(path)?;
            notices.push(format!(
                "{} ilegible ({}); guardado como {} y uso valores por defecto",
                file_name(path),
                e.message(),
                file_name(&bak)
            ));
            write_atomic(path, default_config_toml().as_bytes())?;
            return Ok(Config::default());
        }
    };
    let (cfg, warn) = cfg.sanitize();
    notices.extend(warn.into_iter().map(|w| format!("config.toml: {w}")));
    Ok(cfg)
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| StoreError::Encode {
        path: path.to_owned(),
        msg: e.to_string(),
    })?;
    text.push('\n');
    write_atomic(path, text.as_bytes())
}

pub fn save_config(path: &Path, cfg: &Config) -> Result<(), StoreError> {
    let text = toml::to_string(cfg).map_err(|e| StoreError::Encode {
        path: path.to_owned(),
        msg: e.to_string(),
    })?;
    write_atomic(path, text.as_bytes())
}

fn read_optional(path: &Path) -> Result<Option<String>, StoreError> {
    match fs::read(path) {
        // Bytes no UTF-8 se tratan como contenido ilegible (acaba en `.bak`).
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_err(path)(e)),
    }
}

/// Escribe en `<archivo>.tmp`, sincroniza y renombra encima: nunca deja un archivo a medias.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    use io::Write as _;

    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(io_err(dir))?;
    let tmp = with_suffix(path, ".tmp");
    let mut f = fs::File::create(&tmp).map_err(io_err(&tmp))?;
    f.write_all(bytes).map_err(io_err(&tmp))?;
    f.sync_all().map_err(io_err(&tmp))?;
    drop(f);
    fs::rename(&tmp, path).map_err(io_err(path))?;
    // Persistir también la entrada del directorio (best effort).
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Renombra `path` a `path.bak`, o `path.bak.1`, `.bak.2`… si ya existen.
fn backup(path: &Path) -> Result<PathBuf, StoreError> {
    let mut bak = with_suffix(path, ".bak");
    let mut n = 1u32;
    while bak.exists() {
        bak = with_suffix(path, &format!(".bak.{n}"));
        n += 1;
    }
    fs::rename(path, &bak).map_err(io_err(path))?;
    Ok(bak)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
pub mod testdir {
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicU32, Ordering},
    };

    static N: AtomicU32 = AtomicU32::new(0);

    /// Directorio temporal dentro de `target/` (en el proyecto); se borra al soltarse.
    pub struct TestDir(PathBuf);

    impl TestDir {
        pub fn new(name: &str) -> Self {
            let n = N.fetch_add(1, Ordering::Relaxed);
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("test-tmp")
                .join(format!("{name}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            let _ = std::fs::create_dir_all(&p);
            Self(p)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{testdir::TestDir, *};
    use crate::model::Task;

    fn paths(t: &TestDir) -> Paths {
        Paths::at(t.path().join("data"), t.path().join("config"))
    }

    fn sample_tasks() -> TaskList {
        TaskList {
            version: FORMAT_VERSION,
            next_id: 3,
            active: Some(2),
            tasks: vec![
                Task {
                    id: 1,
                    title: "Repasar vocabulario".into(),
                    estimate: 2,
                    done: 2,
                },
                Task {
                    id: 2,
                    title: "Escribir parser de ejemplo".into(),
                    estimate: 4,
                    done: 1,
                },
            ],
        }
    }

    #[test]
    fn rutas_derivan_de_app_name() {
        let p = Paths::at("/d/x", "/c/x");
        assert_eq!(p.history(), Path::new("/d/x/history.json"));
        assert_eq!(p.tasks(), Path::new("/d/x/tasks.json"));
        assert_eq!(p.config(), Path::new("/c/x/config.toml"));
        if let Ok(real) = Paths::from_env() {
            assert!(real.history().ends_with(format!("{APP_NAME}/history.json")));
            assert!(real.config().ends_with(format!("{APP_NAME}/config.toml")));
        }
    }

    #[test]
    fn arranque_vacio_crea_solo_config() {
        let t = TestDir::new("vacio");
        let p = paths(&t);
        let l = load_all(&p);
        let l = l.as_ref().ok();
        assert_eq!(l.map(|l| l.history.cycles.len()), Some(0));
        assert_eq!(l.map(|l| l.tasks.tasks.len()), Some(0));
        assert_eq!(l.map(|l| l.notices.len()), Some(0));
        assert!(
            p.config().exists(),
            "config.toml creado con valores por defecto"
        );
        assert!(!p.history().exists() && !p.tasks().exists());
        let text = fs::read_to_string(p.config()).unwrap_or_default();
        assert!(text.starts_with(&format!("# {APP_NAME}")));
    }

    #[test]
    fn ida_y_vuelta_json_y_toml() {
        let t = TestDir::new("ida");
        let p = paths(&t);
        let tasks = sample_tasks();
        assert!(save_json(&p.tasks(), &tasks).is_ok());
        let cfg = Config {
            daily_goal: 6,
            ..Config::default()
        };
        assert!(save_config(&p.config(), &cfg).is_ok());

        let l = load_all(&p);
        let l = l.as_ref().ok();
        assert_eq!(l.map(|l| &l.tasks), Some(&tasks));
        assert_eq!(l.map(|l| &l.config), Some(&cfg));
        assert_eq!(l.map(|l| l.notices.len()), Some(0));
    }

    #[test]
    fn json_corrupto_va_a_bak_sin_borrar() {
        let t = TestDir::new("corrupto");
        let p = paths(&t);
        let _ = fs::create_dir_all(t.path().join("data"));
        let _ = fs::write(p.history(), "{ esto no es json");
        let mut notes = Vec::new();
        let h = load_json::<History>(&p.history(), |h| h.version, &mut notes);
        assert_eq!(h.ok(), Some(History::default()));
        assert!(!p.history().exists());
        let bak = with_suffix(&p.history(), ".bak");
        assert_eq!(
            fs::read_to_string(&bak).ok().as_deref(),
            Some("{ esto no es json")
        );
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("history.json.bak"), "{notes:?}");

        // Segundo corrupto: no pisa el primer .bak.
        let _ = fs::write(p.history(), "[]");
        let _ = load_json::<History>(&p.history(), |h| h.version, &mut notes);
        assert!(bak.exists());
        assert!(with_suffix(&p.history(), ".bak.1").exists());
    }

    #[test]
    fn version_desconocida_va_a_bak() {
        let t = TestDir::new("version");
        let p = paths(&t);
        let _ = fs::create_dir_all(t.path().join("data"));
        let _ = fs::write(
            p.tasks(),
            r#"{"version":2,"next_id":1,"active":null,"tasks":[]}"#,
        );
        let mut notes = Vec::new();
        let r = load_json::<TaskList>(&p.tasks(), |t| t.version, &mut notes);
        assert_eq!(r.ok(), Some(TaskList::default()));
        assert!(with_suffix(&p.tasks(), ".bak").exists());
        assert!(notes[0].contains("versión 2"), "{notes:?}");
    }

    #[test]
    fn toml_corrupto_va_a_bak_y_se_regenera() {
        let t = TestDir::new("toml");
        let p = paths(&t);
        let _ = fs::create_dir_all(t.path().join("config"));
        let _ = fs::write(p.config(), "daily_goal = = 3");
        let mut notes = Vec::new();
        let c = load_config(&p.config(), &mut notes);
        assert_eq!(c.ok(), Some(Config::default()));
        assert_eq!(
            fs::read_to_string(with_suffix(&p.config(), ".bak"))
                .ok()
                .as_deref(),
            Some("daily_goal = = 3")
        );
        assert!(p.config().exists());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn toml_fuera_de_rango_corrige_sin_renombrar() {
        let t = TestDir::new("rango");
        let p = paths(&t);
        let _ = fs::create_dir_all(t.path().join("config"));
        let original = "daily_goal = 99\n";
        let _ = fs::write(p.config(), original);
        let mut notes = Vec::new();
        let c = load_config(&p.config(), &mut notes);
        assert_eq!(c.ok().map(|c| c.daily_goal), Some(8));
        assert_eq!(
            fs::read_to_string(p.config()).ok().as_deref(),
            Some(original)
        );
        assert!(!with_suffix(&p.config(), ".bak").exists());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn escritura_atomica_reemplaza_y_no_deja_tmp() {
        let t = TestDir::new("atomica");
        let f = t.path().join("a/b/c/x.json");
        assert!(write_atomic(&f, b"uno").is_ok(), "crea directorios");
        assert!(write_atomic(&f, b"dos").is_ok());
        assert_eq!(fs::read_to_string(&f).ok().as_deref(), Some("dos"));
        assert!(!with_suffix(&f, ".tmp").exists());
        let entries = fs::read_dir(t.path().join("a/b/c")).map(|d| d.count()).ok();
        assert_eq!(entries, Some(1));
    }

    #[test]
    fn tmp_huerfano_no_afecta_al_original() {
        // Simula un corte entre write y rename: queda un .tmp a medias.
        let t = TestDir::new("huerfano");
        let f = t.path().join("tasks.json");
        assert!(save_json(&f, &sample_tasks()).is_ok());
        let _ = fs::write(with_suffix(&f, ".tmp"), "{ a medias");
        let mut notes = Vec::new();
        let r = load_json::<TaskList>(&f, |t| t.version, &mut notes);
        assert_eq!(r.ok(), Some(sample_tasks()));
        assert!(notes.is_empty());
        assert!(
            save_json(&f, &TaskList::default()).is_ok(),
            "sobrescribe el .tmp"
        );
        assert!(!with_suffix(&f, ".tmp").exists());
    }

    #[test]
    fn error_de_lectura_no_se_confunde_con_corrupto() {
        // Un directorio en lugar de archivo: error de E/S, no .bak.
        let t = TestDir::new("dir");
        let f = t.path().join("history.json");
        let _ = fs::create_dir_all(&f);
        let mut notes = Vec::new();
        let r = load_json::<History>(&f, |h| h.version, &mut notes);
        assert!(r.is_err());
        assert!(!with_suffix(&f, ".bak").exists());
    }
}
