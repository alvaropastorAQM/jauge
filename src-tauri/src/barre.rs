//! The menu bar ring, drawn pixel by pixel.

use crate::rythme::Ton;

pub const COTE: u32 = 36;

/// macOS system green, orange and red, readable on a light bar as well as a dark one.
fn couleur(ton: Option<Ton>) -> Option<[f64; 3]> {
    match ton? {
        Ton::Marge => Some([48.0, 209.0, 88.0]),
        Ton::Attention => Some([255.0, 159.0, 10.0]),
        Ton::Mauvais => Some([255.0, 69.0, 58.0]),
        Ton::Normal => None,
    }
}

/// Ring filled to the 5-hour session, in the colours of its bar's veil (like Fil): green with room to spare, orange when ahead,
/// red when really ahead. On pace or with no reading, a template image that macOS tints like its icons (the returned boolean).
/// RGBA, 36 px for 18 points on a Retina screen.
pub fn anneau(pourcent: Option<f64>, ton: Option<Ton>) -> (Vec<u8>, bool) {
    let (t, grand, petit, n) = (COTE as f64, 16.0, 11.5, 4);
    let part = pourcent.map_or(0.0, |p| (p / 100.0).clamp(0.0, 1.0));
    let c = couleur(ton);
    let mut px = vec![0u8; (COTE * COTE * 4) as usize];
    for y in 0..COTE {
        for x in 0..COTE {
            let (mut arc, mut piste) = (0.0, 0.0);
            for sy in 0..n {
                for sx in 0..n {
                    let dx = x as f64 + (sx as f64 + 0.5) / n as f64 - t / 2.0;
                    let dy = y as f64 + (sy as f64 + 0.5) / n as f64 - t / 2.0;
                    let d = dx.hypot(dy);
                    if d > grand || d < petit {
                        continue;
                    }
                    let angle = (dx.atan2(-dy) / std::f64::consts::TAU + 1.0) % 1.0; // 0 at the top, clockwise
                    if angle < part { arc += 1.0 } else { piste += 1.0 }
                }
            }
            let ech = (n * n) as f64;
            let i = ((y * COTE + x) * 4) as usize;
            let a_arc = arc / ech;
            let Some(c) = c else {
                px[i + 3] = ((a_arc + 0.3 * piste / ech) * 255.0).round() as u8;
                continue;
            };
            // The arc in colour, the track in half-transparent grey (non-premultiplied alpha).
            let a_piste = 0.45 * piste / ech;
            let a = a_arc + a_piste;
            if a > 0.0 {
                for k in 0..3 {
                    px[i + k] = ((c[k] * a_arc + 128.0 * a_piste) / a).round() as u8;
                }
                px[i + 3] = (a * 255.0).round() as u8;
            }
        }
    }
    (px, c.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anneau_rempli_et_couleur() {
        let (vide, modele) = anneau(None, None);
        assert!(modele);
        // Top centre: the ring. Centre: nothing.
        let haut = ((2 * COTE + COTE / 2) * 4 + 3) as usize;
        let centre = ((COTE / 2 * COTE + COTE / 2) * 4 + 3) as usize;
        assert!(vide[haut] > 0 && vide[centre] == 0);
        let (vert, modele) = anneau(Some(50.0), Some(Ton::Marge));
        assert!(!modele);
        // On the right (the quarter hour), inside the arc: solid green.
        let droite = ((COTE / 2 * COTE + COTE - 3) * 4) as usize;
        assert_eq!(&vert[droite..droite + 4], &[48, 209, 88, 255]);
    }
}
