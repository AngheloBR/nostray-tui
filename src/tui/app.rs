//! Estado de la TUI y manejo de teclas. No dibuja nada.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use jiff::civil::Date;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::layout::PanelId;
use crate::{
    clock::Clock,
    model::{Config, CycleRecord, History, TaskList},
    notify::{Notifier, NotifyError},
    session::{BREAK_MIN, CYCLES, Custom, Durations, FOCUS_MIN, LONG_BREAK_MIN, SessionKind},
    store::{self, Loaded, Paths},
    timer::{Phase, PhaseEnded, Timer},
};

const TOAST_FOR: Duration = Duration::from_secs(4);
/// Avisos en cola como mucho (el visible incluido): los intermedios se descartan.
const TOAST_MAX: usize = 3;
/// Tope del desplazamiento de la ayuda (más que sus líneas: el dibujo lo ajusta).
const HELP_MAX_TOP: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Info,
    Warn,
    Err,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub msg: String,
    pub level: Level,
}

/// Rango del panel Distribución (`v` alterna).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistRange {
    Today,
    Week,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    /// Ayuda; el número es la primera línea visible (se desplaza con ↑↓).
    Help(u16),
    /// Selector de categoría.
    Categories(usize),
    /// Texto de una tarea: `edit = None` es alta.
    TaskInput {
        edit: Option<u64>,
        buf: String,
    },
    ConfirmDelete(u64),
    /// Selector de sesión; `initial` = abierto al arrancar.
    Sessions {
        idx: usize,
        initial: bool,
    },
    /// `custom`: valores editados de Personalizado que solo se guardan si se confirma.
    ConfirmSwitch {
        kind: SessionKind,
        custom: Option<Custom>,
    },
    EditCustom {
        field: usize,
        values: Custom,
    },
    ConfirmQuit,
}

pub struct App {
    clock: Box<dyn Clock>,
    notifier: Box<dyn Notifier>,
    /// `false` tras ver que `notify-send` no existe: no se vuelve a intentar.
    notify_ok: bool,
    /// `None` en tests: no se escribe a disco.
    paths: Option<Paths>,
    pub history: History,
    pub tasks: TaskList,
    pub config: Config,
    pub session: SessionKind,
    pub timer: Timer,
    pub category: usize,
    pub focus: PanelId,
    pub dist_range: DistRange,
    /// Fila seleccionada en Tareas.
    pub task_sel: usize,
    /// Desplazamiento del Log (0 = lo más reciente arriba).
    pub log_scroll: usize,
    /// Paneles enfocados más recientes primero (para los huecos que rotan).
    recent: Vec<PanelId>,
    pub modal: Option<Modal>,
    toasts: VecDeque<Toast>,
    toast_until: Option<Instant>,
    /// Instante monotónico del fotograma actual.
    pub now: Instant,
    /// Día local actual (cambia a medianoche).
    pub today: Date,
    pub quit: bool,
}

impl App {
    pub fn new(
        clock: Box<dyn Clock>,
        notifier: Box<dyn Notifier>,
        paths: Option<Paths>,
        loaded: Loaded,
        start: Option<SessionKind>,
        category: Option<usize>,
    ) -> Self {
        // Sin elección explícita (CLI), se retoma la última de `state.json`.
        let session = start
            .or_else(|| loaded.state.session())
            .unwrap_or(SessionKind::Clasico);
        let category = category
            .or_else(|| loaded.state.category_in(&loaded.config.categories))
            .unwrap_or(0);
        let durations = durations_of(session, &loaded.config);
        let now = clock.mono();
        let today = clock.today();
        let mut app = Self {
            clock,
            notifier,
            notify_ok: true,
            paths,
            history: loaded.history,
            tasks: loaded.tasks,
            config: loaded.config,
            session,
            timer: Timer::new(durations),
            category,
            focus: PanelId::Timer,
            dist_range: DistRange::Today,
            task_sel: 0,
            log_scroll: 0,
            recent: vec![PanelId::Timer],
            modal: start.is_none().then(|| Modal::Sessions {
                idx: SessionKind::ALL
                    .iter()
                    .position(|k| *k == session)
                    .unwrap_or(0),
                initial: true,
            }),
            toasts: VecDeque::new(),
            toast_until: None,
            now,
            today,
            quit: false,
        };
        for n in loaded.notices {
            app.say(n, Level::Warn);
        }
        app
    }

    pub fn category_name(&self) -> &str {
        self.config
            .categories
            .get(self.category)
            .map_or("Otro", String::as_str)
    }

    pub fn tz(&self) -> jiff::tz::TimeZone {
        self.clock.tz()
    }

    pub fn active_task(&self) -> Option<&crate::model::Task> {
        let id = self.tasks.active?;
        self.tasks.tasks.iter().find(|t| t.id == id)
    }

