//! Les limites du forfait, lues dans la réponse de Claude, et leur suivi d'un relevé à l'autre.

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::rythme::MINUTE;

/// Même forme que celle que lit le panneau (ui/js/vue.js).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Limite {
    pub groupe: String,
    #[serde(rename = "type")]
    pub sorte: Option<String>,
    pub pourcent: Option<f64>,
    pub gravite: Option<String>,
    /// Remise à zéro, en millisecondes depuis 1970.
    pub reinit: Option<i64>,
    pub modele: Option<String>,
    pub surface: Option<String>,
    /// Dernier relevé qui l'a renvoyée.
    pub vu: Option<i64>,
}

pub fn date_ms(v: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|d| d.timestamp_millis())
}

fn texte(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn vers_limite(l: &Value) -> Limite {
    let surface = &l["scope"]["surface"];
    Limite {
        groupe: texte(&l["group"]).unwrap_or_default(),
        sorte: texte(&l["kind"]),
        pourcent: l["percent"].as_f64(),
        gravite: texte(&l["severity"]),
        reinit: date_ms(&l["resets_at"]),
        modele: texte(&l["scope"]["model"]["display_name"]),
        surface: texte(surface).or_else(|| texte(&surface["display_name"])),
        vu: None,
    }
}

/// La liste `limits` quand Claude l'envoie, sinon l'ancienne forme (five_hour, seven_day, model_scoped), traduite dans les mêmes
/// sortes : une limite garde ainsi la même clé quelle que soit la forme de la réponse.
pub fn lire_limites(rl: &Value) -> Vec<Limite> {
    let limites: Vec<Limite> = rl["limits"].as_array().map(|a| a.iter().map(vers_limite).collect()).unwrap_or_default();
    if !limites.is_empty() {
        return limites;
    }
    let mut anciennes = Vec::new();
    for (groupe, sorte, fenetre) in [("session", "session", &rl["five_hour"]), ("weekly", "weekly_all", &rl["seven_day"])] {
        if fenetre.is_object() {
            anciennes.push(vers_limite(&json!({
                "group": groupe, "kind": sorte, "percent": fenetre["utilization"], "resets_at": fenetre["resets_at"],
            })));
        }
    }
    for m in rl["model_scoped"].as_array().into_iter().flatten() {
        anciennes.push(vers_limite(&json!({
            "group": "weekly", "kind": "weekly_scoped", "percent": m["utilization"], "resets_at": m["resets_at"],
            "scope": { "model": { "display_name": m["display_name"] } },
        })));
    }
    anciennes
}

fn cle(l: &Limite) -> (Option<String>, Option<String>, Option<String>) {
    (l.sorte.clone(), l.modele.clone(), l.surface.clone())
}

pub const GRACE: i64 = 10 * MINUTE;

/// Une réponse incomplète (juste après une reconnexion, par exemple) peut omettre une limite, comme Fable : sa dernière valeur
/// reste affichée 10 min au plus, et jamais au-delà de sa remise à zéro.
pub fn fusionner(nouvelles: Vec<Limite>, anciennes: &[Limite], t: i64) -> Vec<Limite> {
    let presentes: HashSet<_> = nouvelles.iter().map(cle).collect();
    let mut sortie: Vec<Limite> = nouvelles.into_iter().map(|l| Limite { vu: Some(t), ..l }).collect();
    sortie.extend(
        anciennes
            .iter()
            .filter(|l| !presentes.contains(&cle(l)) && l.reinit.is_some_and(|r| r > t) && l.vu.is_some_and(|v| t - v < GRACE))
            .cloned(),
    );
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rythme::{libelle, JOUR};

    const QUAND: &str = "2026-11-10T06:00:00.413276+00:00";

    fn neuve() -> Value {
        json!({ "limits": [
            { "kind": "session", "group": "session", "percent": 36, "resets_at": "2026-11-09T19:40:00+00:00", "severity": "normal", "scope": null },
            { "kind": "weekly_all", "group": "weekly", "percent": 74, "resets_at": QUAND, "severity": "normal", "scope": null },
            { "kind": "weekly_scoped", "group": "weekly", "percent": 15, "resets_at": QUAND, "severity": "normal",
              "scope": { "model": { "display_name": "Fable", "id": null }, "surface": null } },
        ] })
    }

    fn ancienne() -> Value {
        json!({ "limits": [], "five_hour": { "utilization": 35, "resets_at": "2026-11-09T19:40:00+00:00" },
                "seven_day": { "utilization": 72, "resets_at": QUAND },
                "model_scoped": [{ "display_name": "Fable", "utilization": 14, "resets_at": QUAND }] })
    }

    #[test]
    fn deux_formes_memes_cles() {
        let (n, a) = (lire_limites(&neuve()), lire_limites(&ancienne()));
        assert_eq!(n.iter().map(libelle).collect::<Vec<_>>(), ["Session", "Semaine", "Fable"]);
        assert_eq!(n.iter().map(cle).collect::<Vec<_>>(), a.iter().map(cle).collect::<Vec<_>>());
        assert_eq!(n[1].reinit, Some(1_794_290_400_413));
    }

    #[test]
    fn fusion_sans_doublon_et_grace() {
        let t = date_ms(&json!("2026-11-09T17:30:00Z")).unwrap();
        let (n, a) = (lire_limites(&neuve()), lire_limites(&ancienne()));
        // Le cas de la capture : un relevé à l'ancienne forme, puis un à la nouvelle. Trois cartes, pas quatre.
        let mut suivies = fusionner(a, &[], t);
        suivies = fusionner(n.clone(), &suivies, t + MINUTE);
        assert_eq!(suivies.iter().map(|l| l.pourcent.unwrap()).collect::<Vec<_>>(), [36.0, 74.0, 15.0]);
        // Fable absent d'une réponse : gardé 10 min au plus, puis oublié.
        suivies = fusionner(n[..2].to_vec(), &suivies, t + 2 * MINUTE);
        assert_eq!(suivies.len(), 3);
        assert_eq!(fusionner(n[..2].to_vec(), &suivies, t + MINUTE + GRACE).len(), 2);
        // Ni au-delà de sa remise à zéro.
        assert!(fusionner(vec![], &suivies, date_ms(&json!(QUAND)).unwrap() + 1).is_empty());
        assert!(JOUR > 0);
    }

    #[test]
    fn surface_en_texte_ou_objet() {
        let l = vers_limite(&json!({ "group": "weekly", "kind": "weekly_scoped", "scope": { "surface": { "display_name": "Claude Code" } } }));
        assert_eq!(l.surface.as_deref(), Some("Claude Code"));
        let l = vers_limite(&json!({ "group": "weekly", "kind": "weekly_scoped", "scope": { "surface": "Cowork" } }));
        assert_eq!(l.surface.as_deref(), Some("Cowork"));
    }
}
