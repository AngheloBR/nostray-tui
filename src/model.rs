//! Modelos persistidos: historial y tareas (JSON) y configuración (TOML).

use jiff::{Timestamp, civil::Date, tz::TimeZone};
use serde::{Deserialize, Serialize};

use crate::{
    session::{BREAK_MIN, CYCLES, Custom, FOCUS_MIN, LONG_BREAK_MIN, SessionKind},
    timer::{Phase, PhaseEnded},
};

/// Versión del formato de los archivos JSON.
pub const FORMAT_VERSION: u32 = 1;

// ── Historial ───────────────────────────────────────────────────────────────

/// Una fase cerrada (foco o descanso, completada o saltada).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleRecord {
    pub ended_at: Timestamp,
    /// Día local en el momento de cerrar: fija la racha aunque cambie la zona.
    pub local_date: Date,
    pub phase: Phase,
    pub session: String,
    pub category: Option<String>,
    pub task_id: Option<u64>,
    pub task: Option<String>,
    pub planned_secs: u64,
    pub actual_secs: u64,
    pub completed: bool,
}

impl CycleRecord {
    /// Construye el registro de una fase cerrada. Categoría y tarea solo aplican a focos.
    pub fn new(
        ended: PhaseEnded,
        at: Timestamp,
        tz: &TimeZone,
        session: SessionKind,
        category: &str,
        task: Option<&Task>,
    ) -> Self {
        let focus = ended.phase == Phase::Focus;
        Self {
            ended_at: at,
            local_date: at.to_zoned_datetime(tz.clone()).date(),
            phase: ended.phase,
            session: session.id().to_owned(),
            category: focus.then(|| category.to_owned()),
            task_id: task.filter(|_| focus).map(|t| t.id),
            task: task.filter(|_| focus).map(|t| t.title.clone()),
            planned_secs: ended.planned.as_secs(),
            actual_secs: ended.actual.as_secs(),
            completed: ended.completed,
        }
    }

    /// Pomodoro que cuenta para estadísticas.
    pub fn is_pomodoro(&self) -> bool {
        self.phase == Phase::Focus && self.completed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    pub version: u32,
    pub cycles: Vec<CycleRecord>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            cycles: Vec::new(),
        }
    }
}

// ── Tareas ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    /// Pomodoros estimados.
    pub estimate: u32,
    /// Pomodoros hechos.
    pub done: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub version: u32,
    pub next_id: u64,
    pub active: Option<u64>,
    pub tasks: Vec<Task>,
}

pub const MAX_TITLE_CHARS: usize = 60;
pub const ESTIMATE: (u32, u32) = (1, 20);

impl TaskList {
    /// Alta con título recortado; devuelve el id. `None` si el título queda vacío.
    pub fn add(&mut self, title: &str) -> Option<u64> {
        let title = clean_title(title)?;
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.tasks.push(Task {
            id,
            title,
            estimate: 1,
            done: 0,
        });
        Some(id)
    }

    pub fn rename(&mut self, id: u64, title: &str) -> bool {
        match (
            clean_title(title),
            self.tasks.iter_mut().find(|t| t.id == id),
        ) {
            (Some(title), Some(t)) => {
                t.title = title;
                true
            }
            _ => false,
        }
    }

    /// Borra la tarea; si era la activa, deja de haber activa.
    pub fn remove(&mut self, id: u64) {
        self.tasks.retain(|t| t.id != id);
        if self.active == Some(id) {
            self.active = None;
        }
    }

    /// Marca como activa, o la desmarca si ya lo era.
    pub fn toggle_active(&mut self, id: u64) {
        self.active = if self.active == Some(id) {
            None
        } else {
            Some(id)
        };
    }

    pub fn bump_estimate(&mut self, id: u64, by: i64) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            let v =
                (i64::from(t.estimate) + by).clamp(i64::from(ESTIMATE.0), i64::from(ESTIMATE.1));
            t.estimate = u32::try_from(v).unwrap_or(ESTIMATE.0);
        }
    }

    /// Suma un pomodoro hecho a la tarea activa. Devuelve si había activa.
    pub fn credit_active(&mut self) -> bool {
        let Some(id) = self.active else { return false };
        match self.tasks.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                t.done = t.done.saturating_add(1);
                true
            }
            None => false,
        }
    }
}

fn clean_title(s: &str) -> Option<String> {
    let t: String = s.trim().chars().take(MAX_TITLE_CHARS).collect();
    let t = t.trim_end().to_owned();
    (!t.is_empty()).then_some(t)
}

impl Default for TaskList {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            next_id: 1,
            active: None,
            tasks: Vec::new(),
        }
    }
}

// ── Configuración ───────────────────────────────────────────────────────────