    /// Pomodoros completados hoy.
    pub fn today_count(&self) -> u32 {
        crate::stats::day(&self.history, self.today, self.config.daily_goal).pomodoros
    }

    /// Hora local («HH:MM») a la que acabará la fase si sigue corriendo.
    pub fn ends_at(&self) -> Option<String> {
        if !self.timer.is_running() {
            return None;
        }
        let rem = jiff::SignedDuration::try_from(self.timer.remaining(self.now)).ok()?;
        let at = self.clock.now().checked_add(rem).ok()?;
        Some(
            at.to_zoned_datetime(self.clock.tz())
                .strftime("%H:%M")
                .to_string(),
        )
    }

    pub fn toast(&self) -> Option<&Toast> {
        self.toasts.front()
    }

    pub fn say(&mut self, msg: impl Into<String>, level: Level) {
        let t = Toast {
            msg: msg.into(),
            level,
        };
        if self.toasts.back() == Some(&t) {
            return;
        }
        // Se conserva el visible y los más nuevos: una ráfaga no deja cola de minutos.
        while self.toasts.len() >= TOAST_MAX {
            if self.toasts.remove(1).is_none() {
                break;
            }
        }
        self.toasts.push_back(t);
    }

    /// El panel que se muestra en un hueco que rota: el más reciente enfocado.
    pub fn pick(&self, members: &[PanelId]) -> PanelId {
        self.recent
            .iter()
            .copied()
            .find(|p| members.contains(p))
            .or_else(|| members.first().copied())
            .unwrap_or(PanelId::Dist)
    }

    /// Avanza el reloj; devuelve `true` si algo visible cambió.
    pub fn update(&mut self) -> bool {
        let before = self.view_key();
        self.now = self.clock.mono();
        self.today = self.clock.today();
        if let Some(ended) = self.timer.tick(self.now) {
            self.on_phase_end(ended);
        }
        match self.toast_until {
            None if !self.toasts.is_empty() => self.toast_until = Some(self.now + TOAST_FOR),
            Some(t) if self.now >= t => {
                self.toasts.pop_front();
                self.toast_until = None;
            }
            _ => {}
        }
        before != self.view_key()
    }

    /// Lo que cambia la pantalla con el paso del tiempo.
    fn view_key(&self) -> (u64, Phase, usize, bool, Date) {
        (
            self.timer.remaining(self.now).as_secs(),
            self.timer.phase(),
            self.toasts.len(),
            self.toast_until.is_some(),
            self.today,
        )
    }

    fn on_phase_end(&mut self, ended: PhaseEnded) {
        let rec = CycleRecord::new(
            ended,
            self.clock.now(),
            &self.clock.tz(),
            self.session,
            self.category_name(),
            self.active_task(),
        );
        self.history.cycles.push(rec);
        self.save_history();
        self.log_scroll = 0;
        if ended.phase == Phase::Focus && ended.completed && self.tasks.credit_active() {
            self.save_tasks();
        }
        let msg = match (ended.phase, ended.completed) {
            (Phase::Focus, true) => "✓ foco completado · toca descansar",
            (Phase::Focus, false) => "foco saltado",
            (_, true) => "✓ descanso terminado · a enfocar",
            (_, false) => "descanso saltado",
        };
        self.say(
            msg,
            if ended.completed {
                Level::Ok
            } else {
                Level::Info
            },
        );
        // Solo al terminar una fase por sí sola: saltar es una acción del usuario.
        if ended.completed {
            self.desktop_notice(ended.phase);
        }
    }

    /// Aviso de escritorio. `self.timer` ya apunta a la fase siguiente.
    fn desktop_notice(&mut self, ended: Phase) {
        if !self.notify_ok {
            return;
        }
        let next_min = self.timer.planned().as_secs() / 60;
        let (title, body) = match ended {
            Phase::Focus => {
                let mut what = self.category_name().to_owned();
                if let Some(t) = self.active_task() {
                    what = format!("{what} · {}", t.title);
                }
                let brk = if self.timer.phase() == Phase::LongBreak {
                    "descanso largo"
                } else {
                    "descanso"
                };
                (
                    "Foco completado".to_owned(),
                    format!("{what}\nToca {brk} de {next_min} min."),
                )
            }
            Phase::ShortBreak | Phase::LongBreak => (
                "Descanso terminado".to_owned(),
                format!("A enfocar: {next_min} min de {}.", self.category_name()),
            ),
        };
        match self
            .notifier
            .notify(&format!("{} · {title}", crate::APP_NAME), &body)
        {
            Ok(()) => {}
            Err(NotifyError::Missing) => {
                self.notify_ok = false;
                self.say(
                    "notify-send no está instalado: los avisos solo se ven aquí",
                    Level::Warn,
                );
            }
            Err(NotifyError::Other(e)) => {
                self.say(format!("no se pudo notificar: {e}"), Level::Warn);
            }
        }
    }

