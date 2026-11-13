//! Pace of the limits, for the menu bar. Same rules as ui/js/rythme.js, which serves the panel: both are tested
//! on the same cases.

use crate::limites::Limite;

pub const MINUTE: i64 = 60_000;
pub const HEURE: i64 = 60 * MINUTE;
pub const JOUR: i64 = 24 * HEURE;
/// Same thresholds as Fil: within 5 points we are on pace, at 15 points we are really ahead.
const TOLERANCE: f64 = 0.05;
const TRES_EN_AVANCE: f64 = 0.15;

pub fn duree(l: &Limite) -> i64 {
    if l.groupe == "session" { 5 * HEURE } else { 7 * JOUR }
}

/// Gap between usage and the share of the window that has elapsed (positive: usage is going faster than time).
fn ecart(l: &Limite, t: i64) -> Option<f64> {
    let ecoule = (1.0 - (l.reinit? - t) as f64 / duree(l) as f64).clamp(0.0, 1.0);
    Some(l.pourcent? / 100.0 - ecoule)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ton {
    Normal,
    Marge,
    Attention,
    Mauvais,
}

/// Like Fil: green when there is room to spare on the clock, neutral on pace, yellow when ahead, red when really ahead.
pub fn ton(l: &Limite, t: i64) -> Ton {
    match ecart(l, t) {
        Some(e) if e >= TRES_EN_AVANCE => Ton::Mauvais,
        Some(e) if e > TOLERANCE => Ton::Attention,
        Some(e) if e < 0.0 => Ton::Marge,
        _ => Ton::Normal,
    }
}

/// Once its reset has passed, a limit starts again from 0 without waiting for the next reading: the week for a new 7-day
/// window, the session at the next message (no time until it is sent).
pub fn a_jour(limites: &[Limite], t: i64) -> Vec<Limite> {
    limites
        .iter()
        .map(|l| match l.reinit {
            Some(r) if r <= t => Limite {
                pourcent: Some(0.0),
                reinit: (l.groupe != "session").then(|| r + ((t - r) / duree(l) + 1) * duree(l)),
                ..l.clone()
            },
            _ => l.clone(),
        })
        .collect()
}

pub fn plus_proche(limites: &[Limite]) -> Option<&Limite> {
    limites.iter().filter(|l| l.pourcent.is_some()).fold(None, |a: Option<&Limite>, l| match a {
        Some(a) if l.pourcent <= a.pourcent => Some(a),
        _ => Some(l),
    })
}

pub fn libelle(l: &Limite) -> String {
    if l.groupe == "session" {
        return "Session".into();
    }
    if let Some(nom) = l.modele.as_ref().or(l.surface.as_ref()) {
        return nom.clone();
    }
    match l.sorte.as_deref() {
        // A kind of limit that Jauge does not know yet gets a name of its own, so there are never two "Weekly" cards.
        Some(s) if s != "weekly" && s != "weekly_all" => {
            format!("Weekly · {}", s.trim_start_matches("weekly").trim_start_matches('_').replace('_', " "))
        }
        _ => "Weekly".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limite(groupe: &str, pourcent: f64, reinit: Option<i64>) -> Limite {
        Limite { groupe: groupe.into(), sorte: None, pourcent: Some(pourcent), gravite: None, reinit, modele: None, surface: None, vu: None }
    }

    #[test]
    fn tons_comme_fil() {
        let t = 1_800_000_000_000;
        let session = |p| limite("session", p, Some(t + 5 * HEURE / 2));
        assert_eq!(ton(&limite("session", 86.0, Some(t + 2 * HEURE)), t), Ton::Mauvais);
        assert_eq!(ton(&session(57.0), t), Ton::Attention);
        assert_eq!(ton(&session(52.0), t), Ton::Normal);
        assert_eq!(ton(&session(48.0), t), Ton::Marge);
        assert_eq!(ton(&limite("weekly", 38.0, Some(t + 7 * JOUR / 2)), t), Ton::Marge);
        assert_eq!(ton(&limite("weekly", 95.0, None), t), Ton::Normal);
    }

    #[test]
    fn remise_a_zero_sans_attendre() {
        let fin = 1_800_000_000_000;
        let l = [limite("session", 40.0, Some(fin - HEURE)), limite("weekly", 74.0, Some(fin)), limite("weekly", 10.0, Some(fin + 1))];
        let a = a_jour(&l, fin + MINUTE);
        assert_eq!((a[0].pourcent, a[0].reinit), (Some(0.0), None));
        assert_eq!((a[1].pourcent, a[1].reinit), (Some(0.0), Some(fin + 7 * JOUR)));
        assert_eq!(a_jour(&l, fin)[1].reinit, Some(fin + 7 * JOUR));
        assert_eq!(a_jour(&l, fin - 1)[1], l[1]);
    }

    #[test]
    fn plus_proche_et_libelles() {
        let l = [limite("weekly", 38.0, None), limite("session", 86.0, None), Limite { pourcent: None, ..limite("weekly", 0.0, None) }];
        assert_eq!(plus_proche(&l).unwrap().groupe, "session");
        assert!(plus_proche(&[]).is_none());
        let inconnue = Limite { sorte: Some("weekly_oauth_apps".into()), ..limite("weekly", 1.0, None) };
        assert_eq!(libelle(&inconnue), "Weekly · oauth apps");
        assert_eq!(libelle(&Limite { modele: Some("Fable".into()), ..limite("weekly", 1.0, None) }), "Fable");
    }
}
