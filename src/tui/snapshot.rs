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
    assert!(s.contains("╭ timer"), "{s}");
    assert!(s.contains("╭ hoy"), "{s}");
    assert!(s.contains("distribución"), "{s}");
    assert!(s.contains("‹ 1/4 ›"), "hueco rotatorio\n{s}");
    assert!(s.contains("██"), "dígitos grandes\n{s}");
    assert!(s.contains("ENFOQUE · en espera"), "{s}");
    assert!(s.contains("Programar"), "{s}");
    assert!(s.contains("espacio"), "pie con atajos\n{s}");
    assert!(s.contains("q  salir") && s.contains("?  ayuda"), "{s}");
    assert!(s.contains("ciclo 1/4"), "{s}");
    assert!(
        s.contains("◉ ○ ○ ○") && s.contains("después descanso 5 min"),
        "{s}"
    );
    assert!(s.contains("○ listo"), "estado en la cabecera\n{s}");
}

#[test]
fn corriendo_muestra_hora_de_fin_y_pausar() {
    let (mut a, c) = app_with(Some(SessionKind::Clasico), empty());
    a.on_key(super::app::tests_support::key(
        ratatui::crossterm::event::KeyCode::Char(' '),
    ));
    c.advance(std::time::Duration::from_secs(60));
    a.update();
    // Reloj de pruebas: 09:01 UTC + 24 min.
    let s = render(&a, 100, 30);
    assert!(s.contains("● enfocando · termina 09:25"), "{s}");
    assert!(s.contains("espacio  pausar"), "{s}");
    let s = render(&a, 80, 24);
    assert!(
        s.contains("termina 09:25") && s.contains("ciclo 1/4"),
        "{s}"
    );
}

#[test]
fn timer_compacto_40x12() {
    let s = render(&clasico(), 40, 12);
    assert!(s.contains("25:00"), "reloj en texto\n{s}");
    assert!(s.contains("timer") && s.contains("hoy"), "{s}");
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
        "timer",
        "hoy",
        "distribución",
        "racha y actividad",
        "tareas",
        "log",
    ] {
        assert!(s.contains(t), "falta {t}\n{s}");
    }
}

