//! Reloj inyectable: el monotónico mide fases; el de pared fecha los registros.

use std::time::Instant;

use jiff::{Timestamp, civil::Date, tz::TimeZone};

pub trait Clock {
    /// Reloj monotónico para medir la duración de las fases.
    fn mono(&self) -> Instant;
    /// Instante actual (UTC) para fechar registros.
    fn now(&self) -> Timestamp;
    /// Zona horaria con la que se calcula el día local.
    fn tz(&self) -> TimeZone;

    /// Fecha local de hoy según `now()` y `tz()`.
    fn today(&self) -> Date {
        self.now().to_zoned_datetime(self.tz()).date()
    }
}

/// Reloj real del sistema. Si la zona del sistema no se puede leer, usa UTC.
pub struct SystemClock {
    tz: TimeZone,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            tz: TimeZone::try_system().unwrap_or(TimeZone::UTC),
        }
    }
}

impl Clock for SystemClock {
    fn mono(&self) -> Instant {
        Instant::now()
    }
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
    fn tz(&self) -> TimeZone {
        self.tz.clone()
    }
}

#[cfg(test)]
pub use fake::FakeClock;

#[cfg(test)]
mod fake {
    use std::{cell::Cell, time::Duration};

    use super::*;

    /// Reloj de pruebas: solo avanza con `advance`, sin `sleep`.
    pub struct FakeClock {
        mono0: Instant,
        wall0: Timestamp,
        tz: TimeZone,
        offset: Cell<Duration>,
    }

    impl FakeClock {
        /// `wall0` en RFC 3339, p. ej. "2026-03-14T09:00:00Z".
        pub fn new(wall0: &str, tz: TimeZone) -> Self {
            Self {
                mono0: Instant::now(),
                wall0: wall0.parse().unwrap_or(Timestamp::UNIX_EPOCH),
                tz,
                offset: Cell::new(Duration::ZERO),
            }
        }

        pub fn advance(&self, d: Duration) {
            self.offset.set(self.offset.get() + d);
        }
    }

    /// Compartible: los tests conservan un `Rc` para avanzar el reloj de la app.
    impl Clock for std::rc::Rc<FakeClock> {
        fn mono(&self) -> Instant {
            (**self).mono()
        }
        fn now(&self) -> Timestamp {
            (**self).now()
        }
        fn tz(&self) -> TimeZone {
            (**self).tz()
        }
    }

    impl Clock for FakeClock {
        fn mono(&self) -> Instant {
            self.mono0 + self.offset.get()
        }
        fn now(&self) -> Timestamp {
            let secs = i64::try_from(self.offset.get().as_secs()).unwrap_or(i64::MAX);
            self.wall0
                .checked_add(jiff::SignedDuration::from_secs(secs))
                .unwrap_or(self.wall0)
        }
        fn tz(&self) -> TimeZone {
            self.tz.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use jiff::civil::date;

    use super::*;

    #[test]
    fn fake_avanza_mono_y_pared() {
        let c = FakeClock::new("2026-03-14T09:00:00Z", TimeZone::UTC);
        let m0 = c.mono();
        c.advance(Duration::from_secs(90));
        assert_eq!(c.mono() - m0, Duration::from_secs(90));
        assert_eq!(c.now().to_string(), "2026-03-14T09:01:30Z");
    }

    #[test]
    fn today_cambia_a_medianoche_local() {
        // UTC-5 fijo: las 04:59Z son todavía el día anterior en local.
        let tz = TimeZone::fixed(jiff::tz::offset(-5));
        let c = FakeClock::new("2026-12-31T04:59:00Z", tz);
        assert_eq!(c.today(), date(2026, 12, 30));
        c.advance(Duration::from_secs(60));
        assert_eq!(c.today(), date(2026, 12, 31));
        c.advance(Duration::from_secs(24 * 3600));
        assert_eq!(c.today(), date(2027, 1, 1));
    }
}
