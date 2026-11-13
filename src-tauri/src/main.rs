//! Jauge : tes limites Claude en direct dans la barre des menus. Pas de fenêtre, pas d'icône dans le Dock.
//!
//! Variables utiles :
//!   JAUGE_ESSAI=<dossier>   auto-test : pas d'icône de barre, une capture du panneau dans ce dossier, puis quitte.
//!   JAUGE_DEMO=1            avec JAUGE_ESSAI : données fictives, sans appeler Claude ni lire les journaux.
//!   JAUGE_THEME=light|dark  force l'apparence (captures).

mod barre;
mod claude;
mod cout;
mod limites;
mod macos;
mod rythme;

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Rect, RunEvent, State, Theme, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use cout::{Bilan, Releveur};
use limites::{fusionner, lire_limites, Limite};
use rythme::{HEURE, JOUR, MINUTE};

const LARGEUR: f64 = 340.0;
const SECONDE: Duration = Duration::from_secs(1);
const PLUS_CONNECTE: &str = "Claude Code n’est plus connecté à ton compte sur ce Mac.";

/// Ce que lit le panneau (ui/js/vue.js), champ pour champ.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Etat {
    forfait: Option<String>,
    limites: Vec<Limite>,
    releve: Option<i64>,
    erreur: Option<String>,
    en_cours: bool,
    deconnecte: bool,
    connexion: bool,
    cout: Option<Bilan>,
    usd_par_euro: Option<f64>,
}

enum Signal {
    /// Relevé demandé (panneau, menu, connexion) : tout de suite.
    Actualiser,
    /// Claude a écrit dans ses journaux : relevé si le dernier date de plus de 20 s.
    Activite,
}

struct Jauge {
    app: AppHandle,
    etat: Mutex<Etat>,
    claude: claude::Claude,
    installee: bool,
    essai: bool,
    signaux: Sender<Signal>,
    demandes_cout: Sender<Option<Vec<PathBuf>>>,
    barre: Mutex<Option<TrayIcon>>,
    dans_la_barre: Mutex<String>,
    dans_le_menu: Mutex<String>,
    dernier_essai: Mutex<Option<Instant>>,
    cout_le: Mutex<Option<Instant>>,
    taux_le: Mutex<Option<Instant>>,
    taux_essai: Mutex<Option<Instant>>,
    taux_en_cours: AtomicBool,
    ferme_le: Mutex<Option<Instant>>,
    etait_ouvert: AtomicBool,
}

/// Un fil qui a paniqué ne doit pas bloquer les autres : on reprend la donnée telle quelle.
fn verrou<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn maintenant_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn dossier_maison() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn journaux() -> PathBuf {
    std::env::var_os("JAUGE_JOURNAUX").map_or_else(|| dossier_maison().join(".claude/projects"), PathBuf::from)
}

/// Le même dossier que la version Electron : la reprise de l'équivalent API et la marque du premier lancement y sont déjà.
fn dossier_donnees() -> PathBuf {
    dossier_maison().join("Library/Application Support/Jauge")
}

/// Taux du jour de la Banque centrale européenne (dollars pour un euro), lu dans son fichier XML.
fn taux_usd(xml: &str) -> Option<f64> {
    let debut = xml.find("currency='USD'").or_else(|| xml.find("currency=\"USD\""))?;
    let reste = &xml[debut..];
    let valeur = reste.get(reste.find("rate=")? + 6..)?;
    valeur[..valeur.find(['\'', '"'])?].parse().ok()
}

impl Jauge {
    fn signal(&self, s: Signal) {
        let _ = self.signaux.send(s);
    }

    fn panneau(&self) -> Option<WebviewWindow> {
        self.app.get_webview_window("panneau")
    }

    fn diffuser(&self) {
        let etat = verrou(&self.etat).clone();
        self.maj_barre(&etat);
        let _ = self.app.emit_to("panneau", "etat", &etat);
    }

