//! WCAG contrast math for the resolved theme projection.

use super::Color;

/// WCAG 2.x relative luminance for sRGB.
fn relative_luminance(c: Color) -> f32 {
    let Color::Rgb(r, g, b) = c;
    let to_lin = |c: u8| -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * to_lin(r) + 0.7152 * to_lin(g) + 0.0722 * to_lin(b)
}

/// WCAG contrast ratio between two colors.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(hex: u32) -> Color {
        Color::Rgb(
            ((hex >> 16) & 0xff) as u8,
            ((hex >> 8) & 0xff) as u8,
            (hex & 0xff) as u8,
        )
    }

    #[test]
    fn contrast_ratio_math() {
        for (fg, bg, want) in [(0x000000, 0xffffff, 21.0), (0x6272a4, 0x6272a4, 1.0)] {
            let r = contrast_ratio(rgb(fg), rgb(bg));
            assert!((r - want).abs() < 0.01, "{fg:06x}/{bg:06x}: {r}");
        }
        let a = contrast_ratio(rgb(0x3c3c3c), rgb(0x50785a));
        let b = contrast_ratio(rgb(0x50785a), rgb(0x3c3c3c));
        assert!((a - b).abs() < 0.001, "symmetric");
    }
}
