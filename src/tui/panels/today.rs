//! Panel Hoy: meta diaria (barra y un punto por pomodoro), foco, lo que falta y la semana.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    stats::{self, DaySummary},
    tui::{
        app::App,
        theme::{DIM, FAINT, MUTED, OK, TEXT, accent, bold, fg},
        ui::width,
    },
};

/// Barra de bloques `▰▱` de `w` celdas; verde al llegar a la meta.
pub fn blocks(frac: f64, w: usize, done: bool) -> Vec<Span<'static>> {
    let filled = (frac.clamp(0.0, 1.0) * w as f64).round() as usize;
    (0..w)
        .map(|i| {
            if i < filled {
                let c = if done {
                    OK
                } else {
                    accent(i as f64 / w.max(1) as f64)
                };
                Span::styled("▰", fg(c))
            } else {
                Span::styled("▱", fg(FAINT))
            }
        })
        .collect()
}

pub fn summary(app: &App) -> DaySummary {
    stats::day(&app.history, app.today, app.config.daily_goal)
}

const WEEKDAYS: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];
const SPARK: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Un punto por pomodoro de la meta: ● hechos (verde si se cumplió), ○ pendientes.
/// Los que pasan de la meta se suman como `+N`. `None` si no cabe ni compacto.
fn goal_dots(d: &DaySummary, max: usize) -> Option<Vec<Span<'static>>> {
    let goal = d.goal.max(1) as usize;
    let done = d.pomodoros as usize;
    let extra = done.saturating_sub(goal);
    let tail = if extra > 0 {
        format!(" +{extra}")
    } else {
        String::new()
    };
    let met = done >= goal;
    // Con espacios si caben; si no, pegados.
    for spaced in [true, false] {
        let w = if spaced { goal * 2 - 1 } else { goal } + tail.chars().count();
        if w > max {
            continue;
        }
        let mut v = Vec::new();
        for i in 0..goal {
            if spaced && i > 0 {
                v.push(Span::raw(" "));
            }
            v.push(if i < done {
                let c = if met {
                    OK
                } else {
                    accent(i as f64 / (goal.max(2) - 1) as f64)
                };
                Span::styled("●", fg(c))
            } else {
                Span::styled("○", fg(FAINT))
            });
        }
        if extra > 0 {
            v.push(Span::styled(tail, bold(OK)));
        }
        return Some(v);
    }
    None
}

fn minutes_text(m: u64) -> String {
    super::timer::human(std::time::Duration::from_secs(m * 60))
}

/// Línea de resumen: pomodoros, foco y lo que falta.
fn detail(d: &DaySummary) -> Vec<Span<'static>> {
    if d.pomodoros == 0 {
        return vec![Span::styled(
            format!("aún no hay pomodoros hoy · meta {}", d.goal),
            fg(DIM),
        )];
    }
    let mut v = vec![
        Span::styled(minutes_text(d.minutes()), bold(TEXT)),
        Span::styled(" de foco", fg(MUTED)),
    ];
    if d.pomodoros >= d.goal {
        v.push(Span::styled("  ✓ meta cumplida", bold(OK)));
    } else {
        let left = d.goal - d.pomodoros;
        v.push(Span::styled(
            format!("  ·  falta{} {left}", if left == 1 { "" } else { "n" }),
            fg(MUTED),
        ));
    }
    v
}

/// Semana en curso (lunes → domingo) en minibarras relativas a la meta.
fn week_line(app: &App, goal: u32, compact: bool) -> Line<'static> {
    let counts = stats::daily_counts(&app.history, app.today);
    let back = i64::from(app.today.weekday().to_monday_zero_offset());
    let mut spans = vec![Span::styled("semana ", fg(MUTED))];
    let mut total = 0;
    for (i, name) in WEEKDAYS.iter().enumerate() {
        let off = i as i64 - back;
        let day = app.today.checked_add(jiff::Span::new().days(off)).ok();
        let is_today = off == 0;
        let n = day.and_then(|d| counts.get(&d).copied()).unwrap_or(0);
        total += n;
        let label = if is_today { bold(TEXT) } else { fg(DIM) };
        let name = if compact {
            (*name).to_owned()
        } else {
            format!(" {name} ")
        };
        spans.push(Span::styled(name, label));
        if off > 0 {
            spans.push(Span::styled("·", fg(FAINT)));
        } else if n == 0 {
            spans.push(Span::styled("▁", fg(FAINT)));
        } else {
            let lvl = (n * 8).div_ceil(goal.max(1)).clamp(1, 8) as usize;
            let c = if n >= goal { OK } else { accent(0.3) };
            spans.push(Span::styled(SPARK[lvl - 1], fg(c)));
        }
    }
    let tail = if compact {
        format!("  {total}")
    } else {
        format!("   {total} esta semana")
    };
    spans.push(Span::styled(tail, fg(DIM)));
    Line::from(spans)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = crate::tui::ui::pad(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let d = summary(app);
    let done = d.pct() >= 100;
    let w = usize::from(inner.width);

    let pct = Span::styled(
        format!(" {:>3}%", d.pct()),
        bold(if done { OK } else { TEXT }),
    );
    let count = Span::styled(format!("  {}/{}", d.pomodoros, d.goal), fg(MUTED));
    let label = Span::styled("meta ", fg(MUTED));

    if inner.height >= 2 {
        let fixed = width(&[label.clone(), pct.clone(), count.clone()]);
        let mut first = vec![label];
        first.extend(blocks(d.frac(), w.saturating_sub(fixed), done));
        first.push(pct);
        first.push(count);
        let mut lines = vec![Line::from(first)];
        // Puntos de la meta + resumen; si no caben juntos, solo el resumen.
        let det = detail(&d);
        let room = w.saturating_sub(width(&det) + 3);
        let mut second = goal_dots(&d, room).unwrap_or_default();
        if !second.is_empty() {
            second.push(Span::raw("   "));
        }
        second.extend(det);
        lines.push(Line::from(second));
        if inner.height >= 3
            && let Some(week) = [false, true]
                .into_iter()
                .map(|c| week_line(app, d.goal, c))
                .find(|l| l.width() <= w)
        {
            lines.push(week);
        }
        f.render_widget(Paragraph::new(lines), inner);
    } else {
        // Una sola línea: la barra se encoge para dejar sitio al resumen.
        let tail = Span::styled(format!(" · {}", minutes_text(d.minutes())), fg(MUTED));
        let fixed = width(&[label.clone(), pct.clone(), count.clone(), tail.clone()]);
        let mut line = vec![label];
        line.extend(blocks(d.frac(), w.saturating_sub(fixed).min(20), done));
        line.extend([pct, count, tail]);
        f.render_widget(Paragraph::new(Line::from(line)), inner);
    }
}