    /// La barre suit la session de 5 h ; la limite la plus proche seulement si le forfait n'en a pas.
    fn maj_barre(&self, etat: &Etat) {
        let Some(barre) = verrou(&self.barre).clone() else { return };
        let t = maintenant_ms();
        let limites = rythme::a_jour(&etat.limites, t);
        let p = limites.iter().find(|l| l.groupe == "session").or_else(|| rythme::plus_proche(&limites));
        let pct = p.and_then(|l| l.pourcent).map(|x| x.round() as i64);
        // Sans relevé réussi depuis 10 min (hors ligne), l'anneau passe en gris : le chiffre n'est plus sûr.
        let frais = etat.releve.is_some_and(|r| t - r < 10 * MINUTE);
        let ton = p.filter(|_| frais).map(|l| rythme::ton(l, t));
        // Même chiffre, même couleur : rien à redessiner dans la barre.
        let cle = format!("{pct:?}:{ton:?}");
        {
            let mut avant = verrou(&self.dans_la_barre);
            if *avant == cle {
                return;
            }
            *avant = cle;
        }
        let (pixels, modele) = barre::anneau(pct.map(|x| x as f64), ton);
        let _ = barre.set_icon_with_as_template(Some(Image::new_owned(pixels, barre::COTE, barre::COTE)), modele);
        let _ = barre.set_title(pct.map(|x| format!(" {x}\u{202f}%")));
        let _ = barre.set_tooltip(Some(match (p, pct) {
            (Some(l), Some(x)) => format!("{} : {x}\u{202f}%", rythme::libelle(l)),
            _ => "Jauge".into(),
        }));
    }

    fn maj_menu(&self) {
        let Some(barre) = verrou(&self.barre).clone() else { return };
        let (deconnecte, connexion) = {
            let e = verrou(&self.etat);
            (e.deconnecte, e.connexion)
        };
        let demarrage = self.installee && macos::ouvre_au_demarrage();
        let cle = format!("{deconnecte}:{connexion}:{demarrage}");
        {
            let mut avant = verrou(&self.dans_le_menu);
            if *avant == cle {
                return;
            }
            *avant = cle;
        }
        let app = &self.app;
        let menu = (|| -> tauri::Result<Menu<tauri::Wry>> {
            let menu = Menu::new(app)?;
            if deconnecte {
                menu.append(&MenuItem::with_id(app, "connecter", "Se connecter à Claude…", !connexion, None::<&str>)?)?;
            }
            menu.append(&MenuItem::with_id(app, "actualiser", "Actualiser", true, None::<&str>)?)?;
            menu.append(&CheckMenuItem::with_id(app, "demarrage", "Ouvrir au démarrage du Mac", self.installee, demarrage, None::<&str>)?)?;
            menu.append(&PredefinedMenuItem::separator(app)?)?;
            menu.append(&MenuItem::with_id(app, "quitter", "Quitter Jauge", true, None::<&str>)?)?;
            Ok(menu)
        })();
        if let Ok(menu) = menu {
            let _ = barre.set_menu(Some(menu));
        }
    }

    /* ------------------------------ Relevés ------------------------------ */

    /// Un relevé complet : forfait (et, au besoin, coût et taux de change). Vrai s'il a réussi.
    fn actualiser(self: &Arc<Self>) -> bool {
        *verrou(&self.dernier_essai) = Some(Instant::now());
        verrou(&self.etat).en_cours = true;
        self.diffuser();
        // Les journaux qui bougent sont relus dès que Claude y écrit (veiller) ; ici tout le dossier toutes les 10 min,
        // au cas où la veille en raterait un, et pour passer minuit.
        if verrou(&self.cout_le).is_none_or(|c| c.elapsed() > 10 * 60 * SECONDE) {
            *verrou(&self.cout_le) = Some(Instant::now());
            let _ = self.demandes_cout.send(None);
        }
        self.relever_taux();
        let resultat = self.relever_forfait();
        {
            let mut e = verrou(&self.etat);
            e.en_cours = false;
            if let Err(message) = &resultat {
                e.erreur = Some(message.clone());
                // Des limites d'un compte déconnecté ne sont plus les tiennes : elles disparaissent.
                if e.deconnecte {
                    e.limites.clear();
                }
            }
        }
        self.diffuser();
        self.maj_menu();
        resultat.is_ok()
    }