#[test]
fn medio_100x30() {
    let s = render(&clasico(), 100, 30);
    for t in ["timer", "hoy", "distribución", "racha y actividad"] {
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
    a.modal = Some(Modal::Help(0));
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
fn ayuda_se_desplaza_en_terminal_minima() {
    let (mut a, _) = app_with(Some(SessionKind::Clasico), empty());
    a.modal = Some(Modal::Help(0));
    let s = render(&a, 40, 12);
    assert!(s.contains("↑↓ desplazar") && s.contains("espacio"), "{s}");
    assert!(!s.contains("Ctrl+C"), "{s}");
    a.modal = Some(Modal::Help(99));
    let s = render(&a, 40, 12);
    assert!(s.contains("Distrib.") && !s.contains("espacio "), "{s}");
}

#[test]
fn ningun_tamano_hace_panic() {
    let (mut a, _) = app_with(None, empty());
    for modal in [None, Some(Modal::Help(0)), Some(Modal::ConfirmQuit)] {
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
    assert!(s.contains("meta ▰"), "{s}");
    assert!(s.contains(" 50%") && s.contains("4/8"), "{s}");
    assert!(s.contains("● ● ● ● ○ ○ ○ ○"), "un punto por pomodoro\n{s}");
    assert!(s.contains("1 h 40 min de foco  ·  faltan 4"), "{s}");
    assert!(s.contains("semana  L ▁ M ▁ X ▁ J ▁ V ▁ S ▄ D ·"), "{s}");
}

#[test]
fn hoy_vacio_y_meta_cumplida() {
    let s = render_panel(&clasico(), PanelId::Today, 80, 4);
    assert!(s.contains("aún no hay pomodoros hoy · meta 8"), "{s}");
    assert!(s.contains("  0%") && s.contains("0/8"), "{s}");

    let mut a = con_datos();
    a.config.daily_goal = 3;
    let s = render_panel(&a, PanelId::Today, 80, 4);
    assert!(s.contains("133%") && s.contains("✓ meta cumplida"), "{s}");
    assert!(s.contains("● ● ● +1"), "los de más van como +N\n{s}");
}

#[test]
fn hoy_pequeno_una_linea() {
    let s = render_panel(&con_datos(), PanelId::Today, 40, 3);
    assert!(
        s.contains("50%") && s.contains("4/8") && s.contains("1 h 40 min"),
        "{s}"
    );
    // 40x12 completo: Hoy visible sin romper.
    let s = render(&con_datos(), 40, 12);
    assert!(s.contains("hoy") && s.contains("50%"), "{s}");
}

#[test]
fn distribucion_80x24_hoy_y_semana() {
    let mut a = con_datos();
    let s = render(&a, 80, 24);
    assert!(s.contains("distribución · hoy"), "{s}");
    assert!(
        s.contains("Programar") && s.contains(" 50%") && s.contains("50 min"),
        "{s}"
    );
    assert!(s.contains("Inglés") && s.contains(" 25%"), "{s}");
    assert!(!s.contains("Estudiar"), "Estudiar es de otro día\n{s}");

    a.dist_range = DistRange::Week;
    let s = render(&a, 80, 24);
    assert!(s.contains("distribución · 7 días"), "{s}");
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
    for l in ["│  L ", "│  X ", "│  D "] {
        assert!(s.contains(l), "etiqueta {l}\n{s}");
    }
    assert!(s.contains("15 pomodoros · 36 semanas"), "{s}");
    assert!(s.contains("menos ■ ■ ■ ■ ■ más"), "leyenda\n{s}");
}

#[test]
fn actividad_colores_por_nivel_y_futuro_vacio() {
    let a = con_racha();
    let Some(buf) = render_buf(&a, PanelId::Activity, 80, 12) else {
        panic!("render")
    };
    // Interior 74 → 36 semanas. Última columna: x = 1 borde + 2 margen + 2 etiqueta + 35·2.
    // Filas: y=1 racha, y=2 meses, y=3 lunes … y=9 domingo.
    let x = 1 + 2 + 2 + 35 * 2;
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
    assert!(!s.contains("menos") && s.contains("│  D "), "{s}");
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
    assert!(s.contains("0 pomodoros · 36 semanas"), "{s}");

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
        s.contains("Racha 3 días") && s.contains("· 31 semanas"),
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
        lines[1].contains("hora") && lines[1].contains("tarea") && lines[1].contains("dur."),
        "encabezado de columnas\n{s}"
    );
    let lines = &lines[1..];
    assert!(
        lines[1].contains("09:37") && lines[1].contains("Inglés") && lines[1].contains("07:00 ↷"),
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
        lines[3].contains("Escribir parser de ejemplo") && lines[3].contains("25:00 ✓"),
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
    // 4 filas: encabezado + 2 registros + indicador.
    let s = render_panel(&a, PanelId::Log, 80, 6);
    assert!(s.contains("1–2 de 4") && s.contains("09:37"), "{s}");
    a.log_scroll = 2;
    let s = render_panel(&a, PanelId::Log, 80, 6);
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
    a.modal = Some(Modal::Help(0));
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
        "distribución · hoy",
        "tareas · 1/3",
        "log · 4",
        "c  categoría",
    ] {
        assert!(s.contains(t), "falta {t}\n{s}");
    }
}

// ───────────────────── capturas del README (docs/*.svg) ─────────────────────

mod docs {
    //! `cargo test snapshots -- --ignored` reescribe `docs/tui-*.svg` y `docs/cli-stats.svg`;
    //! `cargo test demo -- --ignored`, `docs/demo.svg`. Todos los datos son ficticios.

    use std::{rc::Rc, time::Duration};

    use jiff::{Timestamp, civil::Date, tz::TimeZone};
    use ratatui::{
        Terminal, backend::TestBackend, buffer::Buffer, crossterm::event::KeyCode, layout::Rect,
        style::Style,
    };

    use super::super::{
        app::{App, Modal, tests_support::key},
        svg, theme, ui,
    };
    use crate::{
        clock::FakeClock,
        model::CycleRecord,
        notify::Recorder,
        session::SessionKind,
        stats::fixtures::{history, pomo},
        store::Loaded,
        timer::Phase,
    };

    const MIN: Duration = Duration::from_secs(60);

    fn at(d: Date, h: i8, m: i8) -> Timestamp {
        d.at(h, m, 0, 0)
            .to_zoned_datetime(TimeZone::UTC)
            .map(|z| z.timestamp())
            .unwrap_or_default()
    }

    fn focus(d: Date, cat: &str, h: i8, m: i8, task: Option<&str>) -> CycleRecord {
        let mut r = pomo(d, cat, 25);
        r.ended_at = at(d, h, m);
        r.task = task.map(str::to_owned);
        r
    }

    fn brk(d: Date, h: i8, m: i8) -> CycleRecord {
        let mut r = crate::stats::fixtures::rec(d, "", 5, Phase::ShortBreak, true);
        r.ended_at = at(d, h, m);
        r
    }

    /// Historial ficticio de ~5 meses con un patrón fijo (sin azar): más los días
    /// laborables, racha de 9 días hasta hoy y 3 pomodoros esta mañana.
    fn demo_loaded(today: Date) -> Loaded {
        let cats = ["Programar", "Estudiar", "Inglés", "Leer", "Otro"];
        let mut v = Vec::new();
        let mut seed: u32 = 7;
        for back in (1..=150).rev() {
            let Ok(d) = today.checked_sub(jiff::Span::new().days(back)) else {
                continue;
            };
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let r = (seed >> 16) % 10;
            let weekend = d.weekday().to_monday_zero_offset() >= 5;
            let mut n = match (weekend, r) {
                (_, 0..=1) if back > 9 => 0,
                (true, _) => r / 3,
                (false, _) => 2 + r % 7,
            };
            if back <= 9 {
                n = n.max(2); // racha
            }
            for i in 0..n {
                let cat = cats[((seed >> 8) as usize + i as usize) % cats.len()];
                let h = 9 + i8::try_from(i).unwrap_or(0);
                v.push(focus(d, cat, h, 25, None));
            }
        }
        // Esta mañana.
        v.push(focus(today, "Inglés", 9, 25, Some("Repasar vocabulario")));
        v.push(brk(today, 9, 30));
        v.push(focus(today, "Inglés", 9, 55, Some("Repasar vocabulario")));
        v.push(brk(today, 10, 0));
        v.push(focus(today, "Leer", 10, 25, None));
        let mut l = super::empty();
        l.history = history(v);
        for (t, est, done) in [
            ("Repasar vocabulario", 2, 2),
            ("Escribir parser de ejemplo", 4, 1),
            ("Leer capítulo 3", 2, 0),
            ("Preparar la presentación", 3, 0),
        ] {
            if let Some(id) = l.tasks.add(t)
                && let Some(task) = l.tasks.tasks.iter_mut().find(|x| x.id == id)
            {
                task.estimate = est;
                task.done = done;
            }
        }
        l.tasks.active = Some(2);
        l
    }

    /// App de demo con su reloj: hoy 2026-03-14 a las 13:00 UTC, sesión Clásico.
    fn demo_app(start: Option<SessionKind>) -> (App, Rc<FakeClock>) {
        let clock = Rc::new(FakeClock::new("2026-03-14T13:00:00Z", TimeZone::UTC));
        let today = jiff::civil::date(2026, 3, 14);
        let app = App::new(
            Box::new(Rc::clone(&clock)),
            Box::new(Recorder::default()),
            None,
            demo_loaded(today),
            start,
            None,
        );
        (app, clock)
    }

    fn press(app: &mut App, c: KeyCode) {
        app.on_key(key(c));
    }

    /// Completa la fase actual (la arranca y deja pasar su duración).
    fn finish_phase(app: &mut App, clock: &FakeClock) {
        press(app, KeyCode::Char(' '));
        clock.advance(app.timer.planned());
        app.update();
    }

    /// Deja la app en el 3.º foco de la serie, corriendo, con `left` restante.
    fn running_third_focus(left: Duration) -> (App, Rc<FakeClock>) {
        let (mut app, clock) = demo_app(Some(SessionKind::Clasico));
        for _ in 0..4 {
            finish_phase(&mut app, &clock);
        }
        // Se vacían los avisos de esas fases.
        clock.advance(Duration::from_secs(30));
        for _ in 0..4 {
            app.update();
            clock.advance(Duration::from_secs(5));
            app.update();
        }
        press(&mut app, KeyCode::Char(' '));
        clock.advance(app.timer.planned().saturating_sub(left));
        app.update();
        (app, clock)
    }

    fn draw(app: &App, w: u16, h: u16) -> Buffer {
        let Ok(mut t) = Terminal::new(TestBackend::new(w, h));
        let _ = t.draw(|f| ui::draw(f, app));
        t.backend().buffer().clone()
    }

    /// Terminal con un comando y su salida real (texto plano en colores de la paleta).
    fn shell(cmd: &str, out: &str, w: u16) -> Buffer {
        let lines: Vec<&str> = out.lines().collect();
        let h = u16::try_from(lines.len() + 3).unwrap_or(10);
        let mut b = Buffer::empty(Rect::new(0, 0, w, h));
        b.set_string(1, 0, "$ ", Style::new().fg(theme::OK));
        b.set_string(3, 0, cmd, Style::new().fg(theme::TEXT));
        for (i, l) in lines.iter().enumerate() {
            let y = u16::try_from(i + 1).unwrap_or(0);
            let (head, tail) = l.split_at(l.char_indices().nth(9).map_or(l.len(), |(i, _)| i));
            let first = i == 0;
            b.set_string(
                1,
                y,
                head,
                Style::new().fg(if first {
                    theme::accent(0.2)
                } else {
                    theme::MUTED
                }),
            );
            b.set_string(
                1 + u16::try_from(head.chars().count()).unwrap_or(0),
                y,
                tail,
                Style::new().fg(if first {
                    theme::accent(0.2)
                } else {
                    theme::TEXT
                }),
            );
        }
        b.set_string(1, h - 2, "$ ", Style::new().fg(theme::OK));
        b
    }

    fn write(name: &str, svg: &str) {
        let path = format!("{}/docs/{name}", env!("CARGO_MANIFEST_DIR"));
        if let Err(e) = std::fs::write(&path, svg) {
            panic!("{path}: {e}");
        }
    }

    #[test]
    fn demo_dibuja_en_todos_los_tamanos() {
        let (app, _c) = running_third_focus(MIN * 12);
        assert_eq!(app.timer.cycle(), Some((3, 4)));
        assert!(app.timer.is_running());
        for (w, h) in [(40, 12), (80, 24), (100, 30), (140, 40), (220, 60)] {
            let _ = draw(&app, w, h);
        }
    }

    #[test]
    #[ignore = "regenera docs/tui-*.svg y docs/cli-stats.svg"]
    fn snapshots() {
        let name = env!("CARGO_BIN_NAME");
        let (mut app, clock) = running_third_focus(MIN * 12 + Duration::from_secs(48));
        write("tui-focus.svg", &svg::to_svg(&draw(&app, 140, 40), name));

        // Descanso: termina el foco y arranca el descanso corto.
        clock.advance(MIN * 13);
        app.update();
        press(&mut app, KeyCode::Char(' '));
        clock.advance(MIN * 2 + Duration::from_secs(10));
        app.update();
        write("tui-break.svg", &svg::to_svg(&draw(&app, 100, 30), name));

        let (mut picker, _c) = demo_app(None);
        picker.modal = Some(Modal::Sessions {
            idx: 1,
            initial: true,
        });
        write(
            "tui-sessions.svg",
            &svg::to_svg(&draw(&picker, 100, 30), name),
        );

        let today = jiff::civil::date(2026, 3, 14);
        let l = demo_loaded(today);
        let report = crate::stats::text_report(&l.history, &l.config, today);
        let buf = shell(&format!("{name} stats"), &report, 64);
        write(
            "cli-stats.svg",
            &svg::to_svg(&buf, &format!("{name} stats")),
        );
    }

    #[test]
    #[ignore = "regenera docs/demo.svg"]
    fn demo() {
        let (w, h) = (100, 30);
        let mut frames: Vec<(Buffer, f64, bool)> = Vec::new();

        // 1. Al abrir: selector de sesión; baja a Deep work y vuelve a Clásico.
        let (mut app, clock) = demo_app(None);
        frames.push((draw(&app, w, h), 1.6, false));
        press(&mut app, KeyCode::Down);
        frames.push((draw(&app, w, h), 0.9, false));
        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Enter);
        clock.advance(Duration::from_secs(5));
        app.update();
        frames.push((draw(&app, w, h), 1.4, false));

        // 2. Espacio: corre el foco; los últimos segundos y el cierre.
        let (mut app, clock) = running_third_focus(MIN * 25 - Duration::from_secs(1));
        for i in 0..2 {
            frames.push((draw(&app, w, h), 1.0, i == 1));
            clock.advance(Duration::from_secs(1));
            app.update();
        }
        clock.advance(MIN * 25 - Duration::from_secs(6));
        app.update();
        for _ in 0..2 {
            frames.push((draw(&app, w, h), 0.8, false));
            clock.advance(Duration::from_secs(1));
            app.update();
        }
        clock.advance(Duration::from_secs(2));
        app.update();
        // Foco completado: aviso, Hoy +1, nueva línea en el log, descanso en espera.
        frames.push((draw(&app, w, h), 2.6, false));

        // 3. Descanso corriendo (verde).
        press(&mut app, KeyCode::Char(' '));
        clock.advance(Duration::from_secs(5));
        app.update();
        for _ in 0..2 {
            frames.push((draw(&app, w, h), 1.0, false));
            clock.advance(Duration::from_secs(1));
            app.update();
        }

        let svg = svg::to_animated_svg(&frames, &format!("{} — demo", env!("CARGO_BIN_NAME")));
        write("demo.svg", &svg);
        assert!(svg.len() < 700_000, "demo.svg pesa {} B", svg.len());
    }
}
