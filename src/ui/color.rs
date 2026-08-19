use ratatui::style::Color;

pub trait ColorExt {
    fn lighten(&self, amount: f32) -> Color;
    fn darken(&self, amount: f32) -> Color;
}

impl ColorExt for Color {
    fn lighten(&self, amount: f32) -> Color {
        adjust_lightness(*self, amount.abs())
    }

    fn darken(&self, amount: f32) -> Color {
        adjust_lightness(*self, -amount.abs())
    }
}

// Thanks gemini
fn adjust_lightness(color: Color, delta: f32) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };

    // 1. Normalize RGB to 0.0 - 1.0 range
    let r_norm = r as f32 / 255.0;
    let g_norm = g as f32 / 255.0;
    let b_norm = b as f32 / 255.0;

    let max = r_norm.max(g_norm).max(b_norm);
    let min = r_norm.min(g_norm).min(b_norm);
    let chroma = max - min;

    let mut l = (max + min) / 2.0;

    // If completely achromatic (grayscale), adjust lightness directly
    if chroma < 1e-6 {
        l = (l + delta).clamp(0.0, 1.0);
        let val = (l * 255.0).round() as u8;
        return Color::Rgb(val, val, val);
    }

    let mut h = if max == r_norm {
        ((g_norm - b_norm) / chroma) % 6.0
    } else if max == g_norm {
        ((b_norm - r_norm) / chroma) + 2.0
    } else {
        ((r_norm - g_norm) / chroma) + 4.0
    } * 60.0;

    if h < 0.0 {
        h += 360.0;
    }

    let s = chroma / (1.0 - (2.0 * l - 1.0).abs());

    l = (l + delta).clamp(0.0, 1.0);

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r_prime, g_prime, b_prime) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Color::Rgb(
        ((r_prime + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g_prime + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b_prime + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}