    fn save_tasks(&mut self) {
        if let Some(p) = &self.paths
            && let Err(e) = store::save_json(&p.tasks(), &self.tasks)
        {
            self.say(format!("no se pudo guardar: {e}"), Level::Err);
        }
    }

    fn selected_task_id(&self) -> Option<u64> {
        self.tasks.tasks.get(self.task_sel).map(|t| t.id)
    }

    fn clamp_task_sel(&mut self) {
        self.task_sel = self.task_sel.min(self.tasks.tasks.len().saturating_sub(1));
    }

    /// Teclas del panel Tareas (con el foco puesto). Devuelve si la consumió.
    fn on_tasks_key(&mut self, k: KeyEvent) -> bool {
        let n = self.tasks.tasks.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.task_sel = self.task_sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.task_sel = (self.task_sel + 1).min(n.saturating_sub(1));
            }
            KeyCode::Char('a') => {
                self.modal = Some(Modal::TaskInput {
                    edit: None,
                    buf: String::new(),
                });
            }
            KeyCode::Char('e') => {
                if let Some(t) = self.tasks.tasks.get(self.task_sel) {
                    self.modal = Some(Modal::TaskInput {
                        edit: Some(t.id),
                        buf: t.title.clone(),
                    });
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(id) = self.selected_task_id() {
                    self.modal = Some(Modal::ConfirmDelete(id));
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.selected_task_id() {
                    self.tasks.toggle_active(id);
                    self.save_tasks();
                }
            }
            KeyCode::Char('+') | KeyCode::Char('-') => {
                if let Some(id) = self.selected_task_id() {
                    let by = if k.code == KeyCode::Char('+') { 1 } else { -1 };
                    self.tasks.bump_estimate(id, by);
                    self.save_tasks();
                }
            }
            _ => return false,
        }
        true
    }

    /// Teclas del panel Log (con el foco puesto). Devuelve si la consumió.
    fn on_log_key(&mut self, k: KeyEvent) -> bool {
        let max = self.history.cycles.len().saturating_sub(1);
        self.log_scroll = match k.code {
            KeyCode::Down | KeyCode::Char('j') => self.log_scroll + 1,
            KeyCode::Up | KeyCode::Char('k') => self.log_scroll.saturating_sub(1),
            KeyCode::PageDown => self.log_scroll + 10,
            KeyCode::PageUp => self.log_scroll.saturating_sub(10),
            KeyCode::Home => 0,
            KeyCode::End => max,
            _ => return false,
        }
        .min(max);
        true
    }

    fn save_history(&mut self) {
        if let Some(p) = &self.paths
            && let Err(e) = store::save_json(&p.history(), &self.history)
        {
            self.say(format!("no se pudo guardar: {e}"), Level::Err);
        }
    }

    fn apply_session(&mut self, kind: SessionKind) {
        self.session = kind;
        self.timer.set_durations(durations_of(kind, &self.config));
        self.say(format!("sesión {}", kind.label()), Level::Info);
    }

    /// Elige sesión; con un ciclo en curso pide confirmación antes de tocar nada.
    fn choose_session(&mut self, kind: SessionKind, custom: Option<Custom>) -> Option<Modal> {
        if self.timer.in_progress(self.now) {
            return Some(Modal::ConfirmSwitch { kind, custom });
        }
        self.switch_session(kind, custom);
        None
    }

    fn switch_session(&mut self, kind: SessionKind, custom: Option<Custom>) {
        if let Some(values) = custom {
            self.config.custom = values;
            if let Some(p) = &self.paths
                && let Err(e) = store::save_config(&p.config(), &self.config)
            {
                self.say(format!("no se pudo guardar: {e}"), Level::Err);
            }
        }
        self.apply_session(kind);
    }

    fn set_focus(&mut self, p: PanelId) {
        self.focus = p;
        self.recent.retain(|x| *x != p);
        self.recent.insert(0, p);
    }

