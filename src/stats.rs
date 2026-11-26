//! Estadísticas puras sobre el historial: hoy/meta, distribución por categoría,
//! racha y heatmap. Solo cuentan los focos completados (`CycleRecord::is_pomodoro`).

use std::collections::BTreeMap;

use jiff::{Span, civil::Date};

use crate::model::History;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaySummary {
    pub pomodoros: u32,
    pub focus_secs: u64,
    pub goal: u32,
}

impl DaySummary {
    /// Porcentaje de la meta, redondeado hacia abajo; puede pasar de 100.
    pub fn pct(&self) -> u32 {
        if self.goal == 0 {
            return 0;
        }
        self.pomodoros.saturating_mul(100) / self.goal
    }

    /// Fracción de la meta para la barra, en `0.0..=1.0`.
    pub fn frac(&self) -> f64 {
        if self.goal == 0 {
            return 0.0;
        }
        (f64::from(self.pomodoros) / f64::from(self.goal)).clamp(0.0, 1.0)
    }

    pub fn minutes(&self) -> u64 {
        round_min(self.focus_secs)
    }
}

pub fn day(history: &History, date: Date, goal: u32) -> DaySummary {
    let (pomodoros, focus_secs) = history
        .cycles
        .iter()
        .filter(|r| r.is_pomodoro() && r.local_date == date)
        .fold((0u32, 0u64), |(n, s), r| {
            (n.saturating_add(1), s.saturating_add(r.actual_secs))
        });
    DaySummary {
        pomodoros,
        focus_secs,
        goal,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    pub category: String,
    pub secs: u64,
    pub pct: u32,
}

impl Share {
    pub fn minutes(&self) -> u64 {
        round_min(self.secs)
    }
}

fn round_min(secs: u64) -> u64 {
    secs.saturating_add(30) / 60
}

/// Minutos de foco por categoría entre `from` y `to` (inclusive).
///
/// Porcentajes por resto mayor: siempre suman 100 si hay datos.
/// Orden: más tiempo primero; en empate, el orden de `order` (la config) y
/// después las categorías que ya no están en la config, por nombre.
pub fn distribution(history: &History, from: Date, to: Date, order: &[String]) -> Vec<Share> {
    let mut acc: Vec<(String, u64)> = Vec::new();
    for r in &history.cycles {
        if !r.is_pomodoro() || r.local_date < from || r.local_date > to {
            continue;
        }
        let Some(cat) = &r.category else { continue };
        match acc.iter_mut().find(|(c, _)| c == cat) {
            Some((_, s)) => *s = s.saturating_add(r.actual_secs),
            None => acc.push((cat.clone(), r.actual_secs)),
        }
    }
    let total: u64 = acc.iter().map(|(_, s)| *s).sum();
    if total == 0 {
        return Vec::new();
    }

    let rank = |c: &str| order.iter().position(|o| o == c).unwrap_or(usize::MAX);
    acc.sort_by(|(a, sa), (b, sb)| {
        sb.cmp(sa)
            .then_with(|| rank(a).cmp(&rank(b)))
            .then_with(|| a.cmp(b))
    });

    // Resto mayor sobre enteros: floor(100·s/T) y se reparte lo que falta.
    let mut shares: Vec<(Share, u64)> = acc
        .into_iter()
        .map(|(category, secs)| {
            let num = u128::from(secs) * 100;
            let t = u128::from(total);
            let pct = u32::try_from(num / t).unwrap_or(100);
            let rem = u64::try_from(num % t).unwrap_or(0);
            (
                Share {
                    category,
                    secs,
                    pct,
                },
                rem,
            )
        })
        .collect();
    let given: u32 = shares.iter().map(|(s, _)| s.pct).sum();
    let mut missing = 100u32.saturating_sub(given);
    let mut by_rem: Vec<usize> = (0..shares.len()).collect();
    // Orden estable: en empate de resto gana el que va primero en la lista.
    by_rem.sort_by(|&a, &b| shares[b].1.cmp(&shares[a].1));
    for i in by_rem {
        if missing == 0 {
            break;
        }
        if shares[i].1 > 0 {
            shares[i].0.pct += 1;
            missing -= 1;
        }
    }
    shares.into_iter().map(|(s, _)| s).collect()
}

/// Pomodoros y segundos de foco entre `from` y `to` (inclusive).
pub fn range(history: &History, from: Date, to: Date) -> (u32, u64) {
    history
        .cycles
        .iter()
        .filter(|r| r.is_pomodoro() && r.local_date >= from && r.local_date <= to)
        .fold((0u32, 0u64), |(n, s), r| {
            (n.saturating_add(1), s.saturating_add(r.actual_secs))
        })
}

/// Resumen en texto plano para `nostray stats` (sin colores ni secuencias ANSI).
pub fn text_report(history: &History, cfg: &crate::model::Config, today: Date) -> String {
    use std::fmt::Write as _;

    let d = day(history, today, cfg.daily_goal);
    let week_from = today.checked_sub(Span::new().days(6)).unwrap_or(today);
    let (wn, ws) = range(history, week_from, today);
    let shares = distribution(history, week_from, today, &cfg.categories);
    let s = streak(&daily_counts(history, today), today);
    let plural = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} · resumen al {}",
        crate::APP_NAME,
        today.strftime("%d-%m-%Y")
    );
    let _ = writeln!(
        out,
        "Hoy      {}/{} pomodoros ({} %) · {} min de foco",
        d.pomodoros,
        d.goal,
        d.pct(),
        d.minutes()
    );
    let _ = writeln!(
        out,
        "7 días   {} · {} min de foco",
        plural(wn, "pomodoro", "pomodoros"),
        round_min(ws)
    );
    let name_w = shares
        .iter()
        .map(|s| s.category.chars().count())
        .max()
        .unwrap_or(0);
    for sh in &shares {
        let pad = name_w.saturating_sub(sh.category.chars().count());
        let _ = writeln!(
            out,
            "         {}{}  {:>3} %  {:>5} min",
            sh.category,
            " ".repeat(pad),
            sh.pct,
            sh.minutes()
        );
    }
    let _ = writeln!(
        out,
        "Racha    {} · mejor {}",
        plural(s.current, "día", "días"),
        plural(s.best, "día", "días")
    );
    out
}