pub const DAILY_GOAL: (u32, u32) = (1, 24);
pub const MAX_CATEGORIES: usize = 8;
pub const MAX_CATEGORY_CHARS: usize = 20;
pub const DEFAULT_CATEGORIES: [&str; 5] = ["Programar", "Estudiar", "Inglés", "Leer", "Otro"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daily_goal: u32,
    pub categories: Vec<String>,
    pub custom: Custom,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            daily_goal: 8,
            categories: DEFAULT_CATEGORIES.map(str::to_owned).to_vec(),
            custom: Custom::default(),
        }
    }
}

impl Config {
    /// Lleva cada campo inválido a su valor por defecto. Devuelve un aviso por campo.
    pub fn sanitize(mut self) -> (Self, Vec<String>) {
        let def = Self::default();
        let mut warn = Vec::new();

        if !(DAILY_GOAL.0..=DAILY_GOAL.1).contains(&self.daily_goal) {
            warn.push(format!(
                "daily_goal = {} fuera de rango ({}–{}); uso {}",
                self.daily_goal, DAILY_GOAL.0, DAILY_GOAL.1, def.daily_goal
            ));
            self.daily_goal = def.daily_goal;
        }

        let mut cats: Vec<String> = Vec::new();
        for raw in &self.categories {
            let c = raw.trim();
            if c.is_empty() || c.chars().count() > MAX_CATEGORY_CHARS {
                warn.push(format!(
                    "categoría «{raw}» ignorada (1–{MAX_CATEGORY_CHARS} caracteres)"
                ));
            } else if cats.iter().any(|x| x.to_lowercase() == c.to_lowercase()) {
                warn.push(format!("categoría «{c}» repetida; ignorada"));
            } else if cats.len() == MAX_CATEGORIES {
                warn.push(format!(
                    "categoría «{c}» ignorada (máximo {MAX_CATEGORIES})"
                ));
            } else {
                cats.push(c.to_owned());
            }
        }
        if cats.is_empty() {
            warn.push("sin categorías válidas; uso las de por defecto".to_owned());
            cats = def.categories;
        }
        self.categories = cats;

        let dc = Custom::default();
        let mut fix = |name: &str, v: &mut u32, (lo, hi): (u32, u32), d: u32| {
            if !(lo..=hi).contains(v) {
                warn.push(format!(
                    "custom.{name} = {v} fuera de rango ({lo}–{hi}); uso {d}"
                ));
                *v = d;
            }
        };
        let c = &mut self.custom;
        fix("focus_min", &mut c.focus_min, FOCUS_MIN, dc.focus_min);
        fix("break_min", &mut c.break_min, BREAK_MIN, dc.break_min);
        fix(
            "long_break_min",
            &mut c.long_break_min,
            LONG_BREAK_MIN,
            dc.long_break_min,
        );
        fix("cycles", &mut c.cycles, CYCLES, dc.cycles);

        (self, warn)
    }
}

