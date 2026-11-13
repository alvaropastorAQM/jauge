//! The Claude binary shipped with the Claude Agent SDK: plan reading, sign-in state, sign-in.
//!
//! The reading reproduces the SDK's exchange exactly (captured by spying on version 0.3.319): two control requests on
//! standard input, in stream-json format. A fresh process for every reading, closed right after: it reads the sign-in from the
//! keychain each time. Kept open, it would hold stale tokens as soon as Fil or Claude Code renews the sign-in.

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
/// shortcut: `get_usage` is marked experimental by the SDK (Fil uses it too); to be rechecked on every binary update.
const DEMANDES: &str = concat!(
    r#"{"request_id":"jauge-init","type":"control_request","request":{"subtype":"initialize","systemPrompt":[""]}}"#, "\n",
    r#"{"request_id":"jauge-usage","type":"control_request","request":{"subtype":"get_usage","skip_behaviors":true}}"#, "\n",
);

pub struct Claude {
    pub binaire: PathBuf,
}

/// Without the variables of a Claude Code session that may have launched Jauge: otherwise the binary would think it is inside that session.
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

    /// The `get_usage` response: plan and limits.
    pub fn usage(&self) -> Result<Value, String> {
        let mut enfant = self.commande(&ARGS).spawn().map_err(|e| format!("Claude does not start ({e})."))?;
        let mut entree = enfant.stdin.take().ok_or("Claude does not start.")?;
        let sortie = enfant.stdout.take().ok_or("Claude does not start.")?;
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
        // Input closed: Claude stops on its own; otherwise it is stopped.
        drop(entree);
        arreter(enfant);
        ecrit.map_err(|_| "Claude stopped before answering.")?;
        let ligne = ligne.map_err(|_| "Claude is not responding.")?;
        let v: Value = serde_json::from_str(&ligne).map_err(|_| "Claude's response could not be read.")?;
        match v["response"]["subtype"].as_str() {
            Some("success") => Ok(v["response"]["response"].clone()),
            _ => Err(v["response"]["error"].as_str().unwrap_or("Claude refused the request.").to_owned()),
        }
    }

    /// The output of a short command, even if it ends in error (`auth status` exits with 1 when signed out).
    fn executer(&self, args: &[&str], delai: Duration) -> Option<String> {
        let mut enfant = self.commande(args).spawn().ok()?;
        let _entree = enfant.stdin.take(); // kept open to the end: `auth login` waits without reading
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
        // Bounded: a process started by the command (the browser launched by `auth login`) can keep the output open.
        rx.recv_timeout(Duration::from_secs(2)).ok()
    }

    /// `None` when unknown: we do not conclude the user is signed out.
    pub fn est_connecte(&self) -> Option<bool> {
        let s = self.executer(&["auth", "status", "--json"], Duration::from_secs(20))?;
        serde_json::from_str::<Value>(&s).ok()?["loggedIn"].as_bool()
    }

    /// The official sign-in, like Fil: it opens in the browser, no credentials go through Jauge.
    pub fn se_connecter(&self) {
        self.executer(&["auth", "login"], Duration::from_secs(5 * 60));
    }
}
