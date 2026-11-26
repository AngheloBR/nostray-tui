//! Máquina de estados del Pomodoro. Pura: cada operación recibe `now` y nunca duerme.
//!
//! ```text
//!         toggle          toggle
//!  Idle ─────────► Running ◄─────► Paused
//!   ▲                 │ tick: elapsed ≥ planned
//!   └── siguiente fase en Idle ◄── PhaseEnded { completed: true }
//! ```

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::session::Durations;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Running { since: Instant, carried: Duration },
    Paused { elapsed: Duration },
}

/// Registro que se emite al cerrar una fase (completada o saltada).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseEnded {
    pub phase: Phase,
    pub planned: Duration,
    pub actual: Duration,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct Timer {
    durations: Durations,
    phase: Phase,
    status: Status,
    /// Focos completados desde el último descanso largo.
    focus_done: u8,
}

impl Timer {
    pub fn new(durations: Durations) -> Self {
        Self {
            durations,
            phase: Phase::Focus,
            status: Status::Idle,
            focus_done: 0,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn durations(&self) -> Durations {
        self.durations
    }

    pub fn focus_done(&self) -> u8 {
        self.focus_done
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, Status::Running { .. })
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.status, Status::Paused { .. })
    }

    /// Hay un ciclo empezado (corriendo o en pausa con tiempo acumulado).
    pub fn in_progress(&self, now: Instant) -> bool {
        !self.elapsed(now).is_zero()
    }

    pub fn planned(&self) -> Duration {
        match self.phase {
            Phase::Focus => self.durations.focus,
            Phase::ShortBreak => self.durations.short_break,
            Phase::LongBreak => self
                .durations
                .long
                .map_or(self.durations.short_break, |l| l.len),
        }
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        let e = match self.status {
            Status::Idle => Duration::ZERO,
            Status::Running { since, carried } => carried + now.saturating_duration_since(since),
            Status::Paused { elapsed } => elapsed,
        };
        e.min(self.planned())
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        self.planned().saturating_sub(self.elapsed(now))
    }

    /// Progreso de la fase actual en `0.0..=1.0`.
    pub fn progress(&self, now: Instant) -> f64 {
        let planned = self.planned().as_secs_f64();
        if planned <= 0.0 {
            return 0.0;
        }
        (self.elapsed(now).as_secs_f64() / planned).clamp(0.0, 1.0)
    }

    /// Espacio: iniciar, pausar o reanudar.
    pub fn toggle(&mut self, now: Instant) {
        self.status = match self.status {
            Status::Idle => Status::Running {
                since: now,
                carried: Duration::ZERO,
            },
            Status::Running { .. } => Status::Paused {
                elapsed: self.elapsed(now),
            },
            Status::Paused { elapsed } => Status::Running {
                since: now,
                carried: elapsed,
            },
        };
    }

    /// Avanza el reloj; si la fase llegó a su duración, la cierra y prepara la siguiente.
    pub fn tick(&mut self, now: Instant) -> Option<PhaseEnded> {
        if !self.is_running() || self.elapsed(now) < self.planned() {
            return None;
        }
        let ended = PhaseEnded {
            phase: self.phase,
            planned: self.planned(),
            actual: self.planned(),
            completed: true,
        };
        self.advance(true);
        Some(ended)
    }

    /// `s`: cierra la fase sin completarla. Solo emite registro si había tiempo acumulado.
    pub fn skip(&mut self, now: Instant) -> Option<PhaseEnded> {
        let actual = self.elapsed(now);
        let ended = (!actual.is_zero()).then_some(PhaseEnded {
            phase: self.phase,
            planned: self.planned(),
            actual,
            completed: false,
        });
        self.advance(false);
        ended
    }

    /// `r`: la fase actual vuelve a cero, en espera.
    pub fn reset(&mut self) {
        self.status = Status::Idle;
    }

    /// Cambio de sesión: empieza de nuevo por un foco.
    pub fn set_durations(&mut self, durations: Durations) {
        *self = Self::new(durations);
    }

    fn advance(&mut self, completed: bool) {
        self.phase = match self.phase {
            Phase::Focus if completed => {
                self.focus_done = self.focus_done.saturating_add(1);
                match self.durations.long {
                    Some(l) if self.focus_done >= l.every => {
                        self.focus_done = 0;
                        Phase::LongBreak
                    }
                    _ => Phase::ShortBreak,
                }
            }
            Phase::Focus => Phase::ShortBreak,
            Phase::ShortBreak | Phase::LongBreak => Phase::Focus,
        };
        self.status = Status::Idle;
    }
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;

    use super::*;
    use crate::{
        clock::{Clock, FakeClock},
        session::SessionKind,
    };

    const MIN: Duration = Duration::from_secs(60);

    fn clasico() -> (Timer, FakeClock) {
        let d = SessionKind::Clasico.preset().unwrap_or(Durations {
            focus: MIN,
            short_break: MIN,
            long: None,
        });
        (
            Timer::new(d),
            FakeClock::new("2026-03-14T09:00:00Z", TimeZone::UTC),
        )
    }

    /// Inicia la fase actual y la deja terminar.
    fn complete(t: &mut Timer, c: &FakeClock) -> Option<PhaseEnded> {
        t.toggle(c.mono());
        c.advance(t.planned());
        t.tick(c.mono())
    }