    fn relever_forfait(&self) -> Result<(), String> {
        // Le verrou est relâché avant d'interroger Claude : le panneau et la barre le lisent pendant ce temps.
        let deja_deconnecte = verrou(&self.etat).deconnecte;
        if deja_deconnecte && self.claude.est_connecte() == Some(false) {
            return Err(PLUS_CONNECTE.into());
        }
        // Une erreur ou une réponse sans limites, c'est souvent une connexion perdue : on vérifie avant de le dire.
        let perdue = |message: String| {
            let deconnecte = self.claude.est_connecte() == Some(false);
            verrou(&self.etat).deconnecte = deconnecte;
            if deconnecte { PLUS_CONNECTE.to_owned() } else { message }
        };
        let r = self.claude.usage().map_err(perdue)?;
        let rl = &r["rate_limits"];
        if r["rate_limits_available"].as_bool() != Some(true) || !rl.is_object() {
            return Err(perdue("Claude n’a pas renvoyé tes limites.".into()));
        }
        let t = maintenant_ms();
        let mut e = verrou(&self.etat);
        e.deconnecte = false;
        e.forfait = r["subscription_type"].as_str().map(str::to_owned);
        e.limites = fusionner(lire_limites(rl), &e.limites, t);
        e.releve = Some(t);
        e.erreur = None;
        Ok(())
    }

    /// Le taux du jour, pour afficher l'équivalent API en euros (sinon il reste en dollars). À part : rien ne l'attend.
    fn relever_taux(self: &Arc<Self>) {
        if verrou(&self.taux_le).is_some_and(|t| t.elapsed() < 12 * 3600 * SECONDE)
            || verrou(&self.taux_essai).is_some_and(|t| t.elapsed() < 600 * SECONDE)
            || self.taux_en_cours.swap(true, Ordering::SeqCst)
        {
            return;
        }
        *verrou(&self.taux_essai) = Some(Instant::now());
        let j = self.clone();
        thread::spawn(move || {
            // Borné à 15 s : une connexion à moitié ouverte (réveil, portail Wi-Fi) ne doit rien bloquer.
            let xml = Command::new("/usr/bin/curl")
                .args(["-sfL", "--max-time", "15", "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
            if let Some(taux) = xml.as_deref().and_then(taux_usd) {
                verrou(&j.etat).usd_par_euro = Some(taux);
                *verrou(&j.taux_le) = Some(Instant::now());
                j.diffuser();
            }
            j.taux_en_cours.store(false, Ordering::SeqCst);
        });
    }

    fn se_connecter(self: &Arc<Self>) {
        {
            let mut e = verrou(&self.etat);
            if e.connexion {
                return;
            }
            e.connexion = true;
        }
        self.diffuser();
        self.maj_menu();
        let j = self.clone();
        thread::spawn(move || {
            j.claude.se_connecter();
            verrou(&j.etat).connexion = false;
            j.maj_menu();
            j.signal(Signal::Actualiser);
        });
    }

    /* ------------------------------ Panneau ------------------------------ */

    fn ouvrir(&self, rect: Option<Rect>) {
        let Some(f) = self.panneau() else { return };
        let barre = verrou(&self.barre).clone();
        let rect = rect.or_else(|| barre.and_then(|b| b.rect().ok().flatten()));
        if let Some(r) = rect {
            placer(&f, r);
        }
        // Comptes à rebours et repères remis à l'heure avant l'affichage : un panneau caché ne se redessine presque plus.
        self.diffuser();
        let _ = f.show();
        let _ = f.set_focus();
        if verrou(&self.dernier_essai).is_none_or(|d| d.elapsed() > 15 * SECONDE) {
            self.signal(Signal::Actualiser);
        }
    }

    fn basculer(&self) {
        match self.panneau() {
            Some(f) if f.is_visible().unwrap_or(false) => {
                let _ = f.hide();
            }
            _ => self.ouvrir(None),
        }
    }
}

/// Sous l'icône, centré, sans sortir de l'écran.
fn placer(f: &WebviewWindow, r: Rect) {
    let echelle = f.scale_factor().unwrap_or(2.0);
    let pos = r.position.to_physical::<f64>(echelle);
    let taille = r.size.to_physical::<f64>(echelle);
    let largeur = LARGEUR * echelle;
    let mut x = pos.x + taille.width / 2.0 - largeur / 2.0;
    if let Some(m) = f.available_monitors().unwrap_or_default().into_iter().find(|m| {
        let (mp, ms) = (m.position(), m.size());
        pos.x >= mp.x as f64 && pos.x < (mp.x + ms.width as i32) as f64
    }) {
        let (gauche, droite) = (m.position().x as f64 + 8.0 * echelle, (m.position().x + m.size().width as i32) as f64 - largeur - 8.0 * echelle);
        x = x.min(droite).max(gauche);
    }
    let _ = f.set_position(PhysicalPosition::new(x.round() as i32, (pos.y + taille.height + 4.0 * echelle).round() as i32));
}

/* ------------------------------ Fils de fond ------------------------------ */

/// Un fil de fond qui panique (bug imprévu) repart 5 s plus tard : la jauge ne reste jamais figée sans rien dire.
fn relancer_si_panique(mut corps: impl FnMut() + Send + 'static) {
    thread::spawn(move || loop {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut corps)).is_ok() {
            return;
        }
        thread::sleep(5 * SECONDE);
    });
}

