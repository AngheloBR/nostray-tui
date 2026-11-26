//! Sesiones tipo y validación de la sesión personalizada.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Clasico,
    Deep,
    Flow,
    Personalizado,
}

impl SessionKind {
    pub const ALL: [Self; 4] = [Self::Clasico, Self::Deep, Self::Flow, Self::Personalizado];

    /// Identificador estable (CLI e historial).
    pub fn id(self) -> &'static str {
        match self {
            Self::Clasico => "clasico",
            Self::Deep => "deep",
            Self::Flow => "flow",
            Self::Personalizado => "personalizado",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Clasico => "Clásico",
            Self::Deep => "Deep work",
            Self::Flow => "Flow",
            Self::Personalizado => "Personalizado",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Duraciones fijas; `None` para Personalizado (sale de la configuración).
    pub fn preset(self) -> Option<Durations> {
        let m = |n: u64| Duration::from_secs(n * 60);
        match self {
            Self::Clasico => Some(Durations {
                focus: m(25),
                short_break: m(5),
                long: Some(LongBreak {
                    len: m(15),
                    every: 4,
                }),
            }),
            Self::Deep => Some(Durations {
                focus: m(50),
                short_break: m(10),
                long: None,
            }),
            Self::Flow => Some(Durations {
                focus: m(90),
                short_break: m(20),
                long: None,
            }),
            Self::Personalizado => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongBreak {
    pub len: Duration,
    /// Descanso largo tras este número de focos completados.
    pub every: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Durations {
    pub focus: Duration,
    pub short_break: Duration,
    pub long: Option<LongBreak>,
}

/// Valores editables de la sesión Personalizado, en minutos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Custom {
    pub focus_min: u32,
    pub break_min: u32,
    pub long_break_min: u32,
    pub cycles: u32,
}

impl Default for Custom {
    fn default() -> Self {
        Self {
            focus_min: 45,
            break_min: 10,
            long_break_min: 20,
            cycles: 3,
        }
    }
}

pub const FOCUS_MIN: (u32, u32) = (1, 180);
pub const BREAK_MIN: (u32, u32) = (1, 60);
pub const LONG_BREAK_MIN: (u32, u32) = (1, 90);
pub const CYCLES: (u32, u32) = (1, 12);

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{field} = {value} fuera de rango ({min}–{max})")]
pub struct RangeError {
    pub field: &'static str,
    pub value: u32,
    pub min: u32,
    pub max: u32,
}

fn check(field: &'static str, value: u32, (min, max): (u32, u32)) -> Result<u32, RangeError> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(RangeError {
            field,
            value,
            min,
            max,
        })
    }
}

impl Custom {
    pub fn validate(self) -> Result<Durations, RangeError> {
        let m = |n: u32| Duration::from_secs(u64::from(n) * 60);
        let focus = check("focus_min", self.focus_min, FOCUS_MIN)?;
        let brk = check("break_min", self.break_min, BREAK_MIN)?;
        let long = check("long_break_min", self.long_break_min, LONG_BREAK_MIN)?;
        let cycles = check("cycles", self.cycles, CYCLES)?;
        Ok(Durations {
            focus: m(focus),
            short_break: m(brk),
            long: Some(LongBreak {
                len: m(long),
                // CYCLES.1 = 12 cabe en u8.
                every: u8::try_from(cycles).unwrap_or(u8::MAX),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets() {
        let c = SessionKind::Clasico.preset();
        assert_eq!(c.map(|d| d.focus.as_secs()), Some(1500));
        assert_eq!(c.and_then(|d| d.long).map(|l| l.every), Some(4));
        assert_eq!(
            SessionKind::Deep.preset().map(|d| d.short_break.as_secs()),
            Some(600)
        );
        assert_eq!(
            SessionKind::Flow.preset().map(|d| d.focus.as_secs()),
            Some(5400)
        );
        assert_eq!(SessionKind::Personalizado.preset(), None);
    }

    #[test]
    fn ids_ida_y_vuelta() {
        for k in SessionKind::ALL {
            assert_eq!(SessionKind::from_id(k.id()), Some(k));
        }
        assert_eq!(SessionKind::from_id("otro"), None);
    }

    #[test]
    fn custom_limites() {
        let ok = Custom {
            focus_min: 1,
            break_min: 60,
            long_break_min: 90,
            cycles: 12,
        };
        assert!(ok.validate().is_ok());

        let mal = |c: Custom, field| assert_eq!(c.validate().map_err(|e| e.field), Err(field));
        mal(Custom { focus_min: 0, ..ok }, "focus_min");
        mal(
            Custom {
                focus_min: 181,
                ..ok
            },
            "focus_min",
        );
        mal(Custom { break_min: 0, ..ok }, "break_min");
        mal(
            Custom {
                break_min: 61,
                ..ok
            },
            "break_min",
        );
        mal(
            Custom {
                long_break_min: 91,
                ..ok
            },
            "long_break_min",
        );
        mal(Custom { cycles: 0, ..ok }, "cycles");
        mal(Custom { cycles: 13, ..ok }, "cycles");
    }

    #[test]
    fn custom_por_defecto_valido() {
        let d = Custom::default().validate();
        assert_eq!(d.map(|d| d.long.map(|l| l.every)), Ok(Some(3)));
    }
}
