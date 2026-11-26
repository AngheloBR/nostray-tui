//! SVG de las capturas del README a partir de un `Buffer` de ratatui (solo tests).
//! Mismo formato que en nosleep-tui y nodirt-tui: marco de ventana, fondos por celda
//! y texto agrupado por estilo; `to_animated_svg` encadena frames con CSS.

use std::fmt::Write as _;

use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier},
};

const BG: &str = "#0b1120";
const DEFAULT_FG: &str = "#e2e8f0";
const CW: f64 = 8.6;
const LH: f64 = 19.0;

fn hex(c: Color) -> Option<String> {
    match c {
        Color::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        Color::Reset => None,
        other => Some(format!("{other:?}").to_lowercase()),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const PAD_X: f64 = 16.0;
const TOP: f64 = 40.0;

/// Marco de la ventana (fondo, semáforos y título). Devuelve el SVG abierto.
fn chrome(w: u16, h: u16, title: &str, extra_style: &str) -> String {
    let width = f64::from(w) * CW + PAD_X * 2.0;
    let height = f64::from(h) * LH + TOP + 14.0;
    let mut s = String::new();
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" xml:space="preserve" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">
<style>text{{font-family:"JetBrains Mono","Fira Code","DejaVu Sans Mono",Menlo,Consolas,monospace;font-size:14.3px;white-space:pre}}{extra_style}</style>
<rect width="100%" height="100%" rx="12" fill="{BG}" stroke="#1e293b"/>
<circle cx="22" cy="20" r="6" fill="#ff5f57"/><circle cx="42" cy="20" r="6" fill="#febc2e"/><circle cx="62" cy="20" r="6" fill="#28c840"/>
<text x="{:.0}" y="25" fill="#64748b" text-anchor="middle" style="font-size:12px">{}</text>
"##,
        width / 2.0,
        esc(title)
    );
    s
}

/// Contenido de la terminal: fondos por celda y texto agrupado por estilo.
fn cells(buf: &Buffer) -> String {
    let (w, h) = (buf.area.width, buf.area.height);
    let (pw, top) = (PAD_X, TOP);
    let mut s = String::new();
    for y in 0..h {
        let ty = top + f64::from(y) * LH;
        // Fondos por celda.
        for x in 0..w {
            let cell = &buf[(x, y)];
            if cell.symbol() == "█" {
                let fill = hex(cell.fg).unwrap_or_else(|| DEFAULT_FG.to_owned());
                let _ = writeln!(
                    s,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{fill}"/>"#,
                    pw + f64::from(x) * CW,
                    ty,
                    CW + 0.4,
                    LH + 0.4
                );
                continue;
            }
            if let Some(bg) = hex(cell.bg) {
                let _ = writeln!(
                    s,
                    r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{LH}" fill="{bg}"/>"#,
                    pw + f64::from(x) * CW,
                    ty,
                    CW + 0.4
                );
            }
        }
        // Texto agrupado en tramos del mismo estilo.
        let mut x = 0;
        while x < w {
            let c0 = &buf[(x, y)];
            let style = (c0.fg, c0.modifier.contains(Modifier::BOLD));
            let start = x;
            let mut text = String::new();
            while x < w {
                let c = &buf[(x, y)];
                if (c.fg, c.modifier.contains(Modifier::BOLD)) != style {
                    break;
                }
                text.push_str(if c.symbol() == "█" { " " } else { c.symbol() });
                x += 1;
            }
            if text.trim().is_empty() {
                continue;
            }
            let fill = hex(style.0).unwrap_or_else(|| DEFAULT_FG.to_owned());
            let weight = if style.1 { r#" font-weight="700""# } else { "" };
            let n = x - start;
            let _ = writeln!(
                s,
                r#"<text x="{:.1}" y="{:.1}" fill="{fill}"{weight} textLength="{:.1}" lengthAdjust="spacingAndGlyphs">{}</text>"#,
                pw + f64::from(start) * CW,
                ty + LH * 0.74,
                f64::from(n) * CW,
                esc(&text)
            );
        }
    }
    s
}

pub fn to_svg(buf: &Buffer, title: &str) -> String {
    let mut s = chrome(buf.area.width, buf.area.height, title, "");
    s.push_str(&cells(buf));
    s.push_str("</svg>\n");
    s
}

/// SVG animado: cada frame es un grupo que solo se ve en su tramo del bucle.
/// El primero de `frames` con `poster = true` es lo que muestra un visor sin animación.
pub fn to_animated_svg(frames: &[(Buffer, f64, bool)], title: &str) -> String {
    let total: f64 = frames.iter().map(|f| f.1).sum();
    let Some((first, _, _)) = frames.first() else {
        return String::new();
    };
    let mut css = String::from("g{opacity:0}g.poster{opacity:1}");
    let mut t = 0.0;
    for (i, (_, secs, _)) in frames.iter().enumerate() {
        let (a, b) = (t / total * 100.0, (t + secs) / total * 100.0);
        // step-end: cada valor se mantiene hasta el siguiente keyframe.
        let start = if a > 0.0 {
            format!("0%{{opacity:0}}{a:.3}%{{opacity:1}}")
        } else {
            "0%{opacity:1}".to_owned()
        };
        let _ = write!(
            css,
            "@keyframes k{i}{{{start}{b:.3}%{{opacity:0}}}}\
             .f{i}{{animation:k{i} {total:.1}s step-end infinite}}"
        );
        t += secs;
    }
    let mut s = chrome(first.area.width, first.area.height, title, &css);
    let mut poster_done = false;
    for (i, (buf, _, poster)) in frames.iter().enumerate() {
        let class = if *poster && !poster_done {
            poster_done = true;
            format!("f{i} poster")
        } else {
            format!("f{i}")
        };
        let _ = writeln!(s, r#"<g class="{class}">"#);
        s.push_str(&cells(buf));
        s.push_str("</g>\n");
    }
    s.push_str("</svg>\n");
    s
}