// ── Racha y actividad ───────────────────────────────────────────────────────

/// Pomodoros por día local. Ignora fechas posteriores a `today` (reloj adelantado).
pub fn daily_counts(history: &History, today: Date) -> BTreeMap<Date, u32> {
    let mut m = BTreeMap::new();
    for r in history.cycles.iter().filter(|r| r.is_pomodoro()) {
        if r.local_date <= today {
            let n: &mut u32 = m.entry(r.local_date).or_default();
            *n = n.saturating_add(1);
        }
    }
    m
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Streak {
    pub current: u32,
    pub best: u32,
}

/// Días consecutivos con ≥ 1 pomodoro. La racha actual sigue viva hasta que
/// termina el día: si hoy aún no hay, se cuenta desde ayer.
pub fn streak(counts: &BTreeMap<Date, u32>, today: Date) -> Streak {
    let mut best = 0u32;
    let mut run = 0u32;
    let mut prev: Option<Date> = None;
    for &d in counts.keys() {
        run = match prev {
            Some(p) if p.tomorrow().ok() == Some(d) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(d);
    }

    let start = if counts.contains_key(&today) {
        Some(today)
    } else {
        today.yesterday().ok().filter(|y| counts.contains_key(y))
    };
    let mut current = 0u32;
    let mut d = start;
    while let Some(day) = d.filter(|x| counts.contains_key(x)) {
        current += 1;
        d = day.yesterday().ok();
    }
    Streak { current, best }
}

/// Nivel de intensidad 0–4 relativo a la meta diaria (con meta 8: 1–2, 3–4, 5–6, 7+).
pub fn level(n: u32, goal: u32) -> u8 {
    if n == 0 {
        return 0;
    }
    let goal = goal.max(1);
    let l = n.saturating_mul(4).div_ceil(goal).clamp(1, 4);
    u8::try_from(l).unwrap_or(4)
}

/// Una columna del heatmap: lunes→domingo; `None` = día futuro.
pub type Week = [Option<(Date, u32)>; 7];

/// Las últimas `weeks` semanas (lunes a domingo) terminando en la de `today`,
/// de la más antigua a la actual.
pub fn heat_grid(counts: &BTreeMap<Date, u32>, today: Date, weeks: usize) -> Vec<Week> {
    let offset = i64::from(today.weekday().to_monday_zero_offset());
    let back = i64::try_from(weeks.saturating_sub(1)).unwrap_or(0) * 7 + offset;
    let Ok(first) = today.checked_sub(Span::new().days(back)) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(weeks);
    let mut day = Some(first);
    for _ in 0..weeks {
        let mut col: Week = [None; 7];
        for cell in &mut col {
            if let Some(d) = day {
                if d <= today {
                    *cell = Some((d, counts.get(&d).copied().unwrap_or(0)));
                }
                day = d.tomorrow().ok();
            }
        }
        out.push(col);
    }
    out
}

#[cfg(test)]
pub mod fixtures {
    use jiff::{Timestamp, civil::Date};

    use crate::{
        model::{CycleRecord, FORMAT_VERSION, History},
        timer::Phase,
    };

    pub fn rec(date: Date, cat: &str, mins: u64, phase: Phase, completed: bool) -> CycleRecord {
        CycleRecord {
            ended_at: Timestamp::UNIX_EPOCH,
            local_date: date,
            phase,
            session: "clasico".into(),
            category: (phase == Phase::Focus).then(|| cat.to_owned()),
            task_id: None,
            task: None,
            planned_secs: mins * 60,
            actual_secs: mins * 60,
            completed,
        }
    }

    pub fn pomo(date: Date, cat: &str, mins: u64) -> CycleRecord {
        rec(date, cat, mins, Phase::Focus, true)
    }

    pub fn history(cycles: Vec<CycleRecord>) -> History {
        History {
            version: FORMAT_VERSION,
            cycles,
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::{fixtures::*, *};
    use crate::{model::DEFAULT_CATEGORIES, timer::Phase};

    fn order() -> Vec<String> {
        DEFAULT_CATEGORIES.map(str::to_owned).to_vec()
    }

    #[test]
    fn dia_sin_datos() {
        let d = day(&History::default(), date(2026, 3, 14), 8);
        assert_eq!(
            (d.pomodoros, d.focus_secs, d.pct(), d.minutes()),
            (0, 0, 0, 0)
        );
        assert_eq!(d.frac(), 0.0);
    }

    #[test]
    fn dia_cuenta_solo_focos_completos_de_esa_fecha() {
        let hoy = date(2026, 3, 14);
        let h = history(vec![
            pomo(hoy, "Programar", 25),
            pomo(hoy, "Leer", 25),
            pomo(hoy, "Leer", 25),
            rec(hoy, "Leer", 7, Phase::Focus, false), // saltado
            rec(hoy, "", 5, Phase::ShortBreak, true), // descanso
            pomo(date(2026, 3, 13), "Programar", 25), // ayer
        ]);
        let d = day(&h, hoy, 8);
        assert_eq!(d.pomodoros, 3);
        assert_eq!(d.minutes(), 75);
        assert_eq!(d.pct(), 37, "3/8 = 37,5 % → 37");
    }

    #[test]
    fn meta_superada_y_division_por_cero() {
        let d = DaySummary {
            pomodoros: 10,
            focus_secs: 0,
            goal: 8,
        };
        assert_eq!(d.pct(), 125);
        assert_eq!(d.frac(), 1.0);
        let z = DaySummary {
            pomodoros: 3,
            focus_secs: 0,
            goal: 0,
        };
        assert_eq!((z.pct(), z.frac()), (0, 0.0));
    }

    #[test]
    fn distribucion_vacia() {
        let d = distribution(
            &History::default(),
            date(2026, 3, 8),
            date(2026, 3, 14),
            &order(),
        );
        assert!(d.is_empty());
        // Solo descansos y saltados: también vacía (sin dividir por cero).
        let h = history(vec![
            rec(date(2026, 3, 14), "", 5, Phase::ShortBreak, true),
            rec(date(2026, 3, 14), "Leer", 3, Phase::Focus, false),
        ]);
        assert!(distribution(&h, date(2026, 3, 14), date(2026, 3, 14), &order()).is_empty());
    }

    #[test]
    fn distribucion_suma_100_con_terciles() {
        let hoy = date(2026, 3, 14);
        let h = history(vec![
            pomo(hoy, "Programar", 25),
            pomo(hoy, "Inglés", 25),
            pomo(hoy, "Leer", 25),
        ]);
        let d = distribution(&h, hoy, hoy, &order());
        let pcts: Vec<u32> = d.iter().map(|s| s.pct).collect();
        assert_eq!(pcts.iter().sum::<u32>(), 100);
        // Empate total: manda el orden de la config y el +1 va al primero.
        let cats: Vec<&str> = d.iter().map(|s| s.category.as_str()).collect();
        assert_eq!(cats, ["Programar", "Inglés", "Leer"]);
        assert_eq!(pcts, [34, 33, 33]);
    }

    #[test]
    fn distribucion_ordena_por_tiempo_y_redondea_minutos() {
        let hoy = date(2026, 3, 14);
        let h = history(vec![
            pomo(hoy, "Leer", 25),
            pomo(hoy, "Programar", 50),
            pomo(hoy, "Programar", 25),
        ]);
        let d = distribution(&h, hoy, hoy, &order());
        assert_eq!(d[0].category, "Programar");
        assert_eq!((d[0].minutes(), d[0].pct), (75, 75));
        assert_eq!((d[1].minutes(), d[1].pct), (25, 25));
    }

    #[test]
    fn ventana_7_dias_cruza_anio_nuevo() {
        let hoy = date(2027, 1, 3);
        let desde = hoy.checked_sub(jiff::Span::new().days(6)).unwrap_or(hoy);
        assert_eq!(desde, date(2026, 12, 28));
        let h = history(vec![
            pomo(date(2026, 12, 27), "Leer", 25), // fuera
            pomo(date(2026, 12, 28), "Leer", 25), // dentro (borde)
            pomo(date(2026, 12, 31), "Estudiar", 50),
            pomo(date(2027, 1, 3), "Estudiar", 25), // dentro (borde)
            pomo(date(2027, 1, 4), "Leer", 25),     // futuro: fuera
        ]);
        let d = distribution(&h, desde, hoy, &order());
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].category.as_str(), d[0].minutes()), ("Estudiar", 75));
        assert_eq!((d[1].category.as_str(), d[1].minutes()), ("Leer", 25));
    }

    #[test]
    fn categoria_eliminada_de_la_config_sigue_contando() {
        let hoy = date(2026, 3, 14);
        let h = history(vec![pomo(hoy, "Música", 25), pomo(hoy, "Leer", 25)]);
        let d = distribution(&h, hoy, hoy, &order());
        let cats: Vec<&str> = d.iter().map(|s| s.category.as_str()).collect();
        assert_eq!(
            cats,
            ["Leer", "Música"],
            "empate: primero lo que está en la config"
        );
        assert_eq!(d.iter().map(|s| s.pct).sum::<u32>(), 100);
    }

    fn counts_of(days: &[Date]) -> BTreeMap<Date, u32> {
        let h = history(days.iter().map(|d| pomo(*d, "Leer", 25)).collect());
        daily_counts(&h, date(2100, 1, 1))
    }

    #[test]
    fn racha_sin_datos() {
        assert_eq!(
            streak(&BTreeMap::new(), date(2026, 3, 14)),
            Streak::default()
        );
    }

    #[test]
    fn racha_incluye_hoy() {
        let c = counts_of(&[date(2026, 3, 12), date(2026, 3, 13), date(2026, 3, 14)]);
        assert_eq!(
            streak(&c, date(2026, 3, 14)),
            Streak {
                current: 3,
                best: 3
            }
        );
    }

    #[test]
    fn racha_sigue_viva_si_termina_ayer() {
        let c = counts_of(&[date(2026, 3, 12), date(2026, 3, 13)]);
        assert_eq!(streak(&c, date(2026, 3, 14)).current, 2);
        // Anteayer ya no: se rompió.
        assert_eq!(streak(&c, date(2026, 3, 15)).current, 0);
        assert_eq!(streak(&c, date(2026, 3, 15)).best, 2);
    }

    #[test]
    fn hueco_rompe_y_mejor_racha_se_conserva() {
        let c = counts_of(&[
            date(2026, 3, 1),
            date(2026, 3, 2),
            date(2026, 3, 3),
            date(2026, 3, 4), // 4 seguidos
            date(2026, 3, 6), // hueco el 5
            date(2026, 3, 7),
        ]);
        assert_eq!(
            streak(&c, date(2026, 3, 7)),
            Streak {
                current: 2,
                best: 4
            }
        );
    }

    #[test]
    fn racha_cruza_anio_nuevo_y_bisiesto() {
        let c = counts_of(&[date(2026, 12, 30), date(2026, 12, 31), date(2027, 1, 1)]);
        assert_eq!(
            streak(&c, date(2027, 1, 1)),
            Streak {
                current: 3,
                best: 3
            }
        );
        let c = counts_of(&[date(2028, 2, 28), date(2028, 2, 29), date(2028, 3, 1)]);
        assert_eq!(streak(&c, date(2028, 3, 1)).current, 3);
        // 2027 no es bisiesto: 28-feb → 1-mar es consecutivo.
        let c = counts_of(&[date(2027, 2, 28), date(2027, 3, 1)]);
        assert_eq!(streak(&c, date(2027, 3, 1)).current, 2);
    }

    #[test]
    fn varios_por_dia_y_saltados_no_cuentan() {
        let hoy = date(2026, 3, 14);
        let h = history(vec![
            pomo(hoy, "Leer", 25),
            pomo(hoy, "Leer", 25),
            rec(date(2026, 3, 13), "Leer", 10, Phase::Focus, false),
            pomo(date(2026, 3, 20), "Leer", 25), // futuro: se ignora
        ]);
        let c = daily_counts(&h, hoy);
        assert_eq!(c.get(&hoy), Some(&2));
        assert_eq!(c.len(), 1);
        assert_eq!(
            streak(&c, hoy),
            Streak {
                current: 1,
                best: 1
            }
        );
    }

    #[test]
    fn niveles_relativos_a_la_meta() {
        let got: Vec<u8> = (0..=9).map(|n| level(n, 8)).collect();
        assert_eq!(got, [0, 1, 1, 2, 2, 3, 3, 4, 4, 4]);
        assert_eq!(level(1, 1), 4);
        assert_eq!(level(1, 24), 1);
        assert_eq!(level(3, 0), 4, "meta 0 no divide por cero");
        assert_eq!(level(u32::MAX, 8), 4);
    }

    #[test]
    fn grid_alinea_semanas_y_deja_futuro_vacio() {
        // 2026-03-14 es sábado.
        let hoy = date(2026, 3, 14);
        let c = counts_of(&[hoy, date(2026, 3, 9)]);
        let g = heat_grid(&c, hoy, 3);
        assert_eq!(g.len(), 3);
        assert_eq!(
            g[0][0].map(|x| x.0),
            Some(date(2026, 2, 23)),
            "lunes, 2 semanas antes"
        );
        let last = g[2];
        assert_eq!(last[0], Some((date(2026, 3, 9), 1)));
        assert_eq!(last[5], Some((hoy, 1)));
        assert_eq!(last[6], None, "domingo 15 es futuro");
        assert_eq!(last[1], Some((date(2026, 3, 10), 0)));
    }

    #[test]
    fn grid_cruza_anio_y_hoy_lunes() {
        let hoy = date(2027, 1, 4); // lunes
        let g = heat_grid(&BTreeMap::new(), hoy, 2);
        assert_eq!(g[0][0].map(|x| x.0), Some(date(2026, 12, 28)));
        assert_eq!(g[0][6].map(|x| x.0), Some(date(2027, 1, 3)));
        assert_eq!(g[1][0], Some((hoy, 0)));
        assert!(g[1][1..].iter().all(Option::is_none));
        assert!(heat_grid(&BTreeMap::new(), hoy, 0).is_empty());
    }

    #[test]
    fn informe_de_texto() {
        let hoy = date(2026, 3, 14);
        let h = history(vec![
            pomo(hoy, "Programar", 25),
            pomo(hoy, "Inglés", 25),
            pomo(date(2026, 3, 13), "Programar", 50),
            pomo(date(2026, 3, 1), "Leer", 25), // fuera de los 7 días
        ]);
        let cfg = crate::model::Config::default();
        let t = text_report(&h, &cfg, hoy);
        assert_eq!(
            t,
            "nostray · resumen al 14-03-2026\n\
             Hoy      2/8 pomodoros (25 %) · 50 min de foco\n\
             7 días   3 pomodoros · 100 min de foco\n\
             \x20        Programar   75 %     75 min\n\
             \x20        Inglés      25 %     25 min\n\
             Racha    2 días · mejor 2 días\n"
        );
        assert!(!t.contains('\x1b'), "sin ANSI");
    }

    #[test]
    fn informe_sin_datos() {
        let t = text_report(
            &History::default(),
            &crate::model::Config::default(),
            date(2026, 3, 14),
        );
        assert!(
            t.contains("Hoy      0/8 pomodoros (0 %) · 0 min de foco"),
            "{t}"
        );
        assert!(t.contains("7 días   0 pomodoros · 0 min de foco"), "{t}");
        assert!(t.contains("Racha    0 días · mejor 0 días"), "{t}");
    }

    #[test]
    fn resto_mayor_con_muchas_categorias() {
        let hoy = date(2026, 3, 14);
        let mut v = Vec::new();
        for (c, m) in [
            ("A", 1),
            ("B", 1),
            ("C", 1),
            ("D", 1),
            ("E", 1),
            ("F", 1),
            ("G", 1),
        ] {
            v.push(pomo(hoy, c, m));
        }
        let d = distribution(&history(v), hoy, hoy, &order());
        assert_eq!(d.iter().map(|s| s.pct).sum::<u32>(), 100);
        assert!(d.iter().all(|s| s.pct == 14 || s.pct == 15), "{d:?}");
    }
}
