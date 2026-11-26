//! Layout adaptable: decide qué paneles caben y dónde, sin dibujar nada.
//!
//! | ancho      | disposición                                                     |
//! |------------|-----------------------------------------------------------------|
//! | < 100      | una columna: Timer + Hoy fijos; el resto apilado o en rotación  |
//! | 100–139    | dos columnas 50/50: [Timer, Hoy, Distribución] · [Actividad, Tareas, Log] |
//! | ≥ 140      | dos columnas, la izquierda de 70 con dígitos grandes            |
//!
//! Lo que no cabe comparte un hueco que rota con `Tab`.

use ratatui::layout::Rect;

pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 12;
pub const MEDIUM_W: u16 = 100;
pub const WIDE_W: u16 = 140;
const WIDE_LEFT: u16 = 70;
/// Altura mínima de un hueco en rotación (borde + 3 líneas).
const ROT_MIN: u16 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelId {
    Timer,
    Today,
    Dist,
    Activity,
    Tasks,
    Log,
}

impl PanelId {
    pub const ALL: [Self; 6] = [
        Self::Timer,
        Self::Today,
        Self::Dist,
        Self::Activity,
        Self::Tasks,
        Self::Log,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Timer => "Timer",
            Self::Today => "Hoy",
            Self::Dist => "Distribución",
            Self::Activity => "Racha y actividad",
            Self::Tasks => "Tareas",
            Self::Log => "Log",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Altura preferida (con bordes).
    fn pref_h(self) -> u16 {
        match self {
            Self::Timer => 10,
            Self::Today => 4,
            Self::Dist => 7,
            Self::Activity => 12,
            Self::Tasks => 8,
            Self::Log => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Narrow,
    Medium,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub id: PanelId,
    pub rect: Rect,
    /// En un hueco que rota: (posición 1-based, total).
    pub rot: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub mode: Mode,
    pub header: Rect,
    pub footer: Rect,
    pub panels: Vec<Placed>,
    /// Dígitos a escala grande en el Timer.
    pub big_timer: bool,
}

impl Screen {
    #[cfg(test)]
    pub fn visible(&self, id: PanelId) -> bool {
        self.panels.iter().any(|p| p.id == id)
    }
}

/// `pick` elige qué panel mostrar en un hueco que rota, entre sus miembros.
pub fn compute(area: Rect, pick: &dyn Fn(&[PanelId]) -> PanelId) -> Option<Screen> {
    if area.width < MIN_W || area.height < MIN_H {
        return None;
    }
    let header = Rect::new(area.x, area.y, area.width, 1);
    let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 2);
    let h = body.height;

    let two_cols = area.width >= MEDIUM_W && h >= 18;
    if !two_cols {
        let timer_h = if h >= 14 { 10 } else { 5 };
        let today_h = if h >= timer_h + 4 { 4 } else { 3 };
        let panels = column(
            body,
            &[(PanelId::Timer, timer_h), (PanelId::Today, today_h)],
            &[
                PanelId::Dist,
                PanelId::Activity,
                PanelId::Tasks,
                PanelId::Log,
            ],
            pick,
        );
        return Some(Screen {
            mode: Mode::Narrow,
            header,
            footer,
            panels,
            big_timer: false,
        });
    }

    let wide = area.width >= WIDE_W;
    let left_w = if wide { WIDE_LEFT } else { area.width / 2 };
    let left = Rect::new(body.x, body.y, left_w, h);
    let right = Rect::new(body.x + left_w, body.y, body.width - left_w, h);
    let big = wide && h >= 15 + 4 + ROT_MIN;
    let timer_h = if big { 15 } else { 10 };
    let mut panels = column(
        left,
        &[(PanelId::Timer, timer_h), (PanelId::Today, 4)],
        &[PanelId::Dist],
        pick,
    );
    panels.extend(column(
        right,
        &[],
        &[PanelId::Activity, PanelId::Tasks, PanelId::Log],
        pick,
    ));
    Some(Screen {
        mode: if wide { Mode::Wide } else { Mode::Medium },
        header,
        footer,
        panels,
        big_timer: big,
    })
}

/// Apila `fixed` arriba; luego tantos `flex` como quepan y el resto en un hueco rotatorio.
fn column(
    area: Rect,
    fixed: &[(PanelId, u16)],
    flex: &[PanelId],
    pick: &dyn Fn(&[PanelId]) -> PanelId,
) -> Vec<Placed> {
    let mut out = Vec::new();
    let mut y = area.y;
    let bottom = area.bottom();
    let rem = |y: u16| bottom.saturating_sub(y);

    for &(id, h) in fixed {
        let h = h.min(rem(y));
        if h == 0 {
            return out;
        }
        out.push(Placed {
            id,
            rect: Rect::new(area.x, y, area.width, h),
            rot: None,
        });
        y += h;
    }

    let total: u16 = flex.iter().map(|p| p.pref_h()).sum();
    let (stacked, rotating): (&[PanelId], &[PanelId]) = if total <= rem(y) {
        (flex, &[])
    } else {
        let mut n = 0;
        let mut used = 0;
        for p in flex {
            // Tras este, tiene que quedar sitio para el hueco que rota.
            if used + p.pref_h() + ROT_MIN > rem(y) {
                break;
            }
            used += p.pref_h();
            n += 1;
        }
        flex.split_at(n)
    };

    for (i, &id) in stacked.iter().enumerate() {
        let last = i + 1 == stacked.len() && rotating.is_empty();
        let h = if last { rem(y) } else { id.pref_h() };
        out.push(Placed {
            id,
            rect: Rect::new(area.x, y, area.width, h),
            rot: None,
        });
        y += h;
    }

    if !rotating.is_empty() && rem(y) >= 3 {
        let shown = pick(rotating);
        let shown = if rotating.contains(&shown) {
            shown
        } else {
            rotating[0]
        };
        let pos = rotating.iter().position(|p| *p == shown).unwrap_or(0);
        out.push(Placed {
            id: shown,
            rect: Rect::new(area.x, y, area.width, rem(y)),
            rot: (rotating.len() > 1).then_some((pos + 1, rotating.len())),
        });
    } else if let Some(last) = out.last_mut() {
        // Sin hueco útil: el último panel se queda con lo que sobra.
        last.rect.height = bottom - last.rect.y;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(m: &[PanelId]) -> PanelId {
        m[0]
    }

    fn at(w: u16, h: u16) -> Option<Screen> {
        compute(Rect::new(0, 0, w, h), &first)
    }

    fn ids(s: &Screen) -> Vec<PanelId> {
        s.panels.iter().map(|p| p.id).collect()
    }

    /// Ningún panel se sale del área ni se solapa con otro.
    fn sane(s: &Screen, w: u16, h: u16) {
        let area = Rect::new(0, 0, w, h);
        for (i, a) in s.panels.iter().enumerate() {
            assert!(a.rect.height > 0 && a.rect.width > 0, "{a:?}");
            assert_eq!(a.rect.intersection(area), a.rect, "{a:?} fuera de {w}x{h}");
            for b in &s.panels[i + 1..] {
                assert!(!a.rect.intersects(b.rect), "{a:?} solapa {b:?}");
            }
        }
    }

    #[test]
    fn menor_que_minimo() {
        assert!(at(39, 24).is_none());
        assert!(at(80, 11).is_none());
        assert!(at(40, 12).is_some());
    }

    #[test]
    fn corte_80x24() {
        let s = at(80, 24);
        let Some(s) = s else {
            panic!("80x24 debe caber")
        };
        assert_eq!(s.mode, Mode::Narrow);
        assert_eq!(ids(&s), [PanelId::Timer, PanelId::Today, PanelId::Dist]);
        assert_eq!(s.panels[2].rot, Some((1, 4)));
        assert_eq!(s.panels[0].rect.height, 10);
        sane(&s, 80, 24);
    }

    #[test]
    fn estrecho_y_alto_apila_todo() {
        let Some(s) = at(80, 60) else { panic!() };
        assert_eq!(s.mode, Mode::Narrow);
        assert_eq!(ids(&s), PanelId::ALL);
        assert!(s.panels.iter().all(|p| p.rot.is_none()));
        assert_eq!(s.panels.last().map(|p| p.rect.bottom()), Some(59));
        sane(&s, 80, 60);
    }

    #[test]
    fn corte_100x30_todo_visible() {
        let Some(s) = at(100, 30) else { panic!() };
        assert_eq!(s.mode, Mode::Medium);
        assert!(!s.big_timer);
        assert_eq!(ids(&s), PanelId::ALL);
        assert!(s.panels.iter().all(|p| p.rot.is_none()));
        assert_eq!(s.panels[0].rect.width, 50);
        sane(&s, 100, 30);
    }

    #[test]
    fn medio_100x24_rota_tareas_y_log() {
        let Some(s) = at(100, 24) else { panic!() };
        assert_eq!(s.mode, Mode::Medium);
        assert_eq!(
            ids(&s),
            [
                PanelId::Timer,
                PanelId::Today,
                PanelId::Dist,
                PanelId::Activity,
                PanelId::Tasks
            ]
        );
        assert_eq!(s.panels[4].rot, Some((1, 2)));
        sane(&s, 100, 24);
    }

    #[test]
    fn corte_140x40_todo_visible() {
        let Some(s) = at(140, 40) else { panic!() };
        assert_eq!(s.mode, Mode::Wide);
        assert!(s.big_timer);
        assert_eq!(ids(&s).len(), 6);
        assert!(s.panels.iter().all(|p| p.rot.is_none()));
        assert_eq!(s.panels[0].rect.width, 70);
        sane(&s, 140, 40);
    }

    #[test]
    fn ancho_pero_bajo_rota() {
        let Some(s) = at(160, 24) else { panic!() };
        assert_eq!(s.mode, Mode::Wide);
        assert!(!s.big_timer);
        assert!(s.panels.iter().any(|p| p.rot.is_some()));
        sane(&s, 160, 24);
    }

    #[test]
    fn minimo_40x12() {
        let Some(s) = at(40, 12) else { panic!() };
        assert_eq!(ids(&s), [PanelId::Timer, PanelId::Today]);
        sane(&s, 40, 12);
    }

    #[test]
    fn pick_elige_miembro_del_hueco() {
        let s = compute(Rect::new(0, 0, 80, 24), &|m: &[PanelId]| {
            *m.last().unwrap_or(&PanelId::Dist)
        });
        let s = s.map(|s| s.panels[2]);
        assert_eq!(s.map(|p| (p.id, p.rot)), Some((PanelId::Log, Some((4, 4)))));
    }

    #[test]
    fn todos_los_tamanos_son_sanos() {
        for w in 40..=180 {
            for h in 12..=60 {
                if let Some(s) = at(w, h) {
                    sane(&s, w, h);
                    assert!(s.visible(PanelId::Timer), "{w}x{h}");
                }
            }
        }
    }

    #[test]
    fn tab_recorre_todos() {
        let mut p = PanelId::Timer;
        for _ in 0..6 {
            p = p.next();
        }
        assert_eq!(p, PanelId::Timer);
        assert_eq!(PanelId::Timer.prev(), PanelId::Log);
    }
}
