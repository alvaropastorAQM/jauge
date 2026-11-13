//! Ce que Tauri ne fait pas lui-même sur macOS : place et police de l'icône, inactivité, ouverture au démarrage.

use objc2::runtime::AnyObject;
use objc2::{msg_send, MainThreadMarker};
use objc2_app_kit::{NSFont, NSFontWeightRegular, NSStatusItem, NSWindow, NSWindowCollectionBehavior};
use objc2_foundation::{NSString, NSUserDefaults};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Nom fixe de l'icône, le même que la version Electron : macOS retient sa place sous ce nom d'un lancement à l'autre.
const NOM_BARRE: &str = "b3f1e7a2-5c4d-4e8f-9a6b-2d7c1e0f8a35";
/// Distance au bord droit de l'écran, en points : à côté de la batterie (285), du côté visible de l'encoche.
/// Posée une seule fois : si tu déplaces l'icône avec ⌘, macOS réécrit cette valeur et ta place est gardée.
const PLACE_BARRE: f64 = 300.0;

/// À appeler sur le fil principal, juste après la création de l'icône.
pub fn regler_icone(item: &NSStatusItem) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let nom = NSString::from_str(NOM_BARRE);
    let cle = NSString::from_str(&format!("NSStatusItem Preferred Position {NOM_BARRE}"));
    let reglages = NSUserDefaults::standardUserDefaults();
    if reglages.doubleForKey(&cle) == 0.0 {
        reglages.setDouble_forKey(PLACE_BARRE, &cle);
    }
    item.setAutosaveName(Some(&nom));
    // Chiffres à chasse fixe, comme les icônes système : le pourcentage ne fait pas bouger les icônes voisines.
    if let Some(bouton) = item.button(mtm) {
        let police = unsafe { NSFont::monospacedDigitSystemFontOfSize_weight(NSFont::systemFontSize(), NSFontWeightRegular) };
        bouton.setFont(Some(&police));
    }
}

/// Ce que Tauri ne règle pas pour le panneau, à appeler sur le fil principal :
/// - il s'affiche aussi par-dessus une app en plein écran, comme la version Electron (Tauri ne règle que « tous les bureaux ») ;
/// - ses coins sont arrondis à 12 points en coupant toute la fenêtre (verre et page) : l'arrondi du verre de Tauri ne prend plus
///   sur les macOS récents.
pub fn regler_panneau(ns_window: *mut std::ffi::c_void) {
    let Some(fenetre) = (unsafe { (ns_window as *const NSWindow).as_ref() }) else { return };
    fenetre.setCollectionBehavior(
        fenetre.collectionBehavior() | NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    if let Some(vue) = fenetre.contentView() {
        vue.setWantsLayer(true);
        unsafe {
            let couche: *mut AnyObject = msg_send![&*vue, layer];
            if let Some(couche) = couche.as_ref() {
                let () = msg_send![couche, setCornerRadius: 12.0f64];
                let () = msg_send![couche, setMasksToBounds: true];
            }
        }
        // L'ombre suit la nouvelle forme.
        fenetre.invalidateShadow();
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(source: i32, event: u32) -> f64;
}

/// Secondes depuis la dernière action au clavier, à la souris ou au trackpad (Mac verrouillé compris).
pub fn inactif_depuis() -> f64 {
    // kCGEventSourceStateCombinedSessionState, kCGAnyInputEventType
    unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) }
}

/// Ouverture au démarrage du Mac, par le même service que la version Electron (Réglages > Général > Ouverture).
pub fn ouvre_au_demarrage() -> bool {
    unsafe { SMAppService::mainAppService().status() == SMAppServiceStatus::Enabled }
}

pub fn ouvrir_au_demarrage(oui: bool) -> Result<(), String> {
    let service = unsafe { SMAppService::mainAppService() };
    let r = unsafe { if oui { service.registerAndReturnError() } else { service.unregisterAndReturnError() } };
    r.map_err(|e| e.localizedDescription().to_string())
}
