//! Panel Tareas: lista con pomodoros hechos/estimados y tarea activa.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::{
    app::App,
    layout::PanelId,
    theme::{DIM, MUTED, OK, TEXT, accent, bold, fg},
    ui::{ellipsize, width},
};

/// Primera fila visible para que la selección quede siempre a la vista.
pub fn scroll_for(sel: usize, rows: usize, len: usize) -> usize {
    if rows == 0 || len <= rows {
        return 0;
    }
    sel.saturating_sub(rows - 1).min(len - rows)
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let inner = crate::tui::ui::pad(area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let tasks = &app.tasks.tasks;
    if tasks.is_empty() {
        let lines = vec![
            Line::styled("Sin tareas (son opcionales).", fg(DIM)),
            Line::styled("Tab hasta aquí y pulsa a para crear una.", fg(DIM)),
        ];
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let hot = app.focus == PanelId::Tasks;
    let rows = usize::from(inner.height);
    let w = usize::from(inner.width);
    let first = scroll_for(app.task_sel, rows, tasks.len());
    let lines: Vec<Line> = tasks
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(i, t)| {
            let sel = hot && i == app.task_sel;
            let active = app.tasks.active == Some(t.id);
            let done = t.done >= t.estimate;
            let head = vec![
                Span::styled(if sel { "› " } else { "  " }, bold(accent(0.5))),
                Span::styled(if active { "● " } else { "  " }, fg(accent(0.3))),
            ];
            let tail = vec![
                Span::styled(
                    format!(" {}/{}", t.done, t.estimate),
                    if done { bold(OK) } else { fg(MUTED) },
                ),
                Span::styled(if done { " ✓" } else { "  " }, fg(OK)),
            ];
            let room = w.saturating_sub(width(&head) + width(&tail));
            let title = ellipsize(&t.title, room);
            let pad = room.saturating_sub(Span::raw(title.as_str()).width());
            let style = if sel || active { bold(TEXT) } else { fg(TEXT) };
            let mut spans = head;
            spans.push(Span::styled(title, style));
            spans.push(Span::raw(" ".repeat(pad)));
            spans.extend(tail);
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::scroll_for;

    #[test]
    fn scroll_mantiene_seleccion_visible() {
        assert_eq!(scroll_for(0, 3, 10), 0);
        assert_eq!(scroll_for(2, 3, 10), 0);
        assert_eq!(scroll_for(3, 3, 10), 1);
        assert_eq!(scroll_for(9, 3, 10), 7);
        assert_eq!(scroll_for(5, 10, 4), 0);
        assert_eq!(scroll_for(5, 0, 4), 0);
    }
}
