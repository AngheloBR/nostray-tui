//! Tests de render con `TestBackend`: el mismo código que dibuja la TUI.

use ratatui::{Terminal, backend::TestBackend};

use jiff::civil::date;
use ratatui::layout::Rect;

use super::{
    app::{App, DistRange, Modal, tests_support::app_with, tests_support::empty},
    layout::{PanelId, Placed},
    ui,
};
use crate::{
    session::SessionKind,
    stats::fixtures::{history, pomo},
    store::Loaded,
};

/// Dibuja `app` en un buffer `w`×`h` y lo devuelve como texto (una línea por fila).
pub fn render(app: &App, w: u16, h: u16) -> String {
    // El error de `TestBackend` es `Infallible`.
    let Ok(mut t) = Terminal::new(TestBackend::new(w, h));
    if t.draw(|f| ui::draw(f, app)).is_err() {
        return String::new();
    }
    let buf = t.backend().buffer();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn clasico() -> App {
    app_with(Some(SessionKind::Clasico), empty()).0
}

#[test]
fn timer_80x24() {
    let s = render(&clasico(), 80, 24);
    assert!(s.contains("╭ Timer"), "{s}");
    assert!(s.contains("╭ Hoy"), "{s}");
    assert!(s.contains("Distribución"), "{s}");
    assert!(s.contains("‹ 1/4 ›"), "hueco rotatorio\n{s}");
    assert!(s.contains("██"), "dígitos grandes\n{s}");
    assert!(s.contains("ENFOQUE · en espera"), "{s}");
    assert!(s.contains("Programar"), "{s}");
    assert!(s.contains("espacio"), "pie con atajos\n{s}");
    assert!(s.contains("q  salir") && s.contains("?  ayuda"), "{s}");
    assert!(s.contains("ciclo 1/4"), "{s}");
}

#[test]
fn timer_compacto_40x12() {
    let s = render(&clasico(), 40, 12);
    assert!(s.contains("25:00"), "reloj en texto\n{s}");
    assert!(s.contains("Timer") && s.contains("Hoy"), "{s}");
    assert!(!s.contains("demasiado pequeña"), "{s}");
}

#[test]
fn menor_que_minimo_avisa() {
    let s = render(&clasico(), 39, 11);
    assert!(s.contains("demasiado pequeña"), "{s}");
    assert!(s.contains("39×11"), "{s}");
}

#[test]
fn grande_140x40_usa_escala_big() {
    let s = render(&clasico(), 140, 40);
    assert!(s.contains("████████"), "píxeles de 4 columnas\n{s}");
    for t in [
        "Timer",
        "Hoy",
        "Distribución",
        "Racha y actividad",
        "Tareas",
        "Log",
    ] {
        assert!(s.contains(t), "falta {t}\n{s}");
    }
}

#[test]
fn medio_100x30() {
    let s = render(&clasico(), 100, 30);
    for t in ["Timer", "Hoy", "Distribución", "Racha y actividad"] {
        assert!(s.contains(t), "falta {t}\n{s}");
    }
}

#[test]
fn selector_y_ayuda() {
    let (mut a, _) = app_with(None, empty());
    let s = render(&a, 80, 24);
    assert!(
        s.contains("sesión") && s.contains("Deep work") && s.contains("90/20"),
        "{s}"
    );
    a.modal = Some(Modal::Help);
    let s = render(&a, 80, 24);
    assert!(s.contains("atajos") && s.contains("Ctrl+C"), "{s}");
    a.modal = Some(Modal::ConfirmQuit);
    assert!(render(&a, 80, 24).contains("¿salir?"));
}

#[test]
fn modal_en_terminal_minima_no_rompe() {
    let (a, _) = app_with(None, empty());
    let s = render(&a, 40, 12);
    assert!(s.contains("sesión"), "{s}");
}

#[test]
fn ningun_tamano_hace_panic() {
    let (mut a, _) = app_with(None, empty());
    for modal in [None, Some(Modal::Help), Some(Modal::ConfirmQuit)] {
        a.modal = modal;
        for w in 0..=90 {
            for h in 0..=30 {
                let _ = render(&a, w, h);
            }
        }
    }
}

/// Dibuja un solo panel (con su borde) en un buffer `w`×`h`.
pub fn render_panel(app: &App, id: PanelId, w: u16, h: u16) -> String {
    let Ok(mut t) = Terminal::new(TestBackend::new(w, h));
    let placed = Placed {
        id,
        rect: Rect::new(0, 0, w, h),
        rot: None,
    };
    if t.draw(|f| ui::draw_panel(f, app, &placed, false)).is_err() {
        return String::new();
    }
    let buf = t.backend().buffer();
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Datos ficticios: hoy (2026-03-14) 4 pomodoros, más 2 en la semana.
fn con_datos() -> App {
    let hoy = date(2026, 3, 14);
    let mut l: Loaded = empty();
    l.history = history(vec![
        pomo(hoy, "Programar", 25),
        pomo(hoy, "Programar", 25),
        pomo(hoy, "Inglés", 25),
        pomo(hoy, "Leer", 25),
        pomo(date(2026, 3, 10), "Estudiar", 50),
        pomo(date(2026, 3, 1), "Otro", 25), // fuera de los 7 días
    ]);
    app_with(Some(SessionKind::Clasico), l).0
}

#[test]
fn hoy_80x24_con_datos() {
    let s = render(&con_datos(), 80, 24);
    assert!(s.contains("Meta ▰"), "{s}");
    assert!(s.contains(" 50%") && s.contains("4/8"), "{s}");
    assert!(s.contains("4 pomodoros · 100 min de foco"), "{s}");
}

#[test]
fn hoy_vacio_y_meta_cumplida() {
    let s = render_panel(&clasico(), PanelId::Today, 80, 4);
    assert!(s.contains("Aún no hay pomodoros hoy · meta 8"), "{s}");
    assert!(s.contains("  0%") && s.contains("0/8"), "{s}");

    let mut a = con_datos();
    a.config.daily_goal = 3;
    let s = render_panel(&a, PanelId::Today, 80, 4);
    assert!(s.contains("133%") && s.contains("✓ meta cumplida"), "{s}");
}

#[test]
fn hoy_pequeno_una_linea() {
    let s = render_panel(&con_datos(), PanelId::Today, 40, 3);
    assert!(
        s.contains("50%") && s.contains("4/8") && s.contains("100 min"),
        "{s}"
    );
    // 40x12 completo: Hoy visible sin romper.
    let s = render(&con_datos(), 40, 12);
    assert!(s.contains("Hoy") && s.contains("50%"), "{s}");
}

#[test]
fn distribucion_80x24_hoy_y_semana() {
    let mut a = con_datos();
    let s = render(&a, 80, 24);
    assert!(s.contains("Distribución · hoy"), "{s}");
    assert!(
        s.contains("Programar") && s.contains(" 50%") && s.contains("50 min"),
        "{s}"
    );
    assert!(s.contains("Inglés") && s.contains(" 25%"), "{s}");
    assert!(!s.contains("Estudiar"), "Estudiar es de otro día\n{s}");

    a.dist_range = DistRange::Week;
    let s = render(&a, 80, 24);
    assert!(s.contains("Distribución · 7 días"), "{s}");
    assert!(s.contains("Estudiar") && s.contains("Programar"), "{s}");
    assert!(!s.contains("Otro "), "fuera de la ventana\n{s}");
}

#[test]
fn distribucion_vacia_y_pequena() {
    let s = render_panel(&clasico(), PanelId::Dist, 80, 7);
    assert!(s.contains("Sin foco registrado hoy"), "{s}");
    // 3 categorías en 2 filas: una visible + resumen.
    let s = render_panel(&con_datos(), PanelId::Dist, 30, 4);
    assert!(
        s.contains("Program…") && s.contains("+2 más"),
        "nombre recortado\n{s}"
    );
    let _ = render_panel(&con_datos(), PanelId::Dist, 6, 3);
}

#[test]
fn distribucion_enfocada_muestra_atajo_v() {
    let mut a = con_datos();
    a.focus = PanelId::Dist;
    let s = render_panel(&a, PanelId::Dist, 80, 7);
    assert!(s.contains("v hoy / 7 días"), "{s}");
}

fn render_buf(app: &App, id: PanelId, w: u16, h: u16) -> Option<ratatui::buffer::Buffer> {
    let Ok(mut t) = Terminal::new(TestBackend::new(w, h));
    let placed = Placed {
        id,
        rect: Rect::new(0, 0, w, h),
        rot: None,
    };
    t.draw(|f| ui::draw_panel(f, app, &placed, false)).ok()?;
    Some(t.backend().buffer().clone())
}

/// Racha de 3 días hasta hoy (sábado 2026-03-14), mejor racha de 5 en febrero.
fn con_racha() -> App {
    let mut v = Vec::new();
    for d in [10, 11, 12, 13, 14] {
        v.push(pomo(date(2026, 2, d), "Leer", 25));
    }
    for d in [12, 13] {
        v.push(pomo(date(2026, 3, d), "Programar", 25));
    }
    for _ in 0..8 {
        v.push(pomo(date(2026, 3, 14), "Programar", 25)); // nivel 4 con meta 8
    }
    let mut l: Loaded = empty();
    l.history = history(v);
    app_with(Some(SessionKind::Clasico), l).0
}

#[test]
fn actividad_80x12_completa() {
    let s = render_panel(&con_racha(), PanelId::Activity, 80, 12);
    assert!(
        s.contains("Racha 3 días") && s.contains("mejor 5 días"),
        "{s}"
    );
    assert!(s.contains("feb") && s.contains("mar"), "fila de meses\n{s}");
    for l in ["│ L ", "│ X ", "│ D "] {
        assert!(s.contains(l), "etiqueta {l}\n{s}");
    }
    assert!(s.contains("15 pomodoros · 37 semanas"), "{s}");
    assert!(s.contains("menos ■ ■ ■ ■ ■ más"), "leyenda\n{s}");
}

#[test]
fn actividad_colores_por_nivel_y_futuro_vacio() {
    let a = con_racha();
    let Some(buf) = render_buf(&a, PanelId::Activity, 80, 12) else {
        panic!("render")
    };
    // Interior 76 → 37 semanas. Última columna: x = 1 borde + 1 margen + 2 etiqueta + 36·2.
    // Filas: y=1 racha, y=2 meses, y=3 lunes … y=9 domingo.
    let x = 1 + 1 + 2 + 36 * 2;
    let lunes = 3;
    let sat = buf[(x, lunes + 5)].clone();
    assert_eq!(sat.symbol(), "■");
    assert_eq!(sat.fg, super::theme::heat(4), "8 pomodoros hoy → nivel 4");
    assert_eq!(buf[(x, lunes + 6)].symbol(), " ", "domingo futuro vacío");
    let vie = buf[(x, lunes + 4)].clone();
    assert_eq!(vie.fg, super::theme::heat(1), "1 pomodoro → nivel 1");
    let lun = buf[(x, lunes)].clone();
    assert_eq!(lun.fg, super::theme::heat(0), "lunes sin datos → nivel 0");
}

#[test]
fn actividad_degrada_por_tamano() {
    let a = con_racha();
    // Sin fila de meses.
    let s = render_panel(&a, PanelId::Activity, 80, 11);
    assert!(!s.contains("feb") && s.contains("menos"), "{s}");
    // Sin leyenda.
    let s = render_panel(&a, PanelId::Activity, 80, 10);
    assert!(!s.contains("menos") && s.contains("│ D "), "{s}");
    // Pocas semanas pero aún heatmap.
    let s = render_panel(&a, PanelId::Activity, 14, 12);
    assert!(s.contains("│ L ■"), "{s}");
    // Demasiado estrecho o bajo: solo números.
    let s = render_panel(&a, PanelId::Activity, 12, 12);
    assert!(s.contains("Racha") && !s.contains("■"), "{s}");
    let s = render_panel(&a, PanelId::Activity, 80, 5);
    assert!(s.contains("Racha 3 días") && s.contains("amplía"), "{s}");
    let _ = render_panel(&a, PanelId::Activity, 3, 3);
}

#[test]
fn actividad_vacia_y_racha_de_ayer() {
    let s = render_panel(&clasico(), PanelId::Activity, 80, 12);
    assert!(s.contains("Sin racha todavía"), "{s}");
    assert!(s.contains("0 pomodoros · 37 semanas"), "{s}");

    let mut l: Loaded = empty();
    l.history = history(vec![pomo(date(2026, 3, 13), "Leer", 25)]);
    let a = app_with(Some(SessionKind::Clasico), l).0;
    let s = render_panel(&a, PanelId::Activity, 80, 12);
    assert!(
        s.contains("Racha 1 día") && s.contains("haz uno hoy"),
        "{s}"
    );
}

#[test]
fn actividad_en_layouts_completos() {
    let mut a = con_racha();
    let s = render(&a, 140, 40);
    assert!(
        s.contains("Racha 3 días") && s.contains("· 32 semanas"),
        "{s}"
    );
    let s = render(&a, 100, 30);
    assert!(
        s.contains("Racha 3 días") && s.contains("· 22 semanas"),
        "{s}"
    );
    // 80x24: está en el hueco que rota; aparece al llegar con Tab.
    let s = render(&a, 80, 24);
    assert!(!s.contains("Racha 3 días"), "{s}");
    for _ in 0..3 {
        a.on_key(super::app::tests_support::key(
            ratatui::crossterm::event::KeyCode::Tab,
        ));
    }
    assert_eq!(a.focus, PanelId::Activity);
    let s = render(&a, 80, 24);
    assert!(s.contains("Racha 3 días") && s.contains("‹ 2/4 ›"), "{s}");
}

fn con_tareas_y_log() -> App {
    use crate::timer::Phase;
    let mut l: Loaded = empty();
    let hoy = date(2026, 3, 14);
    for t in [
        "Repasar vocabulario",
        "Escribir parser de ejemplo",
        "Leer capítulo 3",
    ] {
        l.tasks.add(t);
    }
    l.tasks.tasks[0].estimate = 2;
    l.tasks.tasks[0].done = 2;
    l.tasks.tasks[1].estimate = 4;
    l.tasks.tasks[1].done = 1;
    l.tasks.active = Some(2);
    let at = |h: &str| h.parse::<jiff::Timestamp>().unwrap_or_default();
    let mut r1 = pomo(hoy, "Programar", 25);
    r1.ended_at = at("2026-03-14T09:25:00Z");
    r1.task = Some("Escribir parser de ejemplo".into());
    let mut r2 = stats_rec(hoy, Phase::ShortBreak, 5, true);
    r2.ended_at = at("2026-03-14T09:30:00Z");
    let mut r3 = stats_rec(hoy, Phase::Focus, 7, false);
    r3.ended_at = at("2026-03-14T09:37:00Z");
    r3.category = Some("Inglés".into());
    let mut r0 = pomo(date(2026, 3, 13), "Leer", 25);
    r0.ended_at = at("2026-03-13T20:00:00Z");
    l.history = history(vec![r0, r1, r2, r3]);
    app_with(Some(SessionKind::Clasico), l).0
}

fn stats_rec(
    d: jiff::civil::Date,
    p: crate::timer::Phase,
    m: u64,
    ok: bool,
) -> crate::model::CycleRecord {
    crate::stats::fixtures::rec(d, "", m, p, ok)
}

#[test]
fn tareas_panel_80x8() {
    let mut a = con_tareas_y_log();
    a.focus = PanelId::Tasks;
    a.task_sel = 1;
    let s = render_panel(&a, PanelId::Tasks, 80, 8);
    assert!(
        s.contains("Repasar vocabulario") && s.contains("2/2 ✓"),
        "{s}"
    );
    assert!(
        s.contains("› ● Escribir parser de ejemplo"),
        "seleccionada y activa\n{s}"
    );
    assert!(s.contains("1/4"), "{s}");
    assert!(s.contains("a alta · e editar"), "atajos del panel\n{s}");
}

#[test]
fn tareas_vacio_y_pequeno() {
    let s = render_panel(&clasico(), PanelId::Tasks, 80, 8);
    assert!(s.contains("Sin tareas (son opcionales)."), "{s}");
    let mut a = con_tareas_y_log();
    a.focus = PanelId::Tasks;
    a.task_sel = 2;
    // 1 fila visible: la selección (la tercera) tiene que verse.
    let s = render_panel(&a, PanelId::Tasks, 30, 3);
    assert!(s.contains("Leer cap") && !s.contains("Repasar"), "{s}");
    let _ = render_panel(&a, PanelId::Tasks, 5, 3);
}

#[test]
fn log_panel_80x8() {
    let s = render_panel(&con_tareas_y_log(), PanelId::Log, 80, 8);
    let lines: Vec<&str> = s.lines().collect();
    // Más reciente arriba, hora local (UTC en el reloj de pruebas).
    assert!(
        lines[1].contains("09:37") && lines[1].contains("Inglés") && lines[1].contains("saltado"),
        "{s}"
    );
    assert!(
        lines[2].contains("09:30") && lines[2].contains("descanso") && lines[2].contains("05:00 ✓"),
        "{s}"
    );
    assert!(
        lines[3].contains("09:25")
            && lines[3].contains("Clásico")
            && lines[3].contains("Programar"),
        "{s}"
    );
    assert!(
        lines[3].contains("· Escribir parser de ejemplo") && lines[3].contains("25:00 ✓"),
        "{s}"
    );
    assert!(
        lines[4].contains("13/03 20:00") && lines[4].contains("Leer"),
        "otro día con fecha\n{s}"
    );
}

#[test]
fn log_vacio_desplazado_y_pequeno() {
    let s = render_panel(&clasico(), PanelId::Log, 80, 8);
    assert!(s.contains("Aquí aparecerá cada ciclo al cerrarse."), "{s}");
    let mut a = con_tareas_y_log();
    // 3 filas: 2 registros + indicador.
    let s = render_panel(&a, PanelId::Log, 80, 5);
    assert!(s.contains("1–2 de 4") && s.contains("09:37"), "{s}");
    a.log_scroll = 2;
    let s = render_panel(&a, PanelId::Log, 80, 5);
    assert!(
        s.contains("3–4 de 4") && s.contains("13/03") && !s.contains("09:37"),
        "{s}"
    );
    let _ = render_panel(&a, PanelId::Log, 10, 3);
}

#[test]
fn modales_de_m7() {
    let mut a = con_tareas_y_log();
    a.modal = Some(Modal::Categories(2));
    let s = render(&a, 80, 24);
    assert!(
        s.contains("categoría") && s.contains("› 3 Inglés") && s.contains("● actual"),
        "{s}"
    );
    a.modal = Some(Modal::TaskInput {
        edit: None,
        buf: String::new(),
    });
    let s = render(&a, 80, 24);
    assert!(
        s.contains("nueva tarea") && s.contains("escribe un título"),
        "{s}"
    );
    a.modal = Some(Modal::ConfirmDelete(2));
    let s = render(&a, 80, 24);
    assert!(
        s.contains("¿borrar tarea?") && s.contains("«Escribir parser de ejemplo»"),
        "{s}"
    );
    a.modal = Some(Modal::Help);
    let s = render(&a, 80, 24);
    assert!(
        s.contains("cambiar de categoría") && s.contains("a alta · e editar"),
        "{s}"
    );
}

#[test]
fn todo_visible_140x40_con_datos() {
    let s = render(&con_tareas_y_log(), 140, 40);
    for t in [
        "Escribir parser de ejemplo",
        "09:37",
        "Racha",
        "Distribución · hoy",
        "c  categoría",
    ] {
        assert!(s.contains(t), "falta {t}\n{s}");
    }
}
