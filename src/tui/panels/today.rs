//! Panel Hoy: meta diaria en porcentaje con barra, pomodoros y minutos de foco.

use ratatui::{
    Frame,
    layout::{Margin, Rect},
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

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = area.inner(Margin::new(1, 0));
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
    let detail: Vec<Span> = if d.pomodoros == 0 {
        vec![Span::styled(
            format!("Aún no hay pomodoros hoy · meta {}", d.goal),
            fg(DIM),
        )]
    } else {
        let p = if d.pomodoros == 1 {
            "pomodoro"
        } else {
            "pomodoros"
        };
        let mut v = vec![
            Span::styled(format!("{} ", d.pomodoros), bold(TEXT)),
            Span::styled(format!("{p} · "), fg(MUTED)),
            Span::styled(format!("{} ", d.minutes()), bold(TEXT)),
            Span::styled("min de foco", fg(MUTED)),
        ];
        if done {
            v.push(Span::styled("  ✓ meta cumplida", fg(OK)));
        }
        v
    };

    let label = Span::styled("Meta ", fg(MUTED));
    if inner.height >= 2 {
        let fixed = width(&[label.clone(), pct.clone(), count.clone()]);
        let mut first = vec![label];
        first.extend(blocks(d.frac(), w.saturating_sub(fixed), done));
        first.push(pct);
        first.push(count);
        let lines = vec![Line::from(first), Line::from(detail)];
        f.render_widget(Paragraph::new(lines), inner);
    } else {
        // Una sola línea: la barra se encoge para dejar sitio al resumen.
        let tail = Span::styled(format!(" · {} min", d.minutes()), fg(MUTED));
        let fixed = width(&[label.clone(), pct.clone(), count.clone(), tail.clone()]);
        let mut line = vec![label];
        line.extend(blocks(d.frac(), w.saturating_sub(fixed).min(20), done));
        line.extend([pct, count, tail]);
        f.render_widget(Paragraph::new(Line::from(line)), inner);
    }
}