    #[test]
    fn idle_no_avanza() {
        let (mut t, c) = clasico();
        c.advance(MIN * 30);
        assert_eq!(t.tick(c.mono()), None);
        assert_eq!(t.elapsed(c.mono()), Duration::ZERO);
        assert!(!t.in_progress(c.mono()));
    }

    #[test]
    fn foco_completo_emite_y_pasa_a_descanso() {
        let (mut t, c) = clasico();
        t.toggle(c.mono());
        c.advance(MIN * 24);
        assert_eq!(t.tick(c.mono()), None);
        assert_eq!(t.remaining(c.mono()), MIN);
        c.advance(MIN);
        let e = t.tick(c.mono());
        assert_eq!(
            e,
            Some(PhaseEnded {
                phase: Phase::Focus,
                planned: MIN * 25,
                actual: MIN * 25,
                completed: true
            })
        );
        assert_eq!(t.phase(), Phase::ShortBreak);
        assert!(!t.is_running(), "la fase siguiente espera en Idle");
        assert_eq!(t.focus_done(), 1);
    }

    #[test]
    fn tick_tardio_no_duplica_ni_excede() {
        let (mut t, c) = clasico();
        t.toggle(c.mono());
        c.advance(MIN * 40); // el tick llegó tarde
        let e = t.tick(c.mono());
        assert_eq!(e.map(|e| e.actual), Some(MIN * 25));
        assert_eq!(t.tick(c.mono()), None);
    }

    #[test]
    fn pausa_no_cuenta() {
        let (mut t, c) = clasico();
        t.toggle(c.mono());
        c.advance(MIN * 10);
        t.toggle(c.mono());
        assert!(t.is_paused());
        c.advance(MIN * 60);
        assert_eq!(t.elapsed(c.mono()), MIN * 10);
        assert_eq!(t.tick(c.mono()), None);
        t.toggle(c.mono());
        c.advance(MIN * 5);
        assert_eq!(t.elapsed(c.mono()), MIN * 15);
        assert!((t.progress(c.mono()) - 0.6).abs() < 1e-9);
    }

    #[test]
    fn descanso_largo_tras_cuatro_focos() {
        let (mut t, c) = clasico();
        for i in 1..=4 {
            assert_eq!(t.phase(), Phase::Focus);
            assert!(complete(&mut t, &c).is_some());
            let esperado = if i == 4 {
                Phase::LongBreak
            } else {
                Phase::ShortBreak
            };
            assert_eq!(t.phase(), esperado, "tras el foco {i}");
            assert!(complete(&mut t, &c).is_some());
        }
        assert_eq!(t.phase(), Phase::Focus);
        assert_eq!(t.focus_done(), 0);
    }

    #[test]
    fn sin_descanso_largo_siempre_corto() {
        let d = SessionKind::Deep.preset().unwrap_or(Durations {
            focus: MIN,
            short_break: MIN,
            long: None,
        });
        let mut t = Timer::new(d);
        let c = FakeClock::new("2026-03-14T09:00:00Z", TimeZone::UTC);
        for _ in 0..6 {
            complete(&mut t, &c);
            assert_eq!(t.phase(), Phase::ShortBreak);
            complete(&mut t, &c);
        }
    }

    #[test]
    fn saltar_foco_no_cuenta() {
        let (mut t, c) = clasico();
        t.toggle(c.mono());
        c.advance(MIN * 7);
        let e = t.skip(c.mono());
        assert_eq!(
            e,
            Some(PhaseEnded {
                phase: Phase::Focus,
                planned: MIN * 25,
                actual: MIN * 7,
                completed: false
            })
        );
        assert_eq!(t.phase(), Phase::ShortBreak);
        assert_eq!(t.focus_done(), 0);
    }

    #[test]
    fn saltar_sin_tiempo_no_registra() {
        let (mut t, c) = clasico();
        assert_eq!(t.skip(c.mono()), None);
        assert_eq!(t.phase(), Phase::ShortBreak);
        assert_eq!(t.skip(c.mono()), None);
        assert_eq!(t.phase(), Phase::Focus);
    }

    #[test]
    fn reset_vuelve_a_cero_sin_cambiar_fase() {
        let (mut t, c) = clasico();
        complete(&mut t, &c);
        t.toggle(c.mono());
        c.advance(MIN * 2);
        t.reset();
        assert_eq!(t.phase(), Phase::ShortBreak);
        assert_eq!(t.elapsed(c.mono()), Duration::ZERO);
        assert!(!t.is_running());
        assert_eq!(t.focus_done(), 1);
    }

    #[test]
    fn cambiar_sesion_reinicia() {
        let (mut t, c) = clasico();
        complete(&mut t, &c);
        let flow = SessionKind::Flow.preset().unwrap_or(t.durations());
        t.set_durations(flow);
        assert_eq!(t.phase(), Phase::Focus);
        assert_eq!(t.focus_done(), 0);
        assert_eq!(t.planned(), MIN * 90);
    }

    #[test]
    fn custom_un_minuto() {
        let d = crate::session::Custom {
            focus_min: 1,
            break_min: 1,
            long_break_min: 1,
            cycles: 1,
        }
        .validate();
        let mut t = Timer::new(d.unwrap_or(Durations {
            focus: MIN * 99,
            short_break: MIN,
            long: None,
        }));
        let c = FakeClock::new("2026-03-14T09:00:00Z", TimeZone::UTC);
        assert_eq!(complete(&mut t, &c).map(|e| e.actual), Some(MIN));
        assert_eq!(t.phase(), Phase::LongBreak);
    }
}
