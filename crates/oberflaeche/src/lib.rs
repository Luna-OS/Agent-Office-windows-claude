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
pub mod lite;
pub mod lokal;
pub mod modal;
mod seitenleiste;
mod tasten;
pub mod terminal;
pub mod texte;
pub mod ui;
pub mod zustand;

pub use bruecke::{Befehl, Bruecke, senden};
pub use empfang::empfangen;
pub use lite::ist_lite;
pub use modal::{Fenster, oeffnen as fenster_oeffnen};

use leptos::prelude::*;

/// Hängt die Oberfläche in `<body>` und prüft die Anmeldung.
pub fn starten(bruecke: Bruecke) {
    bruecke::setzen(bruecke);
    leptos::mount::mount_to_body(app::App);
    modal::esc_abfangen();
    tasten::anmelden();
    anmeldung::pruefen();
    // Die Perspektive aus den Einstellungen an die Welt geben, jetzt und bei jeder Änderung.
    Effect::new(|_| {
        let ich = lokal::lokal().with(|l| l.ich_perspektive);
        bruecke::befehl(Befehl::Perspektive(ich));
    });
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

/// Was der Client über Medien weiß: teilt man selbst, ist man im Voice, ist das Mikro stumm.
pub fn medien_stand(teilt: bool, voice: bool, stumm: bool) {
    let z = zustand::zustand();
    z.teilt.set(teilt);
    z.voice.set(voice);
    z.stumm.set(stumm);
}

/// Die Hinweiszeile unten in der Mitte (von der Welt): was E gerade tun würde.
pub fn hinweis(text: Option<String>) {
    zustand::zustand().hinweis.set(text);
}

/// Der Fernseher: der erste geteilte Bildschirm groß, sonst das Teilen-Fenster.
pub fn tv_oeffnen() {
    let erster = zustand::zustand().bildschirme.with_untracked(|l| l.first().map(|b| b.spieler.clone()));
    modal::oeffnen(match erster {
        Some(s) => Fenster::Bildschirm(s),
        None => Fenster::Teilen,
    });
}