/// Le rythme des relevés : chaque minute ; après un échec, toutes les 30 s juste après le lancement ou un réveil (réseau pas
/// encore prêt) et tant que la connexion manque, sinon toutes les 3 min. Chaque relevé lance un processus Claude (~0,7 s de
/// calcul) : Mac verrouillé ou sans toi depuis 5 min, un toutes les 5 min suffit.
fn planificateur(j: Arc<Jauge>, rx: &Receiver<Signal>) {
    let mut prochain = Instant::now();
    let mut vite_jusqua = Instant::now() + 600 * SECONDE;
    let (mut mur, mut mono) = (SystemTime::now(), Instant::now());
    let mut absent_avant = false;
    loop {
        let attente = prochain.saturating_duration_since(Instant::now()).min(5 * SECONDE);
        let signal = match rx.recv_timeout(attente) {
            Ok(s) => Some(s),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // Réveil du Mac : l'horloge murale a avancé pendant la veille, l'horloge monotone non.
        let (mur2, mono2) = (SystemTime::now(), Instant::now());
        let reveil = mur2.duration_since(mur).unwrap_or_default().saturating_sub(mono2 - mono) > 30 * SECONDE;
        (mur, mono) = (mur2, mono2);
        if reveil {
            vite_jusqua = mono2 + 300 * SECONDE;
            // Ces délais ne comptent pas la veille : la nuit passée, tout est à relire (et « aujourd'hui » a changé).
            *verrou(&j.cout_le) = None;
            *verrou(&j.taux_le) = None;
            *verrou(&j.taux_essai) = None;
        }
        // Retour devant le Mac après 5 min d'absence (déverrouillage compris) : relevé tout de suite.
        let inactif = macos::inactif_depuis();
        let absent = inactif >= 300.0;
        let retour = absent_avant && inactif < 10.0;
        absent_avant = absent;
        let depuis = verrou(&j.dernier_essai).map(|d| d.elapsed());
        let relever = match signal {
            Some(Signal::Actualiser) => true,
            Some(Signal::Activite) => depuis.is_none_or(|d| d > if absent { 300 * SECONDE } else { 20 * SECONDE }),
            None => reveil || retour || (Instant::now() >= prochain && !(absent && depuis.is_some_and(|d| d < 300 * SECONDE))),
        };
        if !relever {
            if Instant::now() >= prochain {
                prochain = Instant::now() + 60 * SECONDE;
            }
            continue;
        }
        let reussi = j.actualiser();
        // Les demandes arrivées pendant le relevé sont servies par lui.
        while rx.try_recv().is_ok() {}
        let vite = verrou(&j.etat).deconnecte || Instant::now() < vite_jusqua;
        prochain = Instant::now() + if reussi { 60 * SECONDE } else if vite { 30 * SECONDE } else { 180 * SECONDE };
    }
}

/// L'équivalent API, dans un fil à part : le premier passage peut lire des Go de journaux, la barre n'a pas à attendre.
/// Les demandes arrivées pendant un passage sont regroupées dans le suivant.
fn fil_cout(j: Arc<Jauge>, rx: &Receiver<Option<Vec<PathBuf>>>, reprise: Option<PathBuf>) {
    let mut releveur = Releveur::new(journaux());
    if let Some(p) = &reprise {
        releveur.charger(p);
    }
    let mut sauve_le: Option<Instant> = None;
    while let Ok(premiere) = rx.recv() {
        let mut tout = premiere.is_none();
        let mut fichiers: HashSet<PathBuf> = premiere.into_iter().flatten().collect();
        while let Ok(d) = rx.try_recv() {
            match d {
                None => tout = true,
                Some(l) => fichiers.extend(l),
            }
        }
        let liste: Vec<PathBuf> = fichiers.into_iter().collect();
        let bilan = releveur.relever(maintenant_ms(), if tout { None } else { Some(&liste) });
        verrou(&j.etat).cout = Some(bilan);
        j.diffuser();
        // Sauvé après le premier passage, puis toutes les 10 min au plus : au pire, le prochain lancement relit ces 10 min.
        if let Some(p) = &reprise {
            if releveur.change && sauve_le.is_none_or(|s| s.elapsed() > 600 * SECONDE) {
                if let Some(d) = p.parent() {
                    let _ = fs::create_dir_all(d);
                }
                let _ = releveur.sauver(p);
                sauve_le = Some(Instant::now());
            }
        }
    }
}

/// Claude écrit dans ses journaux à chaque réponse : la jauge suit sans attendre la minute (limites au plus toutes les 20 s),
/// et l'équivalent API ne relit que les journaux qui ont bougé.
fn veiller(j: Arc<Jauge>) {
    thread::spawn(move || loop {
        let (tx, rx) = mpsc::channel::<Option<PathBuf>>();
        let veille = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| match ev {
            Ok(ev) => {
                for p in ev.paths.into_iter().filter(|p| p.extension().is_some_and(|x| x == "jsonl")) {
                    let _ = tx.send(Some(p));
                }
            }
            Err(_) => {
                let _ = tx.send(None);
            }
        })
        .and_then(|mut w| w.watch(&journaux(), RecursiveMode::Recursive).map(|_| w));
        if let Ok(_veille) = veille {
            while let Ok(Some(premier)) = rx.recv() {
                let mut bouges = HashSet::from([premier]);
                thread::sleep(3 * SECONDE);
                let mut panne = false;
                while let Ok(m) = rx.try_recv() {
                    match m {
                        Some(p) => {
                            bouges.insert(p);
                        }
                        None => panne = true,
                    }
                }
                let _ = j.demandes_cout.send(Some(bouges.into_iter().collect()));
                j.signal(Signal::Activite);
                if panne {
                    break;
                }
            }
        }
        // Pas encore de dossier des projets (Claude Code le créera) ou veille en panne : nouvel essai dans une minute.
        thread::sleep(60 * SECONDE);
    });
}

