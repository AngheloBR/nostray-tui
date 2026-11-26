//! Panel Timer: dígitos grandes, estado, contexto y barra de progreso.

use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Color,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    timer::Phase,
    tui::{
        app::App,
        digits::{self, Scale},
        theme::{
            DARK, DIM, FAINT, IDLE_DIGITS, KEY_BG, MUTED, OK, TEXT, WARN, accent, bold, fg, pill,
        },
        ui::{ellipsize, width},
    },
};

pub fn mmss(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

fn is_break(p: Phase) -> bool {
    p != Phase::Focus
}

/// Color de los dígitos en la fila `t` (0.0 arriba → 1.0 abajo).
fn digit_color(app: &App, t: f64) -> Color {
    if !app.timer.is_running() {
        IDLE_DIGITS
    } else if is_break(app.timer.phase()) {
        OK
    } else {
        accent(t)
    }
}

fn state_pill(app: &App) -> Span<'static> {
    let phase = match app.timer.phase() {
        Phase::Focus => "ENFOQUE",
        Phase::ShortBreak => "DESCANSO",
        Phase::LongBreak => "DESCANSO LARGO",
    };
    if app.timer.is_paused() {
        Span::styled(format!(" ‖ PAUSA · {phase} "), pill(DARK, WARN))
    } else if app.timer.is_running() {
        let bg = if is_break(app.timer.phase()) {
            OK
        } else {
            accent(0.35)
        };
        Span::styled(format!(" ● {phase} "), pill(DARK, bg))
    } else {
        Span::styled(format!(" ○ {phase} · en espera "), pill(MUTED, KEY_BG))
    }
}

fn context_spans(app: &App, max: usize) -> Vec<Span<'static>> {
    let mut spans = vec![state_pill(app), Span::raw("  ")];
    if app.timer.phase() == Phase::Focus {
        spans.push(Span::styled(app.category_name().to_owned(), bold(TEXT)));
        if let Some(t) = app.active_task() {
            spans.push(Span::styled(" · ", fg(FAINT)));
            let used = width(&spans);
            let label = format!("{} {}/{}", t.title, t.done, t.estimate);
            spans.push(Span::styled(
                ellipsize(&label, max.saturating_sub(used)),
                fg(MUTED),
            ));
        }
    } else {
        spans.push(Span::styled("levántate, estira, respira", fg(MUTED)));
    }
    spans
}

fn bar(app: &App, w: usize) -> Line<'static> {
    let frac = app.timer.progress(app.now);
    let label = format!(" {:>3}%", (frac * 100.0).floor() as u32);
    let bw = w.saturating_sub(label.len());
    let filled = (frac * bw as f64).round() as usize;
    let mut spans: Vec<Span> = (0..bw)
        .map(|i| {
            if i < filled {
                let c = if is_break(app.timer.phase()) {
                    OK
                } else {
                    accent(i as f64 / bw.max(1) as f64)
                };
                Span::styled("━", fg(c))
            } else {
                Span::styled("━", fg(FAINT))
            }
        })
        .collect();
    spans.push(Span::styled(label, fg(DIM)));
    Line::from(spans)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App, big: bool) {
    let inner = area.inner(Margin::new(2, 0));
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let clock = mmss(app.timer.remaining(app.now));
    let w = usize::from(inner.width);
    let want = if big { Scale::BIG } else { Scale::WIDE };
    let rows_h = u16::try_from(want.rows()).unwrap_or(5);

    if inner.height >= rows_h + 3 {
        let [digits_r, _, info, bar_r] = Layout::vertical([
            Constraint::Length(rows_h),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        let scale = [want, Scale::WIDE, Scale::SMALL]
            .into_iter()
            .find(|s| digits::width(&clock, *s) <= w && s.rows() <= usize::from(rows_h))
            .unwrap_or(Scale::SMALL);
        let rows = digits::render(&clock, scale);
        let last = rows.len().saturating_sub(1).max(1) as f64;
        let lines: Vec<Line> = rows
            .into_iter()
            .enumerate()
            .map(|(i, r)| Line::styled(r, fg(digit_color(app, i as f64 / last))))
            .collect();
        // Centrado vertical si la escala elegida es más baja que el hueco.
        let pad = digits_r
            .height
            .saturating_sub(u16::try_from(lines.len()).unwrap_or(0))
            / 2;
        let d = Rect::new(
            digits_r.x,
            digits_r.y + pad,
            digits_r.width,
            digits_r.height - pad,
        );
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), d);
        f.render_widget(
            Paragraph::new(Line::from(context_spans(app, w))).alignment(Alignment::Center),
            info,
        );
        f.render_widget(Paragraph::new(bar(app, w)), bar_r);
    } else {
        // Compacto: reloj en texto + estado, contexto y barra.
        let mut first = vec![
            Span::styled(clock, bold(digit_color(app, 0.3))),
            Span::raw("  "),
        ];
        let rest = context_spans(app, w.saturating_sub(width(&first)));
        first.extend(rest);
        let rows = Layout::vertical([Constraint::Length(1); 3]).split(inner);
        f.render_widget(Paragraph::new(Line::from(first)), rows[0]);
        if let Some(r) = rows.get(2).filter(|_| inner.height >= 3) {
            f.render_widget(Paragraph::new(bar(app, w)), *r);
        } else if let Some(r) = rows.get(1).filter(|_| inner.height >= 2) {
            f.render_widget(Paragraph::new(bar(app, w)), *r);
        }
    }
}
