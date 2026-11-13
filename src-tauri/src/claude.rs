//! Le binaire de Claude livré avec le SDK Claude Agent : relevé du forfait, état de la connexion, connexion.
//!
//! Le relevé reprend exactement l'échange du SDK (relevé en espionnant sa version 0.3.319) : deux demandes de contrôle sur
//! l'entrée standard, au format stream-json. Un processus neuf à chaque relevé, fermé juste après : il relit la connexion du
//! trousseau à chaque fois. Gardé ouvert, il garderait des jetons périmés dès que Fil ou Claude Code renouvelle la connexion.

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const ARGS: [&str; 7] = [
    "--output-format", "stream-json", "--verbose", "--input-format", "stream-json", "--setting-sources=", "--no-session-persistence",
];
/// shortcut: `get_usage` est marqué expérimental par le SDK (Fil s'en sert aussi) ; à revoir à chaque mise à jour du binaire.
const DEMANDES: &str = concat!(
    r#"{"request_id":"jauge-init","type":"control_request","request":{"subtype":"initialize","systemPrompt":[""]}}"#, "\n",
    r#"{"request_id":"jauge-usage","type":"control_request","request":{"subtype":"get_usage","skip_behaviors":true}}"#, "\n",
);

pub struct Claude {
    pub binaire: PathBuf,
}

/// Sans les variables d'une session Claude Code qui aurait lancé Jauge : sinon le binaire se croirait dans cette session.
fn env_sans_hote() -> Vec<(String, String)> {
    let hote = ["CLAUDECODE", "CLAUDE_CODE_", "CLAUDE_AGENT_SDK_", "CLAUDE_PID", "CLAUDE_EFFORT", "CLAUDE_PREVIEW_"];
    let mut env: Vec<(String, String)> = std::env::vars().filter(|(cle, _)| !hote.iter().any(|h| cle.starts_with(h))).collect();
    env.push(("CLAUDE_CODE_ENTRYPOINT".into(), "sdk-ts".into()));
    env
}

fn attendre(enfant: &mut Child, delai: Duration) -> Option<ExitStatus> {
    let fin = Instant::now() + delai;
    loop {
        match enfant.try_wait() {
            Ok(Some(s)) => return Some(s),
            Ok(None) if Instant::now() < fin => thread::sleep(Duration::from_millis(50)),
            _ => return None,
        }
    }
}

fn arreter(mut enfant: Child) {
    if attendre(&mut enfant, Duration::from_secs(2)).is_none() {
        let _ = enfant.kill();
        let _ = enfant.wait();
    }
}

impl Claude {
    fn commande(&self, args: &[&str]) -> Command {
        let mut c = Command::new(&self.binaire);
        c.args(args)
            .env_clear()
            .envs(env_sans_hote())
            .current_dir(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        c
    }

    /// La réponse de `get_usage` : forfait et limites.
    pub fn usage(&self) -> Result<Value, String> {
        let mut enfant = self.commande(&ARGS).spawn().map_err(|e| format!("Claude ne démarre pas ({e})."))?;
        let mut entree = enfant.stdin.take().ok_or("Claude ne démarre pas.")?;
        let sortie = enfant.stdout.take().ok_or("Claude ne démarre pas.")?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for ligne in BufReader::new(sortie).lines().map_while(Result::ok) {
                if ligne.contains("\"jauge-usage\"") {
                    let _ = tx.send(ligne);
                    return;
                }
            }
        });
        let ecrit = entree.write_all(DEMANDES.as_bytes());
        let ligne = rx.recv_timeout(Duration::from_secs(20));
        // Entrée fermée : Claude s'arrête de lui-même ; sinon il est arrêté.
        drop(entree);
        arreter(enfant);
        ecrit.map_err(|_| "Claude s’est arrêté avant de répondre.")?;
        let ligne = ligne.map_err(|_| "Claude ne répond pas.")?;
        let v: Value = serde_json::from_str(&ligne).map_err(|_| "Réponse de Claude illisible.")?;
        match v["response"]["subtype"].as_str() {
            Some("success") => Ok(v["response"]["response"].clone()),
            _ => Err(v["response"]["error"].as_str().unwrap_or("Claude a refusé le relevé.").to_owned()),
        }
    }

    /// La sortie d'une commande courte, même si elle se termine en erreur (`auth status` sort en 1 quand on est déconnecté).
    fn executer(&self, args: &[&str], delai: Duration) -> Option<String> {
        let mut enfant = self.commande(args).spawn().ok()?;
        let _entree = enfant.stdin.take(); // gardée ouverte jusqu'au bout : `auth login` attend sans lire
        let mut sortie = enfant.stdout.take()?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut s = String::new();
            let _ = sortie.read_to_string(&mut s);
            let _ = tx.send(s);
        });
        if attendre(&mut enfant, delai).is_none() {
            let _ = enfant.kill();
            let _ = enfant.wait();
        }
        // Borné : un processus lancé par la commande (le navigateur de `auth login`) peut garder la sortie ouverte.
        rx.recv_timeout(Duration::from_secs(2)).ok()
    }

    /// `None` quand on ne sait pas : on ne conclut pas à une déconnexion.
    pub fn est_connecte(&self) -> Option<bool> {
        let s = self.executer(&["auth", "status", "--json"], Duration::from_secs(20))?;
        serde_json::from_str::<Value>(&s).ok()?["loggedIn"].as_bool()
    }

    /// La connexion officielle, comme Fil : elle s'ouvre dans le navigateur, aucun identifiant ne passe par Jauge.
    pub fn se_connecter(&self) {
        self.executer(&["auth", "login"], Duration::from_secs(5 * 60));
    }
}
