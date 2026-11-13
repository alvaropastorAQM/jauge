//! What Tauri does not do by itself on macOS: icon position and font, idle time, opening at login.

use objc2::runtime::AnyObject;
use objc2::{msg_send, MainThreadMarker};
use objc2_app_kit::{NSFont, NSFontWeightRegular, NSStatusItem, NSWindow, NSWindowCollectionBehavior};
use objc2_foundation::{NSString, NSUserDefaults};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Fixed name of the icon, the same as the Electron version: macOS remembers its position under this name from one launch to the next.
const NOM_BARRE: &str = "b3f1e7a2-5c4d-4e8f-9a6b-2d7c1e0f8a35";
/// Distance from the right edge of the screen, in points: next to the battery (285), on the visible side of the notch.
/// Set only once: if you move the icon with ⌘, macOS rewrites this value and your position is kept.
const PLACE_BARRE: f64 = 300.0;

/// To be called on the main thread, right after the icon is created.
pub fn regler_icone(item: &NSStatusItem) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let nom = NSString::from_str(NOM_BARRE);
    let cle = NSString::from_str(&format!("NSStatusItem Preferred Position {NOM_BARRE}"));
    let reglages = NSUserDefaults::standardUserDefaults();
    if reglages.doubleForKey(&cle) == 0.0 {
        reglages.setDouble_forKey(PLACE_BARRE, &cle);
    }
    item.setAutosaveName(Some(&nom));
    // Fixed-width digits, like the system icons: the percentage does not shift the neighbouring icons.
    if let Some(bouton) = item.button(mtm) {
        let police = unsafe { NSFont::monospacedDigitSystemFontOfSize_weight(NSFont::systemFontSize(), NSFontWeightRegular) };
        bouton.setFont(Some(&police));
    }
}

/// What Tauri does not set for the panel, to be called on the main thread:
/// - it also shows over a full-screen app, like the Electron version (Tauri only sets "all desktops");
/// - its corners are rounded to 12 points by clipping the whole window (glass and page): Tauri's glass rounding no longer
///   applies on recent macOS.
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
        // The shadow follows the new shape.
        fenetre.invalidateShadow();
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(source: i32, event: u32) -> f64;
}

/// Seconds since the last keyboard, mouse or trackpad action (including while the Mac is locked).
pub fn inactif_depuis() -> f64 {
    // kCGEventSourceStateCombinedSessionState, kCGAnyInputEventType
    unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) }
}

/// Opening at login, through the same service as the Electron version (Settings > General > Login Items).
pub fn ouvre_au_demarrage() -> bool {
    unsafe { SMAppService::mainAppService().status() == SMAppServiceStatus::Enabled }
}

pub fn ouvrir_au_demarrage(oui: bool) -> Result<(), String> {
    let service = unsafe { SMAppService::mainAppService() };
    let r = unsafe { if oui { service.registerAndReturnError() } else { service.unregisterAndReturnError() } };
    r.map_err(|e| e.localizedDescription().to_string())
}