/* ------------------------------ Échanges avec le panneau ------------------------------ */

#[tauri::command]
fn etat(j: State<'_, Arc<Jauge>>) -> Etat {
    verrou(&j.etat).clone()
}

#[tauri::command]
fn actualiser(j: State<'_, Arc<Jauge>>) {
    j.signal(Signal::Actualiser);
}

#[tauri::command]
fn connecter(j: State<'_, Arc<Jauge>>) {
    j.se_connecter();
}

/// Le panneau prend la hauteur de son contenu.
#[tauri::command]
fn hauteur(h: f64, fenetre: WebviewWindow) {
    if h.is_finite() {
        let _ = fenetre.set_size(LogicalSize::new(LARGEUR, h.ceil().clamp(120.0, 700.0)));
    }
}

/* ------------------------------ Auto-test ------------------------------ */

fn donnees_demo(j: &Jauge) {
    let t = maintenant_ms();
    let l = |groupe: &str, sorte: &str, pourcent: f64, reinit: i64, modele: Option<&str>| Limite {
        groupe: groupe.into(), sorte: Some(sorte.into()), pourcent: Some(pourcent), gravite: Some("normal".into()),
        reinit: Some(reinit), modele: modele.map(Into::into), surface: None, vu: Some(t),
    };
    *verrou(&j.etat) = Etat {
        forfait: Some("max".into()),
        releve: Some(t),
        usd_par_euro: Some(1.08),
        limites: vec![
            l("session", "session", 86.0, t + 2 * HEURE, None),
            l("weekly", "weekly_all", 38.0, t + 42 * JOUR / 10, None),
            l("weekly", "weekly_scoped", 12.0, t + 42 * JOUR / 10, Some("Fable")),
        ],
        cout: Some(Bilan { total_usd: 23766.0, aujourdhui_usd: 41.2, depuis: Some(t - 64 * JOUR), messages: 182_000 }),
        ..Default::default()
    };
}

