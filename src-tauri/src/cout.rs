//! Équivalent API : ce qu'aurait coûté au tarif de l'API la consommation inscrite dans les journaux de Claude Code
//! (~/.claude/projects). Lecture incrémentale : chaque passage ne relit que ce qui s'est ajouté aux fichiers.

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// shortcut: tarifs publics de l'API en dollars par million de jetons (entrée, sortie, lecture du cache), relevés au 06/10/2026 ;
/// à mettre à jour quand Anthropic change ses prix. Le plus précis d'abord. Écrire le cache coûte 1,25 × l'entrée (5 min) ou 2 × (1 h).
const TARIFS: [(&str, f64, f64, f64); 9] = [
    ("fable-5-1|mythos-5-1", 10.0, 50.0, 0.25),
    ("fable|mythos", 10.0, 50.0, 1.0),
    ("opus-5-5", 4.0, 20.0, 0.2),
    ("opus-4-0|opus-4-1|opus-4-2025|3-opus", 15.0, 75.0, 1.5),
    ("opus", 5.0, 25.0, 0.5),
    ("sonnet-5", 2.0, 10.0, 0.2),
    ("sonnet", 3.0, 15.0, 0.3),
    ("haiku-5", 0.1, 0.5, 0.01),
    ("haiku", 1.0, 5.0, 0.1),
];

/// Les coûts sauvés d'un lancement à l'autre ne valent que pour ces tarifs et cette lecture : changer l'un ou l'autre relit tout.
/// Même chaîne que l'ancienne version (Electron), pour reprendre sa sauvegarde. Monter le premier chiffre si la lecture change.
pub fn version() -> String {
    let t: Vec<String> = TARIFS.iter().map(|(m, e, s, l)| format!("[\"{m}\",{e},{s},{l}]")).collect();
    format!("1:[{}]", t.join(","))
}

/// Un modèle inconnu ne compte pas : mieux vaut un total incomplet qu'un montant inventé.
pub fn tarif(modele: &str) -> Option<(f64, f64, f64)> {
    TARIFS.iter().find(|(motif, ..)| motif.split('|').any(|m| modele.contains(m))).map(|&(_, e, s, l)| (e, s, l))
}

/// Coût en dollars d'une réponse, d'après le bloc `usage` du journal. Le mode rapide coûte le double.
pub fn cout_de(modele: &str, u: &Value) -> f64 {
    let Some((entree, sortie, lecture)) = tarif(modele) else { return 0.0 };
    let n = |cle: &str| u[cle].as_f64().unwrap_or(0.0);
    let une_heure = u["cache_creation"]["ephemeral_1h_input_tokens"].as_f64().unwrap_or(0.0);
    let cinq_minutes = (n("cache_creation_input_tokens") - une_heure).max(0.0);
    let dollars = (n("input_tokens") * entree + n("output_tokens") * sortie + cinq_minutes * entree * 1.25
        + une_heure * entree * 2.0 + n("cache_read_input_tokens") * lecture)
        / 1e6;
    if u["speed"] == "fast" { dollars * 2.0 } else { dollars }
}

/// Seuls ces champs sont lus : le reste d'une ligne (souvent très long) est sauté sans être construit.
#[derive(Deserialize)]
struct Ligne {
    #[serde(rename = "type")]
    sorte: Option<String>,
    timestamp: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Value>,
}

pub struct Entree {
    pub id: String,
    pub t: Option<i64>,
    pub cout: f64,
}

/// Une ligne porte une consommation si c'est une vraie réponse d'un modèle.
pub fn lire_ligne(ligne: &str) -> Option<Entree> {
    if !ligne.contains("\"usage\"") {
        return None;
    }
    let j: Ligne = serde_json::from_str(ligne).ok()?;
    let m = j.message?;
    let (id, modele, usage) = (m.id?, m.model?, m.usage?);
    if j.sorte.as_deref() != Some("assistant") || !usage.is_object() || modele.starts_with('<') {
        return None;
    }
    let t = j.timestamp.and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok()).map(|d| d.timestamp_millis());
    Some(Entree { id, t, cout: cout_de(&modele, &usage) })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bilan {
    pub total_usd: f64,
    pub aujourdhui_usd: f64,
    pub depuis: Option<i64>,
    pub messages: usize,
}

