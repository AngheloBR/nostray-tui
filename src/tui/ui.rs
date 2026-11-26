//! Dibujo: cabecera, pie, marco de paneles y modales.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};

use super::{
    app::{App, CUSTOM_FIELDS, Level, Modal, custom_value, durations_of},
    layout::{self, MIN_H, MIN_W, PanelId, Placed},
    panels,
    theme::{BORDER, DIM, ERR, FAINT, KEY_BG, MUTED, OK, TEXT, WARN, accent, bold, fg},
};
use crate::{APP_NAME, session::SessionKind};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let Some(screen) = layout::compute(area, &|m| app.pick(m)) else {
        too_small(f, area);
        return;
    };
    draw_header(f, screen.header, app);
    for p in &screen.panels {
        draw_panel(f, app, p, screen.big_timer);
    }
    draw_footer(f, screen.footer);
    if let Some(m) = app.modal.clone() {
        draw_modal(f, area, app, m);
    }
}

pub(super) fn draw_panel(f: &mut Frame, app: &App, p: &Placed, big: bool) {
    let hot = app.focus == p.id;
    let title = match p.id {
        PanelId::Dist => panels::dist::title(app),
        id => id.title().to_owned(),
    };
    let mut block = panel_block(&title, hot, p.rot);
    let bottom_hint = match p.id {
        PanelId::Dist => Some("v hoy / 7 días"),
        PanelId::Tasks => Some("a alta · e editar · x borrar · ⏎ activa · +/−"),
        PanelId::Log => Some("↑↓ desplazar"),
        _ => None,
    };
    if hot && let Some(h) = bottom_hint {
        block = block
            .title_bottom(Line::styled(format!(" {h} "), fg(MUTED)).alignment(Alignment::Right));
    }
    let inner = block.inner(p.rect);
    f.render_widget(block, p.rect);
    match p.id {
        PanelId::Timer => panels::timer::draw(f, inner, app, big),
        PanelId::Today => panels::today::draw(f, inner, app),
        PanelId::Dist => panels::dist::draw(f, inner, app),
        PanelId::Activity => panels::activity::draw(f, inner, app),
        PanelId::Tasks => panels::tasks::draw(f, inner, app),
        PanelId::Log => panels::log::draw(f, inner, app),
    }
}

// ───────────────────────────── piezas ─────────────────────────────

pub fn panel_block(title: &str, hot: bool, rot: Option<(usize, usize)>) -> Block<'_> {
    let border = if hot { accent(0.5) } else { BORDER };
    let mut b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(border))
        .title(Line::from(vec![
            Span::styled(" ", fg(border)),
            Span::styled(title, bold(if hot { TEXT } else { MUTED })),
            Span::styled(" ", fg(border)),
        ]));
    if let Some((i, n)) = rot {
        b = b.title(
            Line::from(vec![
                Span::styled(" Tab ", fg(DIM)),
                Span::styled(format!("‹ {i}/{n} › "), fg(MUTED)),
            ])
            .alignment(Alignment::Right),
        );
    }
    b
}

pub fn gradient(text: &str) -> Vec<Span<'static>> {
    let n = text.chars().count().max(2) - 1;
    text.chars()
        .enumerate()
        .map(|(i, c)| Span::styled(c.to_string(), bold(accent(i as f64 / n as f64))))
        .collect()
}

pub fn key(k: &str) -> Span<'static> {
    Span::styled(format!(" {k} "), Style::new().fg(TEXT).bg(KEY_BG))
}