    pub fn on_key(&mut self, k: KeyEvent) {
        self.now = self.clock.mono();
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if let Some(m) = self.modal.take() {
            self.modal = self.on_modal_key(m, k);
            return;
        }
        let consumed = match self.focus {
            PanelId::Tasks => self.on_tasks_key(k),
            PanelId::Log => self.on_log_key(k),
            _ => false,
        };
        if consumed {
            return;
        }
        match k.code {
            KeyCode::Char(' ') => self.timer.toggle(self.now),
            KeyCode::Char('s') => {
                if let Some(e) = self.timer.skip(self.now) {
                    self.on_phase_end(e);
                }
            }
            KeyCode::Char('r') => self.timer.reset(),
            KeyCode::Tab => self.set_focus(self.focus.next()),
            KeyCode::BackTab => self.set_focus(self.focus.prev()),
            KeyCode::Char('m') => {
                let idx = SessionKind::ALL
                    .iter()
                    .position(|k| *k == self.session)
                    .unwrap_or(0);
                self.modal = Some(Modal::Sessions {
                    idx,
                    initial: false,
                });
            }
            KeyCode::Char('v') if self.focus == PanelId::Dist => {
                self.dist_range = match self.dist_range {
                    DistRange::Today => DistRange::Week,
                    DistRange::Week => DistRange::Today,
                };
            }
            KeyCode::Char('c') => self.modal = Some(Modal::Categories(self.category)),
            KeyCode::Char('?') => self.modal = Some(Modal::Help(0)),
            KeyCode::Char('q') => {
                if self.timer.in_progress(self.now) {
                    self.modal = Some(Modal::ConfirmQuit);
                } else {
                    self.quit = true;
                }
            }
            _ => {}
        }
    }

    fn on_text_key(&mut self, edit: Option<u64>, mut buf: String, k: KeyEvent) -> Option<Modal> {
        match k.code {
            KeyCode::Esc => return None,
            KeyCode::Enter => {
                let ok = match edit {
                    None => self
                        .tasks
                        .add(&buf)
                        .inspect(|_| {
                            self.task_sel = self.tasks.tasks.len().saturating_sub(1);
                        })
                        .is_some(),
                    Some(id) => self.tasks.rename(id, &buf),
                };
                if ok {
                    self.save_tasks();
                    return None;
                }
                // Título vacío: el modal sigue abierto.
            }
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Char(c)
                if !k.modifiers.contains(KeyModifiers::CONTROL)
                    && buf.chars().count() < crate::model::MAX_TITLE_CHARS =>
            {
                buf.push(c);
            }
            _ => {}
        }
        Some(Modal::TaskInput { edit, buf })
    }

    fn on_modal_key(&mut self, m: Modal, k: KeyEvent) -> Option<Modal> {
        let yes = matches!(
            k.code,
            KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter
        );
        match m {
            Modal::Help(top) => match k.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    Some(Modal::Help(top.saturating_add(1).min(HELP_MAX_TOP)))
                }
                KeyCode::Up | KeyCode::Char('k') => Some(Modal::Help(top.saturating_sub(1))),
                _ => None,
            },
            Modal::TaskInput { edit, buf } => self.on_text_key(edit, buf, k),
            Modal::ConfirmDelete(id) => {
                if yes {
                    self.tasks.remove(id);
                    self.clamp_task_sel();
                    self.save_tasks();
                }
                None
            }
            Modal::Categories(idx) => {
                let n = self.config.categories.len().max(1);
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => Some(Modal::Categories((idx + n - 1) % n)),
                    KeyCode::Down | KeyCode::Char('j') => Some(Modal::Categories((idx + 1) % n)),
                    KeyCode::Char(c @ '1'..='8') => {
                        let i = usize::from(c as u8 - b'1');
                        if i < n {
                            self.category = i;
                            None
                        } else {
                            Some(Modal::Categories(idx))
                        }
                    }
                    KeyCode::Enter => {
                        self.category = idx % n;
                        None
                    }
                    KeyCode::Esc | KeyCode::Char('q') => None,
                    _ => Some(Modal::Categories(idx)),
                }
            }
            Modal::ConfirmQuit => {
                self.quit = yes;
                None
            }
            Modal::ConfirmSwitch { kind, custom } => {
                if yes {
                    self.switch_session(kind, custom);
                }
                None
            }
            Modal::Sessions { idx, initial } => {
                let n = SessionKind::ALL.len();
                let kind = SessionKind::ALL[idx % n];
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => Some(Modal::Sessions {
                        idx: (idx + n - 1) % n,
                        initial,
                    }),
                    KeyCode::Down | KeyCode::Char('j') => Some(Modal::Sessions {
                        idx: (idx + 1) % n,
                        initial,
                    }),
                    KeyCode::Char('e') if kind == SessionKind::Personalizado => {
                        Some(Modal::EditCustom {
                            field: 0,
                            values: self.config.custom,
                        })
                    }
                    KeyCode::Enter => self.choose_session(kind, None),
                    // Recién abierto, `q` sale del programa; si no, solo cierra.
                    KeyCode::Char('q') if initial => {
                        self.quit = true;
                        None
                    }
                    KeyCode::Esc | KeyCode::Char('q') => None,
                    _ => Some(m),
                }
            }
            Modal::EditCustom { field, mut values } => {
                let step: i64 = if k.modifiers.contains(KeyModifiers::SHIFT) {
                    5
                } else {
                    1
                };
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => Some(Modal::EditCustom {
                        field: (field + CUSTOM_FIELDS.len() - 1) % CUSTOM_FIELDS.len(),
                        values,
                    }),
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => Some(Modal::EditCustom {
                        field: (field + 1) % CUSTOM_FIELDS.len(),
                        values,
                    }),
                    KeyCode::Left | KeyCode::Char('-' | 'h') => {
                        bump(&mut values, field, -step);
                        Some(Modal::EditCustom { field, values })
                    }
                    KeyCode::Right | KeyCode::Char('+' | 'l') => {
                        bump(&mut values, field, step);
                        Some(Modal::EditCustom { field, values })
                    }
                    KeyCode::Enter => self.choose_session(SessionKind::Personalizado, Some(values)),
                    KeyCode::Esc => Some(Modal::Sessions {
                        idx: 3,
                        initial: false,
                    }),
                    _ => Some(m),
                }
            }
        }
    }
}

