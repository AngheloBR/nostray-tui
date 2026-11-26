//! Panel Distribución: barras horizontales por categoría (hoy / últimos 7 días).

use jiff::Span as JSpan;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::today::blocks;
use crate::{
    stats::{self, Share},
    tui::{
        app::{App, DistRange},
        theme::{DIM, MUTED, TEXT, bold, fg},
        ui::{ellipsize, width},
    },
};

pub fn title(app: &App) -> String {
    match app.dist_range {
        DistRange::Today => "distribución · hoy".to_owned(),
        DistRange::Week => "distribución · 7 días".to_owned(),
    }
}

pub fn shares(app: &App) -> Vec<Share> {
    let to = app.today;
    let from = match app.dist_range {
        DistRange::Today => to,
        DistRange::Week => to.checked_sub(JSpan::new().days(6)).unwrap_or(to),
    };
    stats::distribution(&app.history, from, to, &app.config.categories)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = crate::tui::ui::pad(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let shares = shares(app);
    if shares.is_empty() {
        let msg = match app.dist_range {
            DistRange::Today => "Sin foco registrado hoy",
            DistRange::Week => "Sin foco en los últimos 7 días",
        };
        f.render_widget(Paragraph::new(Line::styled(msg, fg(DIM))), inner);
        return;
    }

    let w = usize::from(inner.width);
    let rows = usize::from(inner.height);
    // Si no caben todas, la última fila resume las restantes.
    let (show, rest) = if shares.len() > rows {
        (
            rows.saturating_sub(1),
            shares.len() - rows.saturating_sub(1),
        )
    } else {
        (shares.len(), 0)
    };
    let name_w = shares
        .iter()
        .take(show)
        .map(|s| Span::raw(s.category.as_str()).width())
        .max()
        .unwrap_or(0)
        .min(w / 3)
        .max(4);

    let mut lines: Vec<Line> = Vec::new();
    for s in shares.iter().take(show) {
        let name = ellipsize(&s.category, name_w);
        let pad = name_w.saturating_sub(Span::raw(name.as_str()).width());
        let tail = [
            Span::styled(format!(" {:>3}%", s.pct), bold(TEXT)),
            Span::styled(format!(" {:>5} min", s.minutes()), fg(MUTED)),
        ];
        let head = Span::styled(format!("{name}{} ", " ".repeat(pad)), fg(TEXT));
        let bar_w = w.saturating_sub(head.width() + width(&tail));
        let mut spans = vec![head];
        spans.extend(blocks(f64::from(s.pct) / 100.0, bar_w, false));
        spans.extend(tail);
        lines.push(Line::from(spans));
    }
    if rest > 0 {
        lines.push(Line::styled(format!("+{rest} más"), fg(DIM)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