/// Affiche le panneau, attend les vrais chiffres (ou la démo), le capture, écrit l'état sur la sortie et quitte.
fn essai(j: Arc<Jauge>, dossier: PathBuf, demo: bool, theme: Option<Theme>) {
    thread::spawn(move || {
        if demo {
            donnees_demo(&j);
        } else {
            let attendre = |pret: &dyn Fn(&Etat) -> bool, secondes: u32| {
                for _ in 0..secondes * 2 {
                    if pret(&verrou(&j.etat)) {
                        break;
                    }
                    thread::sleep(SECONDE / 2);
                }
            };
            attendre(&|e| e.releve.is_some() || e.erreur.is_some(), 60);
            // Le premier passage dans les journaux peut être long.
            attendre(&|e| e.cout.is_some(), 120);
        }
        if let Some(f) = j.panneau() {
            let _ = f.show();
        }
        j.diffuser();
        thread::sleep(2 * SECONDE);
        let _ = fs::create_dir_all(&dossier);
        if let Some(f) = j.panneau() {
            let echelle = f.scale_factor().unwrap_or(2.0);
            let (p, t) = (f.outer_position().unwrap_or_default(), f.outer_size().unwrap_or_default());
            let nom = if theme == Some(Theme::Light) { "jauge-clair.png" } else { "jauge-sombre.png" };
            let zone = [p.x as f64, p.y as f64, t.width as f64, t.height as f64].map(|v| (v / echelle).round().to_string()).join(",");
            let _ = Command::new("screencapture").args(["-x", &format!("-R{zone}")]).arg(dossier.join(nom)).status();
        }
        let e = verrou(&j.etat).clone();
        println!(
            "{}",
            serde_json::json!({ "forfait": e.forfait, "limites": e.limites, "erreur": e.erreur, "cout": e.cout, "usdParEuro": e.usd_par_euro })
        );
        j.app.exit(if !demo && e.erreur.is_some() { 1 } else { 0 });
    });
}