/// Campos editables de la sesión Personalizado: (etiqueta, rango).
pub const CUSTOM_FIELDS: [(&str, (u32, u32)); 4] = [
    ("enfoque (min)", FOCUS_MIN),
    ("descanso (min)", BREAK_MIN),
    ("descanso largo (min)", LONG_BREAK_MIN),
    ("ciclos hasta el largo", CYCLES),
];

pub fn custom_value(c: &Custom, field: usize) -> u32 {
    match field {
        0 => c.focus_min,
        1 => c.break_min,
        2 => c.long_break_min,
        _ => c.cycles,
    }
}

fn bump(c: &mut Custom, field: usize, by: i64) {
    let (lo, hi) = CUSTOM_FIELDS[field.min(3)].1;
    let v = (i64::from(custom_value(c, field)) + by).clamp(i64::from(lo), i64::from(hi));
    let v = u32::try_from(v).unwrap_or(lo);
    match field {
        0 => c.focus_min = v,
        1 => c.break_min = v,
        2 => c.long_break_min = v,
        _ => c.cycles = v,
    }
}

pub fn durations_of(kind: SessionKind, cfg: &Config) -> Durations {
    kind.preset()
        .or_else(|| cfg.custom.validate().ok())
        .or_else(|| Custom::default().validate().ok())
        .unwrap_or(Durations {
            focus: Duration::from_secs(25 * 60),
            short_break: Duration::from_secs(5 * 60),
            long: None,
        })
}

#[cfg(test)]
pub mod tests_support {
    use jiff::tz::TimeZone;
    use ratatui::crossterm::event::KeyEventKind;

    use std::rc::Rc;

    use super::*;
    use crate::{clock::FakeClock, model::State, notify::Recorder};

    pub fn key(c: KeyCode) -> KeyEvent {
        KeyEvent {
            code: c,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::NONE,
        }
    }

    pub fn app_with(start: Option<SessionKind>, loaded: Loaded) -> (App, Rc<FakeClock>) {
        let (app, clock, _) = app_notify(start, loaded, Recorder::default());
        (app, clock)
    }

    /// Como `app_with`, devolviendo también el notificador de pruebas.
    pub fn app_notify(
        start: Option<SessionKind>,
        loaded: Loaded,
        rec: Recorder,
    ) -> (App, Rc<FakeClock>, Recorder) {
        let clock = Rc::new(FakeClock::new("2026-03-14T09:00:00Z", TimeZone::UTC));
        let app = App::new(
            Box::new(Rc::clone(&clock)),
            Box::new(rec.clone()),
            None,
            loaded,
            start,
            None,
        );
        (app, clock, rec)
    }

    pub fn empty() -> Loaded {
        Loaded {
            history: History::default(),
            tasks: TaskList::default(),
            config: Config::default(),
            state: State::default(),
            notices: vec![],
        }
    }

    fn app(start: Option<SessionKind>) -> App {
        app_with(start, empty()).0
    }

    #[test]
    fn retoma_ultima_sesion_y_categoria() {
        let loaded = || {
            let mut l = empty();
            l.state = State {
                last_session: Some("deep".into()),
                last_category: Some("Leer".into()),
                ..State::default()
            };
            l
        };
        let a = app_with(None, loaded()).0;
        assert_eq!(a.session, SessionKind::Deep);
        assert_eq!(a.category_name(), "Leer");
        assert!(matches!(
            a.modal,
            Some(Modal::Sessions {
                idx: 1,
                initial: true
            })
        ));
        // Lo explícito (CLI) manda sobre lo recordado.
        let a = app_with(Some(SessionKind::Flow), loaded()).0;
        assert_eq!(a.session, SessionKind::Flow);
        assert_eq!(a.category_name(), "Leer");
    }

