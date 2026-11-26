//! Panel Log: una línea por fase cerrada (hora, sesión, categoría, tarea, duración),
//! la más reciente arriba; desplazable con el panel enfocado.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
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
        theme::{DIM, FAINT, MUTED, OK, TEXT, WARN, bold, fg},
        ui::ellipsize,
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

/// Anchos de columna según el ancho disponible.
struct Cols {
    when: usize,
    what: usize,
    /// 0 = sin columna de sesión.
    session: usize,
}

const DUR_W: usize = 6; // " 25:00"
const MARK_W: usize = 2; // " ✓"

impl Cols {
    fn new(w: usize, any_old: bool) -> Self {
        let when = if any_old { 11 } else { 5 };
        let what = 14;
        let session = if w >= when + what + 10 + 12 + DUR_W + MARK_W + 3 {
            10
        } else {
            0
        };
        Self {
            when,
            what,
            session,
        }
    }

    fn task(&self, w: usize) -> usize {
        let used = self.when + 1 + self.what + 1 + DUR_W + MARK_W;
        let used = used
            + if self.session > 0 {
                self.session + 1
            } else {
                0
            };
        // Una columna de separación tras la tarea.
        w.saturating_sub(used + 1)
    }
}

fn cell(text: &str, w: usize) -> String {
    let t = ellipsize(text, w);
    let pad = w.saturating_sub(Span::raw(t.as_str()).width());
    format!("{t}{}", " ".repeat(pad))
}

fn header(c: &Cols, w: usize) -> Line<'static> {
    let st = bold(DIM);
    let mut spans = vec![
        Span::styled(cell("hora", c.when) + " ", st),
        Span::styled(cell("qué", c.what) + " ", st),
        Span::styled(cell("tarea", c.task(w)) + " ", st),
    ];
    if c.session > 0 {
        spans.push(Span::styled(cell("sesión", c.session) + " ", st));
    }
    spans.push(Span::styled(format!("{:>DUR_W$}", "dur."), st));
    spans.push(Span::raw(" ".repeat(MARK_W)));
    Line::from(spans)
}

fn line(r: &CycleRecord, app: &App, c: &Cols, w: usize) -> Line<'static> {
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
        (" ↷", fg(WARN))
    };
    let mut spans = vec![
        Span::styled(cell(&when(r, app), c.when) + " ", fg(DIM)),
        Span::styled(cell(&what, c.what) + " ", what_style),
        Span::styled(
            cell(r.task.as_deref().unwrap_or(""), c.task(w)) + " ",
            fg(MUTED),
        ),
    ];
    if c.session > 0 {
        let session = SessionKind::from_id(&r.session)
            .map_or_else(|| r.session.clone(), |k| k.label().to_owned());
        spans.push(Span::styled(cell(&session, c.session) + " ", fg(DIM)));
    }
    let dur_style = if r.completed { fg(TEXT) } else { fg(DIM) };
    spans.push(Span::styled(format!("{:>DUR_W$}", mmss(dur)), dur_style));
    spans.push(Span::styled(mark, mark_style));
    Line::from(spans)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = crate::tui::ui::pad(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let total = app.history.cycles.len();
    if total == 0 {
        let lines = vec![
            Line::styled("Aquí aparecerá cada ciclo al cerrarse.", fg(DIM)),
            Line::styled("✓ completado · ↷ saltado", fg(FAINT)),
        ];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }
    let w = usize::from(inner.width);
    let mut rows = usize::from(inner.height);
    let skip = app.log_scroll.min(total.saturating_sub(1));
    // Encabezado de columnas si hay sitio para él y al menos dos filas.
    let with_header = rows >= 3;
    if with_header {
        rows -= 1;
    }
    // Si no cabe todo, la última fila es el indicador de posición.
    let overflow = total > rows && rows >= 2;
    let take = if overflow { rows - 1 } else { rows };
    let shown: Vec<&CycleRecord> = app
        .history
        .cycles
        .iter()
        .rev()
        .skip(skip)
        .take(take)
        .collect();
    let any_old = shown.iter().any(|r| r.local_date != app.today);
    let cols = Cols::new(w, any_old);
    let mut lines = Vec::new();
    if with_header {
        lines.push(header(&cols, w));
    }
    lines.extend(shown.iter().map(|r| line(r, app, &cols, w)));
    if overflow {
        let n = shown.len();
        let pos = format!("{}–{} de {total} · ↑↓", skip + 1, skip + n);
        lines.push(Line::styled(pos, fg(FAINT)).alignment(Alignment::Right));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
