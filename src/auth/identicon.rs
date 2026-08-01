//! GitHub-style 5×5 mirrored identicons from a seed string.

use sha2::{Digest, Sha256};

/// Render a crisp SVG identicon for `seed` (typically a user id).
pub fn svg_for_seed(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let (r, g, b) = foreground_color(&digest);
    let color = format!("#{r:02x}{g:02x}{b:02x}");

    let mut out = String::with_capacity(512);
    out.push_str(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 5 5" shape-rendering="crispEdges">"##,
    );
    out.push_str(r##"<rect width="5" height="5" fill="#f0f0f0"/>"##);

    // Same layout as GitHub/identicon.js: 15 cells for center + left half, mirrored right.
    for i in 0..15 {
        // Even nibble → foreground (matches identicon.js).
        if digest[i] % 2 != 0 {
            continue;
        }
        let (col, row) = match i {
            0..=4 => (2u8, i as u8),
            5..=9 => (1, (i - 5) as u8),
            _ => (0, (i - 10) as u8),
        };
        push_cell(&mut out, col, row, &color);
        if col != 2 {
            push_cell(&mut out, 4 - col, row, &color);
        }
    }

    out.push_str("</svg>");
    out
}

fn push_cell(out: &mut String, x: u8, y: u8, color: &str) {
    out.push_str(&format!(
        r#"<rect x="{x}" y="{y}" width="1" height="1" fill="{color}"/>"#
    ));
}

/// HSL → RGB with GitHub-like saturation/lightness from the hash.
fn foreground_color(digest: &[u8]) -> (u8, u8, u8) {
    let n = u32::from_be_bytes([digest[28], digest[29], digest[30], digest[31]]);
    let hue = (n as f64 / 0xffff_ffffu32 as f64) * 360.0;
    hsl_to_rgb(hue, 0.65, 0.45)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u8 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_is_deterministic() {
        let a = svg_for_seed("11111111-1111-1111-1111-111111111111");
        let b = svg_for_seed("11111111-1111-1111-1111-111111111111");
        assert_eq!(a, b);
        assert!(a.starts_with("<svg"));
        assert!(a.contains("shape-rendering=\"crispEdges\""));
    }

    #[test]
    fn different_seeds_differ() {
        let a = svg_for_seed("user-a");
        let b = svg_for_seed("user-b");
        assert_ne!(a, b);
    }
}