/// Texto del `config.toml` inicial, con comentarios para editarlo a mano.
pub fn default_config_toml() -> String {
    let d = Config::default();
    let cats = d
        .categories
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "# {app}: configuración\n\
         \n\
         # Pomodoros por día ({g0}–{g1}).\n\
         daily_goal = {goal}\n\
         \n\
         # Categorías de enfoque (1–{mc}, únicas, 1–{mch} caracteres).\n\
         categories = [{cats}]\n\
         \n\
         # Sesión Personalizado, en minutos.\n\
         [custom]\n\
         focus_min = {f}        # {f0}–{f1}\n\
         break_min = {b}        # {b0}–{b1}\n\
         long_break_min = {l}   # {l0}–{l1}\n\
         cycles = {cy}           # {c0}–{c1}: descanso largo tras N focos\n",
        app = crate::APP_NAME,
        g0 = DAILY_GOAL.0,
        g1 = DAILY_GOAL.1,
        goal = d.daily_goal,
        mc = MAX_CATEGORIES,
        mch = MAX_CATEGORY_CHARS,
        f = d.custom.focus_min,
        f0 = FOCUS_MIN.0,
        f1 = FOCUS_MIN.1,
        b = d.custom.break_min,
        b0 = BREAK_MIN.0,
        b1 = BREAK_MIN.1,
        l = d.custom.long_break_min,
        l0 = LONG_BREAK_MIN.0,
        l1 = LONG_BREAK_MIN.1,
        cy = d.custom.cycles,
        c0 = CYCLES.0,
        c1 = CYCLES.1,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;

    #[test]
    fn config_por_defecto_es_valida_y_coincide_con_plantilla() {
        let (c, warn) = Config::default().sanitize();
        assert!(warn.is_empty(), "{warn:?}");
        let parsed: Result<Config, _> = toml::from_str(&default_config_toml());
        assert_eq!(parsed.ok(), Some(c));
    }

    #[test]
    fn sanitize_corrige_cada_campo() {
        let c = Config {
            daily_goal: 0,
            categories: vec![
                " Programar ".into(),
                "programar".into(),
                "".into(),
                "x".repeat(21),
                "A".into(),
                "B".into(),
                "C".into(),
                "D".into(),
                "E".into(),
                "F".into(),
                "G".into(),
                "H".into(),
            ],
            custom: Custom {
                focus_min: 500,
                break_min: 5,
                long_break_min: 0,
                cycles: 3,
            },
        };
        let (c, warn) = c.sanitize();
        assert_eq!(c.daily_goal, 8);
        assert_eq!(
            c.categories,
            ["Programar", "A", "B", "C", "D", "E", "F", "G"]
        );
        assert_eq!(c.custom.focus_min, 45);
        assert_eq!(c.custom.break_min, 5);
        assert_eq!(c.custom.long_break_min, 20);
        // goal + repetida + vacía + larga + exceso(H) + focus + long_break
        assert_eq!(warn.len(), 7, "{warn:?}");
    }

    #[test]
    fn sanitize_sin_categorias_usa_defecto() {
        let (c, warn) = Config {
            categories: vec![],
            ..Config::default()
        }
        .sanitize();
        assert_eq!(c.categories.len(), DEFAULT_CATEGORIES.len());
        assert_eq!(warn.len(), 1);
    }

    #[test]
    fn toml_parcial_completa_con_defecto() {
        let c: Result<Config, _> = toml::from_str("daily_goal = 4\n[custom]\ncycles = 2\n");
        let c = c.ok();
        assert_eq!(c.as_ref().map(|c| c.daily_goal), Some(4));
        assert_eq!(c.as_ref().map(|c| c.custom.cycles), Some(2));
        assert_eq!(c.map(|c| c.custom.focus_min), Some(45));
    }

    fn ended(phase: Phase, completed: bool) -> PhaseEnded {
        PhaseEnded {
            phase,
            planned: Duration::from_secs(1500),
            actual: Duration::from_secs(1500),
            completed,
        }
    }

    #[test]
    fn registro_usa_fecha_local_y_limpia_descansos() {
        let task = Task {
            id: 2,
            title: "Escribir parser de ejemplo".into(),
            estimate: 4,
            done: 1,
        };
        let tz = TimeZone::fixed(jiff::tz::offset(-5));
        let at: Timestamp = "2027-01-01T03:00:00Z".parse().unwrap_or_default();
        let r = CycleRecord::new(
            ended(Phase::Focus, true),
            at,
            &tz,
            SessionKind::Clasico,
            "Programar",
            Some(&task),
        );
        assert_eq!(r.local_date, date(2026, 12, 31));
        assert_eq!(r.task_id, Some(2));
        assert!(r.is_pomodoro());

        let b = CycleRecord::new(
            ended(Phase::ShortBreak, true),
            at,
            &tz,
            SessionKind::Clasico,
            "Programar",
            Some(&task),
        );
        assert_eq!((&b.category, b.task_id, &b.task), (&None, None, &None));
        assert!(!b.is_pomodoro());
    }

    #[test]
    fn tareas_alta_edicion_borrado_y_activa() {
        let mut l = TaskList::default();
        assert_eq!(l.add("   "), None);
        let a = l.add("  Repasar vocabulario  ");
        let b = l.add(&"x".repeat(80));
        assert_eq!((a, b), (Some(1), Some(2)));
        assert_eq!(l.tasks[0].title, "Repasar vocabulario");
        assert_eq!(l.tasks[1].title.chars().count(), MAX_TITLE_CHARS);
        assert!(l.rename(1, "Leer capítulo 3"));
        assert!(!l.rename(1, " "), "título vacío no se acepta");
        assert!(!l.rename(9, "nada"));

        l.toggle_active(1);
        assert_eq!(l.active, Some(1));
        assert!(l.credit_active());
        assert_eq!(l.tasks[0].done, 1);
        l.toggle_active(1);
        assert_eq!(l.active, None);
        assert!(!l.credit_active());

        l.bump_estimate(1, -5);
        assert_eq!(l.tasks[0].estimate, ESTIMATE.0);
        l.bump_estimate(1, 100);
        assert_eq!(l.tasks[0].estimate, ESTIMATE.1);

        l.toggle_active(2);
        l.remove(2);
        assert_eq!((l.active, l.tasks.len()), (None, 1));
        assert_eq!(l.add("otra"), Some(3), "los ids no se reutilizan");
    }

    #[test]
    fn formato_json_del_registro() {
        let at: Timestamp = "2026-03-14T15:25:00Z".parse().unwrap_or_default();
        let r = CycleRecord::new(
            ended(Phase::Focus, true),
            at,
            &TimeZone::UTC,
            SessionKind::Clasico,
            "Programar",
            None,
        );
        let json = serde_json::to_string(&r).unwrap_or_default();
        assert_eq!(
            json,
            r#"{"ended_at":"2026-03-14T15:25:00Z","local_date":"2026-03-14","phase":"focus","session":"clasico","category":"Programar","task_id":null,"task":null,"planned_secs":1500,"actual_secs":1500,"completed":true}"#
        );
    }
}
