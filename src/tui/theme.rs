//! Paleta truecolor. El fondo nunca se pinta: se respeta el tema de la terminal.

use ratatui::style::{Color, Modifier, Style};

pub const TEXT: Color = Color::Rgb(226, 232, 240);
pub const MUTED: Color = Color::Rgb(148, 163, 184);
pub const DIM: Color = Color::Rgb(100, 116, 139);
pub const FAINT: Color = Color::Rgb(51, 65, 85);
pub const BORDER: Color = Color::Rgb(51, 65, 85);
pub const KEY_BG: Color = Color::Rgb(30, 41, 59);
pub const DARK: Color = Color::Rgb(15, 23, 42);
pub const IDLE_DIGITS: Color = Color::Rgb(71, 85, 105);

pub const OK: Color = Color::Rgb(52, 211, 153);
pub const WARN: Color = Color::Rgb(251, 191, 36);
pub const ERR: Color = Color::Rgb(248, 113, 113);

const A: (u8, u8, u8) = (167, 139, 250); // violeta
const B: (u8, u8, u8) = (34, 211, 238); // cian

/// Degradado de la marca: `t` = 0.0 violeta → 1.0 cian.
pub fn accent(t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
    Color::Rgb(mix(A.0, B.0), mix(A.1, B.1), mix(A.2, B.2))
}

pub fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

pub fn pill(fg: Color, bg: Color) -> Style {
    Style::new().fg(fg).bg(bg).add_modifier(Modifier::BOLD)
}

/// Color de una celda del heatmap por nivel 0–4: de `FAINT` (vacío) al acento.
pub fn heat(level: u8) -> Color {
    let Color::Rgb(r0, g0, b0) = FAINT else {
        return FAINT;
    };
    let Color::Rgb(r1, g1, b1) = accent(0.75) else {
        return FAINT;
    };
    let t = f64::from(level.min(4)) / 4.0;
    let mix = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
    Color::Rgb(mix(r0, r1), mix(g0, g1), mix(b0, b1))
}