/// Le format de la sauvegarde, inchangé depuis la version Electron.
#[derive(Serialize, Deserialize)]
struct Reprise {
    version: String,
    decalages: Vec<(String, u64)>,
    messages: Vec<(String, Option<i64>, f64)>,
}

pub struct Releveur {
    racine: PathBuf,
    pub decalages: HashMap<String, u64>,
    /// Le même message est écrit plusieurs fois, parfois dans plusieurs fichiers (reprise de session) : seule sa dernière version compte.
    pub messages: HashMap<String, (Option<i64>, f64)>,
    pub change: bool,
}

fn lister(racine: &Path, sortie: &mut Vec<PathBuf>) {
    let Ok(entrees) = fs::read_dir(racine) else { return };
    for e in entrees.flatten() {
        let chemin = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => lister(&chemin, sortie),
            Ok(_) if chemin.extension().is_some_and(|x| x == "jsonl") => sortie.push(chemin),
            _ => {}
        }
    }
}

impl Releveur {
    pub fn new(racine: PathBuf) -> Self {
        Releveur { racine, decalages: HashMap::new(), messages: HashMap::new(), change: false }
    }

    /// `seulement` : les journaux qui ont bougé (chemins absolus) ; sans, tout le dossier est passé en revue.
    pub fn relever(&mut self, maintenant: i64, seulement: Option<&[PathBuf]>) -> Bilan {
        let mut tous = Vec::new();
        let chemins = match seulement {
            Some(liste) => liste,
            None => {
                lister(&self.racine, &mut tous);
                &tous[..]
            }
        };
        let mut vus = HashSet::new();
        for chemin in chemins {
            if !chemin.extension().is_some_and(|x| x == "jsonl") {
                continue;
            }
            let Ok(info) = fs::metadata(chemin) else { continue };
            let cle = chemin.to_string_lossy().into_owned();
            let taille = info.len();
            // Fichier réécrit plus court : on le relit depuis le début.
            let deja = self.decalages.get(&cle).copied().filter(|&d| d <= taille).unwrap_or(0);
            if taille > deja {
                let _ = self.lire_fichier(chemin, &cle, deja, taille);
            }
            vus.insert(cle);
        }
        // Un journal effacé par Claude Code garde ses réponses dans le total ; seule sa position de lecture est oubliée.
        if seulement.is_none() {
            let avant = self.decalages.len();
            self.decalages.retain(|c, _| vus.contains(c));
            self.change |= self.decalages.len() != avant;
        }
        self.bilan(maintenant)
    }

    fn lire_fichier(&mut self, chemin: &Path, cle: &str, depart: u64, taille: u64) -> io::Result<()> {
        let mut f = File::open(chemin)?;
        f.seek(SeekFrom::Start(depart))?;
        let mut f = f.take(taille - depart);
        let mut tampon = vec![0u8; 1 << 20];
        let mut reste: Vec<u8> = Vec::new();
        let mut pos = depart;
        loop {
            let n = f.read(&mut tampon)?;
            if n == 0 {
                break;
            }
            pos += n as u64;
            reste.extend_from_slice(&tampon[..n]);
            // Une ligne pas encore terminée attend le prochain passage.
            let Some(fin) = reste.iter().rposition(|&b| b == b'\n') else { continue };
            for ligne in reste[..fin].split(|&b| b == b'\n') {
                if let Some(e) = std::str::from_utf8(ligne).ok().and_then(lire_ligne) {
                    self.messages.insert(e.id, (e.t, e.cout));
                }
            }
            reste.drain(..=fin);
        }
        self.decalages.insert(cle.to_owned(), pos - reste.len() as u64);
        self.change = true;
        Ok(())
    }

    pub fn bilan(&self, maintenant: i64) -> Bilan {
        let minuit = Local
            .timestamp_millis_opt(maintenant)
            .single()
            .and_then(|d| d.date_naive().and_hms_opt(0, 0, 0))
            .and_then(|d| Local.from_local_datetime(&d).earliest())
            .map_or(i64::MIN, |d| d.timestamp_millis());
        let (mut total, mut aujourdhui, mut depuis) = (0.0, 0.0, None::<i64>);
        for &(t, cout) in self.messages.values() {
            total += cout;
            if let Some(t) = t {
                if t >= minuit {
                    aujourdhui += cout;
                }
                depuis = Some(depuis.map_or(t, |d| d.min(t)));
            }
        }
        Bilan { total_usd: total, aujourdhui_usd: aujourdhui, depuis, messages: self.messages.len() }
    }

