//! Panel Log: una línea por fase cerrada (hora, sesión, categoría, tarea, duración),
//! la más reciente arriba; desplazable con el panel enfocado.

use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    model::CycleRecord,
    session::SessionKind,
    timer::Phase,
    tui::{
        app::App,
        panels::timer::mmss,
        theme::{DIM, FAINT, MUTED, OK, TEXT, WARN, fg},
        ui::{ellipsize, width},
    },
};

fn when(r: &CycleRecord, app: &App) -> String {
    let z = r.ended_at.to_zoned_datetime(app.tz());
    if r.local_date == app.today {
        z.strftime("%H:%M").to_string()
    } else {
        z.strftime("%d/%m %H:%M").to_string()
    }
}

fn line(r: &CycleRecord, app: &App, w: usize) -> Line<'static> {
    let session = SessionKind::from_id(&r.session)
        .map_or_else(|| r.session.clone(), |k| k.label().to_owned());
    let (what, what_style) = match r.phase {
        Phase::Focus => (
            r.category.clone().unwrap_or_else(|| "—".to_owned()),
            fg(TEXT),
        ),
        Phase::ShortBreak => ("descanso".to_owned(), fg(MUTED)),
        Phase::LongBreak => ("descanso largo".to_owned(), fg(MUTED)),
    };
    let dur = std::time::Duration::from_secs(r.actual_secs);
    let (mark, mark_style) = if r.completed {
        (" ✓", fg(OK))
    } else {
        (" saltado", fg(WARN))
    };
    let head = vec![
        Span::styled(format!("{:<11} ", when(r, app)), fg(DIM)),
        Span::styled(format!("{session:<13} "), fg(MUTED)),
        Span::styled(format!("{} ", ellipsize(&what, 14)), what_style),
    ];
    let tail = vec![
        Span::styled(format!(" {:>6}", mmss(dur)), fg(TEXT)),
        Span::styled(mark, mark_style),
    ];
    let room = w.saturating_sub(width(&head) + width(&tail));
    let task = r
        .task
        .as_deref()
        .map(|t| format!("· {t}"))
        .unwrap_or_default();
    let task = ellipsize(&task, room);
    let pad = room.saturating_sub(Span::raw(task.as_str()).width());
    let mut spans = head;
    spans.push(Span::styled(task, fg(MUTED)));
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(tail);
    Line::from(spans)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = area.inner(Margin::new(1, 0));
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let total = app.history.cycles.len();
    if total == 0 {
        let msg = Line::styled("Aquí aparecerá cada ciclo al cerrarse.", fg(DIM));
        f.render_widget(Paragraph::new(msg), inner);
        return;
    }
    let w = usize::from(inner.width);
    let rows = usize::from(inner.height);
    let skip = app.log_scroll.min(total.saturating_sub(1));
    // Si no cabe todo, la última fila es el indicador de posición.
    let overflow = total > rows && rows >= 2;
    let take = if overflow { rows - 1 } else { rows };
    let mut lines: Vec<Line> = app
        .history
        .cycles
        .iter()
        .rev()
        .skip(skip)
        .take(take)
        .map(|r| line(r, app, w))
        .collect();
    if overflow {
        let shown = lines.len();
        let pos = format!("{}–{} de {total}", skip + 1, skip + shown);
        lines.push(Line::styled(pos, fg(FAINT)).alignment(Alignment::Right));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
