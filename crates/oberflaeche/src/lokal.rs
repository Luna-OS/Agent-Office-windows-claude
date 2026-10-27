//! Was nur dieser Browser sich merkt (im localStorage): welche HUD-Tafeln sichtbar sind, Perspektive,
//! Lautstärken. Ein neuer Wert ist ein Feld in [`Lokal`] mit `#[serde(default)]`.

use std::cell::OnceCell;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

const SCHLUESSEL: &str = "oao-einstellungen";

fn ja() -> bool {
    true
}

fn halb() -> f32 {
    0.6
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lokal {
    #[serde(default = "ja")]
    pub tafel_arbeiter: bool,
    #[serde(default = "ja")]
    pub tafel_personen: bool,
    #[serde(default = "ja")]
    pub tafel_chat: bool,
    /// Ich-Perspektive (sonst Schulterblick).
    #[serde(default = "ja")]
    pub ich_perspektive: bool,
    /// Gesamtlautstärke 0–1.
    #[serde(default = "halb")]
    pub lautstaerke: f32,
    #[serde(default)]
    pub stumm: bool,
    /// Voice beitreten mit stummem Mikro (Push-to-Talk mit V).
    #[serde(default)]
    pub voice_stumm: bool,
}

impl Default for Lokal {
    fn default() -> Self {
        serde_json::from_str("{}").expect("Standardwerte")
    }
}

thread_local! {
    static LOKAL: OnceCell<RwSignal<Lokal>> = const { OnceCell::new() };
}

fn speicher() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Die Einstellungen dieses Browsers (beim ersten Aufruf aus dem localStorage geladen).
pub fn lokal() -> RwSignal<Lokal> {
    LOKAL.with(|l| {
        *l.get_or_init(|| {
            let geladen = speicher()
                .and_then(|s| s.get_item(SCHLUESSEL).ok().flatten())
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
            RwSignal::new(geladen)
        })
    })
}

/// Ändert die Einstellungen und speichert sie.
pub fn aendern(f: impl FnOnce(&mut Lokal)) {
    let l = lokal();
    l.update(f);
    if let (Some(s), Ok(t)) = (speicher(), serde_json::to_string(&l.get_untracked())) {
        let _ = s.set_item(SCHLUESSEL, &t);
    }
}