    /// Reprend là où le dernier lancement s'était arrêté : sans ça, chaque démarrage du Mac relirait tous les journaux (plusieurs Go).
    /// Les positions et les réponses arrivent ensemble ou pas du tout : les unes sans les autres feraient sauter des réponses pour toujours.
    pub fn charger(&mut self, fichier: &Path) {
        let Some(r) = fs::read(fichier).ok().and_then(|o| serde_json::from_slice::<Reprise>(&o).ok()) else { return };
        if r.version == version() {
            self.decalages = r.decalages.into_iter().collect();
            self.messages = r.messages.into_iter().map(|(id, t, cout)| (id, (t, cout))).collect();
        }
    }

    pub fn sauver(&mut self, fichier: &Path) -> io::Result<()> {
        let r = Reprise {
            version: version(),
            decalages: self.decalages.iter().map(|(c, &d)| (c.clone(), d)).collect(),
            messages: self.messages.iter().map(|(id, &(t, cout))| (id.clone(), t, cout)).collect(),
        };
        let provisoire = fichier.with_extension("json.tmp");
        fs::write(&provisoire, serde_json::to_vec(&r)?)?;
        fs::rename(&provisoire, fichier)?;
        self.change = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn proche(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} ≠ {b}");
    }

    fn ligne(id: &str, sortie: f64, horodatage: &str, modele: &str) -> String {
        json!({ "type": "assistant", "timestamp": horodatage,
                "message": { "id": id, "model": modele, "usage": { "input_tokens": 0, "output_tokens": sortie } } })
        .to_string()
    }
    const MATIN: &str = "2026-11-08T10:00:00.000Z";
    const OCTOBRE: &str = "2026-10-01T10:00:00.000Z";

    #[test]
    fn version_identique_a_electron() {
        assert_eq!(
            version(),
            r#"1:[["fable-5-1|mythos-5-1",10,50,0.25],["fable|mythos",10,50,1],["opus-5-5",4,20,0.2],["opus-4-0|opus-4-1|opus-4-2025|3-opus",15,75,1.5],["opus",5,25,0.5],["sonnet-5",2,10,0.2],["sonnet",3,15,0.3],["haiku-5",0.1,0.5,0.01],["haiku",1,5,0.1]]"#
        );
    }

