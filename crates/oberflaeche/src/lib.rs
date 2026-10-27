//! Die Oberfläche mit Leptos und rust-ui: HUD, Menüs, Fenster, Terminals.
//!
//! Der Client ruft [`starten`] mit einer [`Bruecke`] auf und reicht jeden Server-Frame an [`empfangen`].
//! Alles andere (Welt, WebSocket) kennt die Oberfläche nur über die Brücke.

mod anmeldung;
mod app;
mod beitreten;
mod bruecke;
mod chat;
mod empfang;
pub mod fenster;
mod http;
mod hud;
pub mod modal;
mod seitenleiste;
mod tasten;
pub mod terminal;
pub mod texte;
pub mod ui;
pub mod zustand;

pub use bruecke::{Bruecke, senden};
pub use empfang::empfangen;
pub use modal::{Fenster, oeffnen as fenster_oeffnen};

use leptos::prelude::*;

/// Hängt die Oberfläche in `<body>` und prüft die Anmeldung.
pub fn starten(bruecke: Bruecke) {
    bruecke::setzen(bruecke);
    leptos::mount::mount_to_body(app::App);
    modal::esc_abfangen();
    tasten::anmelden();
    anmeldung::pruefen();
}

/// Der WebSocket steht.
pub fn verbunden() {
    let z = zustand::zustand();
    z.verbunden.set(true);
    z.war_verbunden.set(true);
}

/// Der WebSocket ist weg: kurz warten, Anmeldung prüfen, neu verbinden (oder die Anmeldeseite zeigen).
pub fn getrennt() {
    zustand::zustand().verbunden.set(false);
    set_timeout(anmeldung::pruefen, std::time::Duration::from_millis(1500));
}