    #[test]
    fn arranca_con_selector_o_directo() {
        assert!(matches!(
            app(None).modal,
            Some(Modal::Sessions { initial: true, .. })
        ));
        let a = app(Some(SessionKind::Deep));
        assert_eq!(a.modal, None);
        assert_eq!(a.timer.planned(), Duration::from_secs(50 * 60));
    }

    #[test]
    fn q_en_selector_inicial_sale() {
        let mut a = app(None);
        a.on_key(key(KeyCode::Char('q')));
        assert!(a.quit);
        let mut a = app(Some(SessionKind::Clasico));
        a.on_key(key(KeyCode::Char('m')));
        a.on_key(key(KeyCode::Char('q')));
        assert!(!a.quit && a.modal.is_none(), "con m, q solo cierra");
    }

    #[test]
    fn selector_elige_flow() {
        let mut a = app(None);
        a.on_key(key(KeyCode::Down));
        a.on_key(key(KeyCode::Down));
        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.modal, None);
        assert_eq!(a.session, SessionKind::Flow);
    }

    #[test]
    fn q_confirma_solo_con_ciclo_en_curso() {
        let mut a = app(Some(SessionKind::Clasico));
        a.on_key(key(KeyCode::Char('q')));
        assert!(a.quit, "sin ciclo sale directo");

        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(1));
        a.on_key(key(KeyCode::Char('q')));
        assert!(!a.quit);
        assert_eq!(a.modal, Some(Modal::ConfirmQuit));
        a.on_key(key(KeyCode::Char('n')));
        assert!(!a.quit && a.modal.is_none());
    }

    #[test]
    fn editar_personalizado_respeta_rangos() {
        let mut a = app(None);
        for _ in 0..3 {
            a.on_key(key(KeyCode::Down));
        }
        a.on_key(key(KeyCode::Char('e')));
        for _ in 0..200 {
            a.on_key(key(KeyCode::Left)); // enfoque → 1
        }
        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.session, SessionKind::Personalizado);
        assert_eq!(a.config.custom.focus_min, 1);
        assert_eq!(a.timer.planned(), Duration::from_secs(60));
    }

    #[test]
    fn foco_completo_se_registra() {
        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(25 * 60));
        assert!(a.update());
        assert_eq!(a.history.cycles.len(), 1);
        let r = &a.history.cycles[0];
        assert!(r.is_pomodoro());
        assert_eq!(r.category.as_deref(), Some("Programar"));
        assert_eq!(a.timer.phase(), Phase::ShortBreak);
        assert!(a.toast().is_some());
    }

    #[test]
    fn cambiar_sesion_en_curso_pide_confirmar() {
        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(60));
        a.on_key(key(KeyCode::Char('m')));
        a.on_key(key(KeyCode::Down));
        a.on_key(key(KeyCode::Enter));
        assert_eq!(
            a.modal,
            Some(Modal::ConfirmSwitch {
                kind: SessionKind::Deep,
                custom: None
            })
        );
        assert_eq!(a.session, SessionKind::Clasico);
        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.session, SessionKind::Deep);
        assert!(!a.timer.in_progress(a.now));
    }

    #[test]
    fn cancelar_cambio_no_guarda_personalizado() {
        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        let antes = a.config.custom;
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(60));
        a.on_key(key(KeyCode::Char('m')));
        for _ in 0..3 {
            a.on_key(key(KeyCode::Down));
        }
        a.on_key(key(KeyCode::Char('e')));
        a.on_key(key(KeyCode::Left));
        a.on_key(key(KeyCode::Enter));
        assert!(matches!(a.modal, Some(Modal::ConfirmSwitch { .. })));
        a.on_key(key(KeyCode::Char('n')));
        assert_eq!(a.config.custom, antes, "cancelado: config sin tocar");
        assert_eq!(a.session, SessionKind::Clasico);
    }

    #[test]
    fn q_recien_iniciado_pide_confirmar() {
        let (mut a, _c) = app_with(Some(SessionKind::Clasico), empty());
        a.on_key(key(KeyCode::Char(' ')));
        a.on_key(key(KeyCode::Char('q')));
        assert!(!a.quit);
        assert_eq!(a.modal, Some(Modal::ConfirmQuit));
    }

    #[test]
    fn rafaga_de_avisos_no_hace_cola() {
        let mut a = app(Some(SessionKind::Clasico));
        for i in 0..20 {
            a.say(format!("aviso {i}"), Level::Info);
        }
        assert_eq!(a.toasts.len(), TOAST_MAX);
        assert_eq!(a.toast().map(|t| t.msg.as_str()), Some("aviso 0"));
        assert_eq!(a.toasts.back().map(|t| t.msg.as_str()), Some("aviso 19"));
        a.say("aviso 19", Level::Info);
        assert_eq!(a.toasts.len(), TOAST_MAX, "repetido seguido no se encola");
    }

    #[test]
    fn v_alterna_rango_solo_con_distribucion_enfocada() {
        let mut a = app(Some(SessionKind::Clasico));
        a.on_key(key(KeyCode::Char('v')));
        assert_eq!(
            a.dist_range,
            DistRange::Today,
            "Timer enfocado: v no hace nada"
        );
        a.on_key(key(KeyCode::Tab));
        a.on_key(key(KeyCode::Tab));
        assert_eq!(a.focus, PanelId::Dist);
        a.on_key(key(KeyCode::Char('v')));
        assert_eq!(a.dist_range, DistRange::Week);
        a.on_key(key(KeyCode::Char('v')));
        assert_eq!(a.dist_range, DistRange::Today);
    }

    fn type_str(a: &mut App, s: &str) {
        for c in s.chars() {
            a.on_key(key(KeyCode::Char(c)));
        }
    }

    fn focus(a: &mut App, p: PanelId) {
        while a.focus != p {
            a.on_key(key(KeyCode::Tab));
        }
    }

    #[test]
    fn tareas_alta_por_teclado_sin_filtrar_atajos() {
        let mut a = app(Some(SessionKind::Clasico));
        focus(&mut a, PanelId::Tasks);
        a.on_key(key(KeyCode::Char('a')));
        // q, s, espacio y c se escriben: no actúan como atajos dentro del modal.
        type_str(&mut a, "Leer q s c");
        a.on_key(key(KeyCode::Backspace));
        a.on_key(key(KeyCode::Enter));
        assert!(!a.quit && a.modal.is_none());
        assert_eq!(a.tasks.tasks.len(), 1);
        assert_eq!(a.tasks.tasks[0].title, "Leer q s");
        assert!(!a.timer.is_running(), "el espacio no arrancó el timer");
    }

    #[test]
    fn tareas_titulo_vacio_no_cierra_y_esc_cancela() {
        let mut a = app(Some(SessionKind::Clasico));
        focus(&mut a, PanelId::Tasks);
        a.on_key(key(KeyCode::Char('a')));
        type_str(&mut a, "   ");
        a.on_key(key(KeyCode::Enter));
        assert!(matches!(a.modal, Some(Modal::TaskInput { .. })));
        a.on_key(key(KeyCode::Esc));
        assert!(a.modal.is_none() && a.tasks.tasks.is_empty());
    }

    #[test]
    fn tareas_editar_activar_estimar_y_borrar() {
        let mut a = app(Some(SessionKind::Clasico));
        focus(&mut a, PanelId::Tasks);
        for t in ["Uno", "Dos"] {
            a.on_key(key(KeyCode::Char('a')));
            type_str(&mut a, t);
            a.on_key(key(KeyCode::Enter));
        }
        assert_eq!(a.task_sel, 1, "la nueva queda seleccionada");
        a.on_key(key(KeyCode::Char('e')));
        type_str(&mut a, "!");
        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.tasks.tasks[1].title, "Dos!");

        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.active_task().map(|t| t.title.as_str()), Some("Dos!"));
        a.on_key(key(KeyCode::Char('+')));
        a.on_key(key(KeyCode::Char('+')));
        assert_eq!(a.tasks.tasks[1].estimate, 3);

        a.on_key(key(KeyCode::Char('x')));
        a.on_key(key(KeyCode::Char('n')));
        assert_eq!(a.tasks.tasks.len(), 2, "n cancela");
        a.on_key(key(KeyCode::Char('x')));
        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.tasks.tasks.len(), 1);
        assert_eq!(a.tasks.active, None, "borrar la activa la desmarca");
        assert_eq!(a.task_sel, 0);
        a.on_key(key(KeyCode::Up));
        a.on_key(key(KeyCode::Char('x')));
        a.on_key(key(KeyCode::Enter));
        a.on_key(key(KeyCode::Char('x')));
        assert!(a.modal.is_none(), "sin tareas, x no abre nada");
    }

    #[test]
    fn foco_completo_suma_a_la_tarea_activa() {
        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        a.tasks.add("Parser");
        a.tasks.toggle_active(1);
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(25 * 60));
        a.update();
        assert_eq!(a.tasks.tasks[0].done, 1);
        assert_eq!(a.history.cycles[0].task.as_deref(), Some("Parser"));
        // El descanso no suma.
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(5 * 60));
        a.update();
        assert_eq!(a.tasks.tasks[0].done, 1);
        assert_eq!(a.history.cycles[1].task, None);
    }

    #[test]
    fn categoria_con_c_y_numero() {
        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        a.on_key(key(KeyCode::Char('c')));
        assert_eq!(a.modal, Some(Modal::Categories(0)));
        a.on_key(key(KeyCode::Down));
        a.on_key(key(KeyCode::Down));
        a.on_key(key(KeyCode::Enter));
        assert_eq!(a.category_name(), "Inglés");
        a.on_key(key(KeyCode::Char('c')));
        a.on_key(key(KeyCode::Char('4')));
        assert_eq!(a.category_name(), "Leer");
        a.on_key(key(KeyCode::Char('c')));
        a.on_key(key(KeyCode::Char('9')));
        assert!(a.modal.is_some(), "fuera de rango no cierra");
        a.on_key(key(KeyCode::Esc));
        // El registro usa la categoría vigente al cerrar.
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(25 * 60));
        a.update();
        assert_eq!(a.history.cycles[0].category.as_deref(), Some("Leer"));
    }

    #[test]
    fn log_desplaza_con_limites() {
        let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
        for _ in 0..3 {
            a.on_key(key(KeyCode::Char(' ')));
            c.advance(Duration::from_secs(60));
            a.on_key(key(KeyCode::Char('s')));
        }
        assert_eq!(a.history.cycles.len(), 3);
        focus(&mut a, PanelId::Log);
        a.on_key(key(KeyCode::Up));
        assert_eq!(a.log_scroll, 0);
        a.on_key(key(KeyCode::PageDown));
        assert_eq!(a.log_scroll, 2);
        a.on_key(key(KeyCode::Home));
        assert_eq!(a.log_scroll, 0);
        a.on_key(key(KeyCode::End));
        assert_eq!(a.log_scroll, 2);
        // Las teclas globales siguen funcionando con el Log enfocado.
        a.on_key(key(KeyCode::Char('?')));
        assert_eq!(a.modal, Some(Modal::Help(0)));
    }

    #[test]
    fn notifica_al_completar_y_no_al_saltar() {
        let (mut a, c, rec) = app_notify(Some(SessionKind::Clasico), empty(), Recorder::default());
        a.tasks.add("-rm -rf parser"); // título con guion: va tras `--`
        a.tasks.toggle_active(1);
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(25 * 60));
        a.update();
        {
            let sent = rec.sent.borrow();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].0, "nostray · Foco completado");
            assert_eq!(
                sent[0].1,
                "Programar · -rm -rf parser\nToca descanso de 5 min."
            );
        }
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(60));
        a.on_key(key(KeyCode::Char('s'))); // saltar: sin aviso
        assert_eq!(rec.sent.borrow().len(), 1);
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(25 * 60));
        a.update();
        a.on_key(key(KeyCode::Char(' ')));
        c.advance(Duration::from_secs(5 * 60));
        a.update();
        let sent = rec.sent.borrow();
        assert_eq!(sent.len(), 3);
        assert_eq!(sent[2].0, "nostray · Descanso terminado");
        assert_eq!(sent[2].1, "A enfocar: 25 min de Programar.");
    }

    #[test]
    fn descanso_largo_en_el_aviso() {
        let (mut a, c, rec) = app_notify(Some(SessionKind::Clasico), empty(), Recorder::default());
        for _ in 0..4 {
            a.on_key(key(KeyCode::Char(' ')));
            c.advance(a.timer.planned());
            a.update();
            if a.timer.phase() != Phase::LongBreak {
                a.on_key(key(KeyCode::Char(' ')));
                c.advance(a.timer.planned());
                a.update();
            }
        }
        let sent = rec.sent.borrow();
        assert!(
            sent.last()
                .is_some_and(|s| s.1.ends_with("Toca descanso largo de 15 min.")),
            "{sent:?}"
        );
    }

    #[test]
    fn sin_notify_send_avisa_una_vez_y_no_rompe() {
        let rec = Recorder {
            missing: true,
            ..Recorder::default()
        };
        let (mut a, c, _) = app_notify(Some(SessionKind::Clasico), empty(), rec);
        for _ in 0..2 {
            a.on_key(key(KeyCode::Char(' ')));
            c.advance(a.timer.planned());
            a.update();
        }
        assert_eq!(a.history.cycles.len(), 2, "el ciclo sigue registrándose");
        let warns = a
            .toasts
            .iter()
            .filter(|t| t.msg.contains("notify-send no está instalado"))
            .count();
        assert_eq!(warns, 1);
    }

    #[test]
    fn tab_actualiza_recientes() {
        let mut a = app(Some(SessionKind::Clasico));
        for _ in 0..5 {
            a.on_key(key(KeyCode::Tab));
        }
        assert_eq!(a.focus, PanelId::Log);
        let all = [
            PanelId::Dist,
            PanelId::Activity,
            PanelId::Tasks,
            PanelId::Log,
        ];
        assert_eq!(a.pick(&all), PanelId::Log);
        a.on_key(key(KeyCode::Tab));
        assert_eq!(a.focus, PanelId::Timer);
        assert_eq!(a.pick(&all), PanelId::Log, "el hueco recuerda el último");
    }
}