/* ------------------------------ Démarrage ------------------------------ */

fn creer_panneau(app: &AppHandle, essai: bool, theme: Option<Theme>) -> tauri::Result<WebviewWindow> {
    let mut b = WebviewWindowBuilder::new(app, "panneau", WebviewUrl::App("index.html".into()))
        .title("Jauge")
        .inner_size(LARGEUR, 300.0)
        .visible(false)
        .focused(false)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .theme(theme);
    b = if essai {
        b.initialization_script("window.JAUGE_ESSAI = true;").position(40.0, 60.0)
    } else {
        // Le verre de macOS (comme les panneaux des icônes système), aux coins arrondis.
        b.transparent(true).shadow(true).effects(EffectsBuilder::new().effect(Effect::Popover).state(EffectState::Active).radius(12.0).build())
    };
    b.build()
}

fn main() {
    let essai_dans = std::env::var_os("JAUGE_ESSAI").map(PathBuf::from);
    let demo = essai_dans.is_some() && std::env::var_os("JAUGE_DEMO").is_some();
    let theme = match std::env::var("JAUGE_THEME").as_deref() {
        Ok("light") => Some(Theme::Light),
        Ok("dark") => Some(Theme::Dark),
        _ => None,
    };
    let mut builder = tauri::Builder::default();
    if essai_dans.is_none() {
        // Une seule Jauge : une seconde ouverture montre le panneau de la première, puis s'arrête.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _dossier| {
            if let Some(j) = app.try_state::<Arc<Jauge>>() {
                j.basculer();
            }
        }));
    }
    let app = builder
        .invoke_handler(tauri::generate_handler![etat, actualiser, connecter, hauteur])
        .setup(move |app| {
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let exe = std::env::current_exe()?;
            let installee = exe.to_string_lossy().contains(".app/Contents/MacOS/");
            let binaire = if installee {
                exe.parent().and_then(|p| p.parent()).map(|c| c.join("Resources/claude")).unwrap_or_default()
            } else {
                PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"))
            };
            let (signaux, rx_signaux) = mpsc::channel();
            let (demandes_cout, rx_cout) = mpsc::channel();
            let j = Arc::new(Jauge {
                app: app.handle().clone(),
                etat: Mutex::new(Etat::default()),
                claude: claude::Claude { binaire },
                installee,
                essai: essai_dans.is_some(),
                signaux,
                demandes_cout,
                barre: Mutex::new(None),
                dans_la_barre: Mutex::new(String::new()),
                dans_le_menu: Mutex::new(String::new()),
                dernier_essai: Mutex::new(None),
                cout_le: Mutex::new(None),
                taux_le: Mutex::new(None),
                taux_essai: Mutex::new(None),
                taux_en_cours: AtomicBool::new(false),
                ferme_le: Mutex::new(None),
                etait_ouvert: AtomicBool::new(false),
            });
            app.manage(j.clone());

            let fenetre = creer_panneau(app.handle(), j.essai, theme)?;
            if let Ok(ns) = fenetre.ns_window() {
                macos::regler_panneau(ns);
            }
            let jf = j.clone();
            fenetre.on_window_event(move |ev| {
                if let WindowEvent::Focused(false) = ev {
                    if !jf.essai {
                        if let Some(f) = jf.panneau() {
                            let _ = f.hide();
                        }
                        *verrou(&jf.ferme_le) = Some(Instant::now());
                    }
                }
            });

            if let Some(dossier) = essai_dans.clone() {
                if !demo {
                    let jp = j.clone();
                    relancer_si_panique(move || planificateur(jp.clone(), &rx_signaux));
                    let jc = j.clone();
                    relancer_si_panique(move || fil_cout(jc.clone(), &rx_cout, None));
                }
                essai(j, dossier, demo, theme);
                return Ok(());
            }

            let (pixels, _) = barre::anneau(None, None);
            let barre = TrayIconBuilder::with_id("jauge")
                .icon(Image::new_owned(pixels, barre::COTE, barre::COTE))
                .icon_as_template(true)
                .tooltip("Jauge")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|barre, ev| {
                    let TrayIconEvent::Click { button: MouseButton::Left, button_state, rect, .. } = ev else { return };
                    let Some(j) = barre.app_handle().try_state::<Arc<Jauge>>() else { return };
                    // Un clic sur l'icône quand le panneau est ouvert le ferme d'abord (perte du focus) : il ne doit pas
                    // se rouvrir au relâchement.
                    match button_state {
                        MouseButtonState::Down => {
                            let ouvert = j.panneau().is_some_and(|f| f.is_visible().unwrap_or(false))
                                || verrou(&j.ferme_le).is_some_and(|f| f.elapsed() < Duration::from_millis(400));
                            j.etait_ouvert.store(ouvert, Ordering::SeqCst);
                        }
                        MouseButtonState::Up => {
                            if j.etait_ouvert.load(Ordering::SeqCst) {
                                if let Some(f) = j.panneau() {
                                    let _ = f.hide();
                                }
                            } else {
                                j.ouvrir(Some(rect));
                            }
                        }
                    }
                })
                .on_menu_event(|app, ev| {
                    let Some(j) = app.try_state::<Arc<Jauge>>() else { return };
                    match ev.id().as_ref() {
                        "actualiser" => j.signal(Signal::Actualiser),
                        "connecter" => j.se_connecter(),
                        "demarrage" => {
                            let _ = macos::ouvrir_au_demarrage(!macos::ouvre_au_demarrage());
                            // Le menu est refait même si rien n'a changé (échec) : la coche suit l'état réel.
                            verrou(&j.dans_le_menu).clear();
                            j.maj_menu();
                        }
                        "quitter" => app.exit(0),
                        _ => {}
                    }
                })
                .build(app)?;
            let _ = barre.with_inner_tray_icon(|b| {
                if let Some(item) = b.ns_status_item() {
                    macos::regler_icone(&item);
                }
            });
            *verrou(&j.barre) = Some(barre);
            j.maj_menu();
            j.diffuser();

            // Premier lancement de l'app installée : elle s'ouvrira avec le Mac. Décocher dans le menu reste ton choix.
            let donnees = dossier_donnees();
            let marque = donnees.join("premier-lancement-fait");
            if installee && !marque.exists() {
                let _ = macos::ouvrir_au_demarrage(true);
                let _ = fs::create_dir_all(&donnees);
                let _ = fs::write(&marque, "");
                j.maj_menu();
            }

            let jp = j.clone();
            relancer_si_panique(move || planificateur(jp.clone(), &rx_signaux));
            let jc = j.clone();
            let reprise = donnees.join("equivalent-api.json");
            relancer_si_panique(move || fil_cout(jc.clone(), &rx_cout, Some(reprise.clone())));
            veiller(j);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Jauge ne démarre pas");
    // Jauge vit dans la barre des menus : cacher le panneau ne la quitte pas, seul « Quitter Jauge » le fait.
    app.run(|_app, ev| {
        if let RunEvent::ExitRequested { api, code: None, .. } = ev {
            api.prevent_exit();
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn taux_de_la_bce() {
        let xml = "<Cube time='2026-11-11'><Cube currency='USD' rate='1.1193'/><Cube currency='JPY' rate='170.2'/></Cube>";
        assert_eq!(super::taux_usd(xml), Some(1.1193));
        assert_eq!(super::taux_usd(r#"<Cube currency="USD" rate="1.05"/>"#), Some(1.05));
        assert_eq!(super::taux_usd("<rien/>"), None);
        // Fichier coupé juste après « rate= » : pas de panique.
        assert_eq!(super::taux_usd("<Cube currency='USD' rate="), None);
    }
}
