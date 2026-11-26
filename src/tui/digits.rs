//! Reloj grande: fuente de 3×5 "píxeles" escalable. Cada píxel ocupa `sx`
//! columnas y `sy` filas; con `sx = 2·sy` se ve cuadrado, porque las celdas
//! de la terminal son el doble de altas que de anchas.

const ROWS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    pub sx: usize,
    pub sy: usize,
}

impl Scale {
    pub const SMALL: Self = Self { sx: 1, sy: 1 };
    pub const WIDE: Self = Self { sx: 2, sy: 1 };
    pub const BIG: Self = Self { sx: 4, sy: 2 };

    pub fn rows(self) -> usize {
        ROWS * self.sy
    }

    fn gap(self) -> usize {
        (self.sx / 2).max(1)
    }
}

fn glyph(c: char) -> [&'static str; ROWS] {
    match c {
        '0' => ["###", "# #", "# #", "# #", "###"],
        '1' => [" # ", "## ", " # ", " # ", "###"],
        '2' => ["###", "  #", "###", "#  ", "###"],
        '3' => ["###", "  #", "###", "  #", "###"],
        '4' => ["# #", "# #", "###", "  #", "  #"],
        '5' => ["###", "#  ", "###", "  #", "###"],
        '6' => ["###", "#  ", "###", "# #", "###"],
        '7' => ["###", "  #", "  #", "  #", "  #"],
        '8' => ["###", "# #", "###", "# #", "###"],
        '9' => ["###", "# #", "###", "  #", "###"],
        ':' => [" ", "#", " ", "#", " "],
        '-' => ["   ", "   ", "###", "   ", "   "],
        _ => ["   "; ROWS],
    }
}

/// Columnas que ocupa `text` a esta escala.
pub fn width(text: &str, scale: Scale) -> usize {
    let px: usize = text.chars().map(|c| glyph(c)[0].len()).sum();
    let gaps = text.chars().count().saturating_sub(1);
    px * scale.sx + gaps * scale.gap()
}

/// Las `scale.rows()` filas de `text` ya renderizadas con bloques.
pub fn render(text: &str, scale: Scale) -> Vec<String> {
    let on = "█".repeat(scale.sx);
    let off = " ".repeat(scale.sx);
    let gap = " ".repeat(scale.gap());
    (0..scale.rows())
        .map(|row| {
            text.chars()
                .map(|c| {
                    glyph(c)[row / scale.sy]
                        .chars()
                        .map(|p| if p == '#' { on.as_str() } else { off.as_str() })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join(&gap)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancho_coincide_con_render() {
        for scale in [Scale::SMALL, Scale::WIDE, Scale::BIG] {
            let rows = render("180:00", scale);
            assert_eq!(rows.len(), scale.rows());
            for r in &rows {
                assert_eq!(r.chars().count(), width("180:00", scale));
            }
        }
    }
}