pub fn width(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// Recorta a `max` columnas con «…» si hace falta.
pub fn ellipsize(s: &str, max: usize) -> String {
    if Span::raw(s).width() <= max {
        return s.to_owned();
    }
    let mut out = String::new();
    for c in s.chars() {
        if Span::raw(format!("{out}{c}…")).width() > max {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

pub fn session_summary(kind: SessionKind, app: &App) -> String {
    let d = durations_of(kind, &app.config);
    let m = |x: std::time::Duration| x.as_secs() / 60;
    match d.long {
        Some(l) => format!(
            "{}/{} · largo {} tras {}",
            m(d.focus),
            m(d.short_break),
            m(l.len),
            l.every
        ),
        None => format!("{}/{}", m(d.focus), m(d.short_break)),
    }
}

fn too_small(f: &mut Frame, area: Rect) {
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .areas(area);
    let p = Paragraph::new(vec![
        Line::styled("terminal demasiado pequeña", bold(WARN)),
        Line::styled(
            format!("{}×{} · mínimo {MIN_W}×{MIN_H}", area.width, area.height),
            fg(DIM),
        ),
    ])
    .alignment(Alignment::Center);
    f.render_widget(p, mid);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let area = area.inner(Margin::new(1, 0));
    let mut left = gradient(APP_NAME);
    left.push(Span::styled(
        format!(" v{}", env!("CARGO_PKG_VERSION")),
        fg(FAINT),
    ));

    let d = app.timer.durations();
    let mut right = vec![
        Span::styled(app.session.label(), fg(MUTED)),
        Span::styled(" · ", fg(FAINT)),
        Span::styled(app.category_name().to_owned(), fg(MUTED)),
    ];
    if let Some(l) = d.long {
        right.push(Span::styled(" · ", fg(FAINT)));
        right.push(Span::styled(
            format!("ciclo {}/{}", app.timer.focus_done() + 1, l.every),
            fg(MUTED),
        ));
    }
    right.push(Span::styled("  ? ayuda", fg(DIM)));

    // Si no cabe todo, se sacrifica primero la parte derecha.
    let w = usize::from(area.width);
    let right_fits = width(&left) + width(&right) + 2 <= w;
    f.render_widget(Paragraph::new(Line::from(left.clone())), area);
    if right_fits {
        f.render_widget(
            Paragraph::new(Line::from(right.clone())).alignment(Alignment::Right),
            area,
        );
    }

    if let Some(t) = app.toast() {
        let used = width(&left) + if right_fits { width(&right) } else { 0 };
        let free = w.saturating_sub(used + 4);
        let (icon, c) = match t.level {
            Level::Ok => ("✓ ", OK),
            Level::Info => ("· ", accent(0.6)),
            Level::Warn => ("! ", WARN),
            Level::Err => ("✗ ", ERR),
        };
        let msg = t.msg.trim_start_matches("✓ ");
        let text = ellipsize(&format!("{icon}{msg}"), free);
        if free >= 6 {
            let x = area.x + u16::try_from(width(&left) + 2).unwrap_or(0);
            let r = Rect::new(x, area.y, u16::try_from(free).unwrap_or(0), 1);
            f.render_widget(Paragraph::new(Line::styled(text, fg(c))), r);
        }
    }
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let area = area.inner(Margin::new(1, 0));
    let items: [(&str, &str); 8] = [
        ("espacio", "iniciar"),
        ("s", "saltar"),
        ("r", "reiniciar"),
        ("m", "sesión"),
        ("c", "categoría"),
        ("Tab", "panel"),
        ("?", "ayuda"),
        ("q", "salir"),
    ];
    // Entran por prioridad (espacio, q, ?, Tab, c, m; luego s y r) y se muestran en su orden.
    const PRIORITY: [usize; 8] = [0, 7, 6, 5, 4, 3, 1, 2];
    let piece = |(k, d): (&str, &str)| [key(k), Span::styled(format!(" {d}  "), fg(MUTED))];
    let max = usize::from(area.width);
    let mut keep = [false; 8];
    let mut used = 0;
    for i in PRIORITY {
        let w = width(&piece(items[i]));
        if used + w <= max {
            keep[i] = true;
            used += w;
        }
    }
    let spans: Vec<Span> = items
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .flat_map(|(it, _)| piece(it))
        .collect();
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ───────────────────────────── modales ─────────────────────────────

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn modal_block(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(accent(0.5)))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(title, bold(TEXT)),
            Span::raw(" "),
        ]))
}

fn show(f: &mut Frame, area: Rect, title: &str, lines: Vec<Line>, w: u16) {
    let h = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let r = centered(area, w, h);
    f.render_widget(Clear, r);
    f.render_widget(Paragraph::new(lines).block(modal_block(title)), r);
}

fn hint(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    for (k, d) in pairs {
        spans.push(key(k));
        spans.push(Span::styled(format!(" {d}  "), fg(MUTED)));
    }
    Line::from(spans)
}

fn draw_modal(f: &mut Frame, area: Rect, app: &App, m: Modal) {
    match m {
        Modal::Help => {
            let rows: [(&str, &str); 10] = [
                ("espacio", "iniciar · pausar · reanudar"),
                ("s", "saltar la fase actual"),
                ("r", "reiniciar la fase actual"),
                ("m", "cambiar de sesión"),
                ("c", "cambiar de categoría"),
                ("Tab", "panel siguiente (rota los ocultos)"),
                ("Shift+Tab", "panel anterior"),
                ("?", "esta ayuda"),
                ("q", "salir (confirma si hay ciclo en curso)"),
                ("Ctrl+C", "salir ya"),
            ];
            let panel_rows: [(&str, &str); 4] = [
                ("Tareas", "a alta · e editar · x borrar"),
                ("", "⏎ activa · +/− estimado · ↑↓ mover"),
                ("Log", "↑↓ · RePág/AvPág · Inicio/Fin"),
                ("Distrib.", "v hoy / 7 días"),
            ];
            let row = |k: &str, d: &str| {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{k:<11}"), bold(accent(0.4))),
                    Span::styled(d.to_owned(), fg(TEXT)),
                ])
            };
            let mut lines = vec![Line::default()];
            lines.extend(rows.iter().map(|(k, d)| row(k, d)));
            lines.push(Line::styled("  con el panel enfocado", fg(DIM)));
            lines.extend(panel_rows.iter().map(|(k, d)| row(k, d)));
            lines.push(Line::default());
            lines.push(Line::styled("  cualquier tecla para cerrar", fg(DIM)));
            show(f, area, "atajos", lines, 56);
        }
        Modal::Categories(idx) => {
            let mut lines = vec![Line::default()];
            for (i, c) in app.config.categories.iter().enumerate() {
                let sel = i == idx;
                let cur = i == app.category;
                lines.push(Line::from(vec![
                    Span::styled(if sel { "  › " } else { "    " }, bold(accent(0.5))),
                    Span::styled(format!("{} ", i + 1), fg(DIM)),
                    Span::styled(ellipsize(c, 30), if sel { bold(TEXT) } else { fg(MUTED) }),
                    Span::styled(if cur { "  ● actual" } else { "" }, fg(OK)),
                ]));
            }
            lines.push(Line::default());
            lines.push(hint(&[
                ("↑↓/1-8", "elegir"),
                ("⏎", "usar"),
                ("Esc", "cerrar"),
            ]));
            show(f, area, "categoría", lines, 48);
        }
        Modal::TaskInput { edit, buf } => {
            let title = if edit.is_some() {
                "editar tarea"
            } else {
                "nueva tarea"
            };
            let cursor = Span::styled("▏", fg(accent(0.5)));
            let empty = buf.trim().is_empty();
            let lines = vec![
                Line::default(),
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(ellipsize(&buf, 48), bold(TEXT)),
                    cursor,
                ]),
                Line::styled(
                    if empty {
                        "  escribe un título (máx. 60 caracteres)"
                    } else {
                        ""
                    },
                    fg(DIM),
                ),
                hint(&[("⏎", "guardar"), ("Esc", "cancelar")]),
            ];
            show(f, area, title, lines, 56);
        }
        Modal::ConfirmDelete(id) => {
            let name = app
                .tasks
                .tasks
                .iter()
                .find(|t| t.id == id)
                .map_or_else(String::new, |t| ellipsize(&t.title, 40));
            let lines = vec![
                Line::default(),
                Line::styled(format!("  «{name}»"), bold(TEXT)),
                Line::styled("  El historial conserva sus pomodoros.", fg(MUTED)),
                Line::default(),
                hint(&[("⏎/s", "borrar"), ("otra", "cancelar")]),
            ];
            show(f, area, "¿borrar tarea?", lines, 56);
        }
        Modal::Sessions { idx, initial } => {
            let mut lines = vec![Line::default()];
            for (i, k) in SessionKind::ALL.into_iter().enumerate() {
                let sel = i == idx;
                lines.push(Line::from(vec![
                    Span::styled(if sel { "  › " } else { "    " }, bold(accent(0.5))),
                    Span::styled(
                        format!("{:<14}", k.label()),
                        if sel { bold(TEXT) } else { fg(MUTED) },
                    ),
                    Span::styled(session_summary(k, app), fg(if sel { MUTED } else { DIM })),
                ]));
            }
            lines.push(Line::default());
            let mut keys = vec![("↑↓", "elegir"), ("⏎", "usar")];
            if SessionKind::ALL.get(idx) == Some(&SessionKind::Personalizado) {
                keys.push(("e", "editar"));
            }
            keys.push(("Esc", if initial { "Clásico" } else { "cerrar" }));
            lines.push(hint(&keys));
            show(f, area, "sesión", lines, 56);
        }
        Modal::ConfirmSwitch(k) => {
            let lines = vec![
                Line::default(),
                Line::styled(
                    format!("  Cambiar a {} reinicia el ciclo en curso.", k.label()),
                    fg(TEXT),
                ),
                Line::styled("  El tiempo de esta fase no se guarda.", fg(MUTED)),
                Line::default(),
                hint(&[("⏎/s", "cambiar"), ("otra", "seguir")]),
            ];
            show(f, area, "¿cambiar de sesión?", lines, 56);
        }
        Modal::EditCustom { field, values } => {
            let mut lines = vec![Line::default()];
            for (i, (label, (lo, hi))) in CUSTOM_FIELDS.into_iter().enumerate() {
                let sel = i == field;
                lines.push(Line::from(vec![
                    Span::styled(if sel { "  › " } else { "    " }, bold(accent(0.5))),
                    Span::styled(
                        format!("{label:<22}"),
                        if sel { bold(TEXT) } else { fg(MUTED) },
                    ),
                    Span::styled(
                        format!("‹ {:>3} ›", custom_value(&values, i)),
                        if sel { bold(accent(0.3)) } else { fg(TEXT) },
                    ),
                    Span::styled(format!("  {lo}–{hi}"), fg(DIM)),
                ]));
            }
            lines.push(Line::default());
            lines.push(hint(&[("↑↓", "campo"), ("←→", "±1"), ("⇧", "±5")]));
            lines.push(hint(&[("⏎", "guardar y usar"), ("Esc", "volver")]));
            show(f, area, "sesión personalizada", lines, 56);
        }
        Modal::ConfirmQuit => {
            let lines = vec![
                Line::default(),
                Line::styled("  Hay un ciclo en curso.", fg(TEXT)),
                Line::styled(
                    "  Si sales, el tiempo de esta fase no se guarda.",
                    fg(MUTED),
                ),
                Line::default(),
                hint(&[("⏎/s", "salir"), ("otra", "seguir")]),
            ];
            show(f, area, "¿salir?", lines, 56);
        }
    }
}