    #[test]
    fn tarifs_et_couts() {
        assert_eq!(tarif("claude-opus-5-5"), Some((4.0, 20.0, 0.2)));
        assert_eq!(tarif("claude-opus-4-1-20250805"), Some((15.0, 75.0, 1.5)));
        assert_eq!(tarif("claude-opus-4-7"), Some((5.0, 25.0, 0.5)));
        assert_eq!(tarif("claude-fable-5-1"), Some((10.0, 50.0, 0.25)));
        assert_eq!(tarif("claude-sonnet-4-6"), Some((3.0, 15.0, 0.3)));
        assert_eq!(tarif("gpt-5"), None);
        proche(cout_de("gpt-5", &json!({ "input_tokens": 1e6 })), 0.0);
        // Un million de jetons de chaque sorte, sur Opus 5.5 : 4 + 20 + 0,2 de lecture + 4 × 1,25 (5 min) + 4 × 2 (1 h).
        let u = json!({ "input_tokens": 1e6, "output_tokens": 1e6, "cache_read_input_tokens": 1e6, "cache_creation_input_tokens": 2e6,
                        "cache_creation": { "ephemeral_1h_input_tokens": 1e6 } });
        proche(cout_de("claude-opus-5-5", &u), 4.0 + 20.0 + 0.2 + 5.0 + 8.0);
        let mut rapide = u.clone();
        rapide["speed"] = json!("fast");
        proche(cout_de("claude-opus-5-5", &rapide), 2.0 * (4.0 + 20.0 + 0.2 + 5.0 + 8.0));
        proche(cout_de("claude-sonnet-5-5", &json!({ "cache_creation_input_tokens": 1e6 })), 2.5);
        assert!(lire_ligne(r#"{"type":"user","message":{"content":"usage"}}"#).is_none());
        assert!(lire_ligne(&ligne("m", 10.0, MATIN, "<synthetic>")).is_none());
        assert!(lire_ligne(r#"{"usage": tronqué"#).is_none());
        proche(lire_ligne(&ligne("m", 1e6, MATIN, "claude-opus-5-5")).unwrap().cout, 20.0);
    }

    #[test]
    fn lecture_incrementale_reprise_et_effacement() {
        let racine = std::env::temp_dir().join(format!("jauge-test-{}", std::process::id()));
        let dossier = racine.join("-Users-moi-projet");
        fs::create_dir_all(&dossier).unwrap();
        let fichier = dossier.join("a.jsonl");
        let maintenant = chrono::DateTime::parse_from_rfc3339("2026-11-08T12:00:00Z").unwrap().timestamp_millis();
        let opus = "claude-opus-5-5";

        // Le même message écrit deux fois : seule la dernière version compte. Une ligne pas finie attend le passage suivant.
        let complet = format!("{}\n{}\n{}", ligne("m1", 1e5, MATIN, opus), ligne("m1", 1e6, MATIN, opus), ligne("m2", 1e6, OCTOBRE, opus));
        fs::write(&fichier, &complet[..complet.len() - 20]).unwrap();
        let mut r = Releveur::new(racine.clone());
        let b = r.relever(maintenant, None);
        assert_eq!(b.messages, 1);
        proche(b.total_usd, 20.0);

        fs::write(&fichier, complet + "\n").unwrap();
        let b = r.relever(maintenant, None);
        assert_eq!(b.messages, 2);
        proche(b.total_usd, 40.0);
        proche(b.aujourdhui_usd, 20.0);
        assert_eq!(b.depuis, chrono::DateTime::parse_from_rfc3339(OCTOBRE).ok().map(|d| d.timestamp_millis()));

        // Une reprise de session recopie un message dans un autre fichier : il n'est compté qu'une fois.
        fs::write(dossier.join("b.jsonl"), ligne("m2", 1e6, OCTOBRE, opus) + "\n").unwrap();
        assert_eq!(r.relever(maintenant, None).messages, 2);

        // Reprise d'un lancement à l'autre : on repart des positions sauvées, sans rien compter deux fois ni rien perdre.
        let reprise = racine.join("reprise.json");
        r.sauver(&reprise).unwrap();
        assert!(!r.change);
        let ajouter = |id: &str| {
            use std::io::Write;
            let mut f = fs::OpenOptions::new().append(true).open(&fichier).unwrap();
            writeln!(f, "{}", ligne(id, 1e6, MATIN, opus)).unwrap();
        };
        ajouter("m3");
        let mut r2 = Releveur::new(racine.clone());
        r2.charger(&reprise);
        let b = r2.relever(maintenant, None);
        assert_eq!(b.messages, 3);
        proche(b.total_usd, 60.0);

        // Relecture ciblée (journaux signalés par la veille) : seul ce fichier est lu, les autres positions restent.
        ajouter("m4");
        assert_eq!(r2.relever(maintenant, Some(&[fichier.clone()])).messages, 4);
        assert_eq!(r2.decalages.len(), 2);

        // Un journal effacé garde ses réponses dans le total.
        fs::remove_file(dossier.join("b.jsonl")).unwrap();
        fs::remove_file(&fichier).unwrap();
        assert_eq!(r2.relever(maintenant, None).messages, 4);
        assert!(r2.decalages.is_empty());

        // Une reprise abîmée ou d'une autre version : premier passage complet, sans planter.
        let mut r3 = Releveur::new(racine.clone());
        fs::write(&reprise, r#"{"version":"0:ancien","decalages":[],"messages":[["x",null,1]]}"#).unwrap();
        r3.charger(&reprise);
        assert!(r3.messages.is_empty());
        fs::write(&reprise, "{tronqué").unwrap();
        r3.charger(&reprise);
        assert!(r3.messages.is_empty());
        fs::remove_dir_all(&racine).unwrap();
    }
}
