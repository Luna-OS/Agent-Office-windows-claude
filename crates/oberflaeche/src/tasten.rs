//! Tastenkürzel des Office, solange kein Fenster offen ist und man nicht in ein Feld tippt.
//! Ein neues Kürzel ist eine Zeile in [`KUERZEL`] und eine in der Hilfe ([`crate::fenster::hilfe`]).

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::bruecke::{Befehl, befehl};
use crate::modal::{self, Fenster};
use crate::zustand::zustand;
use protokoll::{ClientNachricht, praesenz};

/// Taste → was sie tut.
const KUERZEL: &[(&str, fn())] = &[
    ("Tab", || modal::umschalten(Fenster::Menue)),
    ("F1", || modal::umschalten(Fenster::Hilfe)),
    ("?", || modal::umschalten(Fenster::Hilfe)),
    ("t", chat_oeffnen),
    ("T", chat_oeffnen),
    ("/", || modal::oeffnen(Fenster::Palette)),
    ("v", voice_taste),
    ("V", voice_taste),
    ("m", || befehl(Befehl::StummUmschalten)),
    ("M", || befehl(Befehl::StummUmschalten)),
    ("g", emote_leiste),
    ("G", emote_leiste),
    ("1", || emote(0)),
    ("2", || emote(1)),
    ("3", || emote(2)),
    ("4", || emote(3)),
    ("5", || emote(4)),
    ("6", || emote(5)),
];

/// Die Emotes in der Reihenfolge der Tasten 1–6.
pub const EMOTES: [(praesenz::Emote, &str, &str); 6] = [
    (praesenz::Emote::Winken, "👋", "winken"),
    (praesenz::Emote::Daumen, "👍", "Daumen hoch"),
    (praesenz::Emote::Klatschen, "👏", "klatschen"),
    (praesenz::Emote::Tanzen, "💃", "tanzen"),
    (praesenz::Emote::Zeigen, "👉", "zeigen"),
    (praesenz::Emote::Facepalm, "🤦", "Facepalm"),
];

/// Ein Emote für alle auf der Etage.
pub fn emote(i: usize) {
    if let Some((e, _, _)) = EMOTES.get(i) {
        crate::bruecke::senden(ClientNachricht::Praesenz(praesenz::Client::Emote {
            emote: *e,
        }));
        befehl(Befehl::EigenesEmote(*e));
    }
    let z = zustand();
    if z.emotes_offen.get_untracked() {
        z.emotes_offen.set(false);
        crate::bruecke::fenster(false);
    }
}

/// G: die Emote-Leiste auf- oder zuklappen (offen gibt sie die Maus frei, damit man klicken kann).
fn emote_leiste() {
    let z = zustand();
    let offen = !z.emotes_offen.get_untracked();
    z.emotes_offen.set(offen);
    crate::bruecke::fenster(offen);
}

/// V: außerhalb des Voice beitreten, drinnen sprechen, solange V gedrückt ist.
fn voice_taste() {
    if zustand().voice.get_untracked() {
        befehl(Befehl::SprechenStart);
    } else {
        befehl(Befehl::VoiceBeitreten);
    }
}

fn chat_oeffnen() {
    zustand().chat_offen.set(true);
    crate::bruecke::fenster(true);
}

/// Ob gerade in ein Eingabefeld getippt wird.
pub fn tippt(e: &web_sys::KeyboardEvent) -> bool {
    let Some(ziel) = e
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    else {
        return false;
    };
    let tag = ziel.tag_name();
    tag == "INPUT" || tag == "TEXTAREA" || ziel.closest(".terminal").ok().flatten().is_some()
}

/// Hängt die Kürzel an das Fenster.
pub(crate) fn anmelden() {
    let _ = window_event_listener(leptos::ev::keydown, |e| {
        if zustand().phase.get_untracked() != crate::zustand::Phase::Office {
            return;
        }
        // Strg+K (⌘K): die Befehlspalette, auch aus einem Fenster heraus.
        if (e.ctrl_key() || e.meta_key()) && e.key().eq_ignore_ascii_case("k") {
            e.prevent_default();
            modal::umschalten(Fenster::Palette);
            return;
        }
        if e.ctrl_key() || e.meta_key() || e.alt_key() || tippt(&e) {
            return;
        }
        // Esc klappt die Emote-Leiste zu.
        if e.key() == "Escape" && zustand().emotes_offen.get_untracked() {
            emote_leiste();
            return;
        }
        let key = e.key();
        // Tab schließt das Menü auch wieder.
        if modal::offen()
            && !(key == "Tab"
                && zustand()
                    .fenster
                    .with_untracked(|s| s.last() == Some(&Fenster::Menue)))
        {
            return;
        }
        if e.repeat() {
            return;
        }
        if let Some((_, tun)) = KUERZEL.iter().find(|(k, _)| *k == key) {
            e.prevent_default();
            tun();
        }
    });
    // V losgelassen (oder das Fenster verliert den Fokus): nicht mehr sprechen.
    let _ = window_event_listener(leptos::ev::keyup, |e| {
        if e.key().eq_ignore_ascii_case("v") && zustand().voice.get_untracked() {
            befehl(Befehl::SprechenStopp);
        }
    });
    let _ = window_event_listener(leptos::ev::blur, |_| {
        if zustand().voice.get_untracked() {
            befehl(Befehl::SprechenStopp);
        }
    });
}
