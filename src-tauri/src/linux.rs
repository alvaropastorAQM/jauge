//! The Linux side of what `macos.rs` does on a Mac. Under WSLg there is no tray and no session to watch, so Jauge runs as
//! a normal window: idle time is unknown and opening at login is not offered.

/// Seconds since the last keyboard or mouse action: unknown here, so Jauge always counts as present.
pub fn inactif_depuis() -> f64 {
    0.0
}

pub fn ouvre_au_demarrage() -> bool {
    false
}

pub fn ouvrir_au_demarrage(_oui: bool) -> Result<(), String> {
    Err("Opening at login is not available on Linux.".into())
}
