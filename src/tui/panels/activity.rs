//! Panel Racha y actividad: racha actual y mejor, heatmap estilo GitHub
//! (semanas en columnas, lunes→domingo en filas) con leyenda.
//!
//! Degradado por tamaño: sin fila de meses → sin leyenda → solo números.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    stats::{self, Week},
    tui::{
        app::App,
        theme::{DIM, MUTED, TEXT, accent, bold, fg, heat},
        ui::width,
    },
};

const DAY_LABELS: [&str; 7] = ["L", "M", "X", "J", "V", "S", "D"];
const MONTHS: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];
/// Columnas de la etiqueta del día ("L ").
const LABEL_W: usize = 2;
/// Cada semana ocupa una celda y un espacio.
const CELL_W: usize = 2;
pub const MAX_WEEKS: usize = 53;
/// Por debajo de esto el heatmap no se dibuja: solo los números.
const MIN_WEEKS: usize = 4;

/// Semanas que caben en `inner_w` columnas.
pub fn weeks_for(inner_w: usize) -> usize {
    (inner_w.saturating_sub(LABEL_W) / CELL_W).min(MAX_WEEKS)
}

fn days(n: u32) -> String {
    format!("{n} {}", if n == 1 { "día" } else { "días" })
}

fn streak_line(app: &App, s: stats::Streak) -> Line<'static> {
    let mut spans = vec![
        Span::styled("Racha ", fg(MUTED)),
        Span::styled(
            days(s.current),
            bold(if s.current > 0 { accent(0.3) } else { DIM }),
        ),
        Span::styled("  ·  mejor ", fg(MUTED)),
        Span::styled(days(s.best), bold(TEXT)),
    ];
    if s.current == 0 && s.best == 0 {
        spans = vec![Span::styled(
            "Sin racha todavía · completa un pomodoro para empezar",
            fg(DIM),
        )];
    } else if app.today_count() == 0 && s.current > 0 {
        spans.push(Span::styled("  · haz uno hoy para seguir", fg(DIM)));
    }
    Line::from(spans)
}

fn month_row(grid: &[Week]) -> Line<'static> {
    let mut s = " ".repeat(LABEL_W);
    let mut last_month = None;
    for (i, col) in grid.iter().enumerate() {
        let Some((d, _)) = col[0] else { continue };
        let m = d.month();
        if last_month != Some(m) {
            let x = LABEL_W + i * CELL_W;
            let name = MONTHS[usize::try_from(m - 1).unwrap_or(0) % 12];
            // Solo si no pisa la etiqueta anterior y cabe en el ancho del grid.
            if s.chars().count() <= x && x + name.len() <= LABEL_W + grid.len() * CELL_W {
                s.push_str(&" ".repeat(x - s.chars().count()));
                s.push_str(name);
            }
            last_month = Some(m);
        }
    }
    Line::styled(s, fg(DIM))
}

fn grid_rows(grid: &[Week], goal: u32) -> Vec<Line<'static>> {
    (0..7)
        .map(|row| {
            let mut spans = vec![Span::styled(format!("{} ", DAY_LABELS[row]), fg(DIM))];
            for col in grid {
                match col[row] {
                    Some((_, n)) => {
                        spans.push(Span::styled("■", fg(heat(stats::level(n, goal)))));
                        spans.push(Span::raw(" "));
                    }
                    None => spans.push(Span::raw("  ")),
                }
            }
            Line::from(spans)
        })
        .collect()
}

fn legend(total: u32, weeks: usize, w: usize) -> Line<'static> {
    let mut right = vec![Span::styled("menos ", fg(DIM))];
    for l in 0..=4 {
        right.push(Span::styled("■", fg(heat(l))));
        right.push(Span::raw(" "));
    }
    right.push(Span::styled("más", fg(DIM)));
    // Resumen completo, corto o ninguno; la leyenda siempre a la derecha.
    let left = [
        format!("{total} pomodoros · {weeks} semanas"),
        format!("{total} en {weeks} sem."),
        String::new(),
    ]
    .into_iter()
    .map(|t| vec![Span::styled(t, fg(MUTED))])
    .find(|l| width(l) + width(&right) + 2 <= w)
    .unwrap_or_default();
    let gap = w.saturating_sub(width(&left) + width(&right));
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    Line::from(spans)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = crate::tui::ui::pad(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let counts = stats::daily_counts(&app.history, app.today);
    let s = stats::streak(&counts, app.today);
    let mut lines = vec![streak_line(app, s)];

    let w = usize::from(inner.width);
    let h = usize::from(inner.height);
    let weeks = weeks_for(w);
    // Mínimo: racha + 7 filas.
    if weeks >= MIN_WEEKS && h >= 8 {
        let grid = stats::heat_grid(&counts, app.today, weeks);
        let total: u32 = grid
            .iter()
            .flat_map(|c| c.iter().flatten().map(|(_, n)| *n))
            .sum();
        let with_months = h >= 10;
        let with_legend = h >= 9;
        if with_months {
            lines.push(month_row(&grid));
        }
        lines.extend(grid_rows(&grid, app.config.daily_goal));
        if with_legend {
            lines.push(legend(total, weeks, w));
        }
    } else if h >= 2 {
        lines.push(Line::styled(
            "amplía la terminal para ver el heatmap",
            fg(DIM),
        ));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
