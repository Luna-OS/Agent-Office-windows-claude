//! Der gemeinsame Zustand des laufenden Office und das Verteilen von Nachrichten an die Browser.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use axum::extract::ws::Utf8Bytes;
use protokoll::ServerNachricht;
use protokoll::praesenz::{ChatZeile, Person};
use tokio::sync::mpsc;

use crate::anmeldung::{Sitzung, Zugang};
use crate::einstellungen::Einstellungen;
use crate::etagen::Etagen;
use crate::konfig::Konfig;
use crate::spielzeug::Spielzeug;
use crate::ws::Register;

pub use protokoll::etagen::DACH;
const CHAT_LAENGE: usize = 200;

/// Was an eine Verbindung hinausgeht.
#[derive(Debug, Clone)]
pub enum Ausgang {
    Text(Utf8Bytes),
    /// Verbindung schließen (abgemeldet), mit Grund.
    Schliessen(&'static str),
}

/// Ein verbundener Browser.
pub struct Verbindung {
    pub tx: mpsc::UnboundedSender<Ausgang>,
    pub person: Person,
    pub sitzung: Sitzung,
    pub emotes: EmoteEimer,
}

/// Emote-Bremse: 3 auf einmal, dann eins alle 2 s.
#[derive(Debug, Clone)]
pub struct EmoteEimer {
    marken: f64,
    zuletzt: u64,
}

impl Default for EmoteEimer {
    fn default() -> Self {
        Self { marken: 3.0, zuletzt: 0 }
    }
}

impl EmoteEimer {
    pub fn nehmen(&mut self, jetzt_ms: u64) -> bool {
        self.marken = (self.marken + jetzt_ms.saturating_sub(self.zuletzt) as f64 / 2000.0).min(3.0);
        self.zuletzt = jetzt_ms;
        if self.marken >= 1.0 {
            self.marken -= 1.0;
            true
        } else {
            false
        }
    }
}

pub struct Office {
    pub konfig: Konfig,
    pub zugang: Zugang,
    pub verbindungen: Mutex<HashMap<String, Verbindung>>,
    pub chat: Mutex<VecDeque<ChatZeile>>,
    pub etagen: Etagen,
    pub spielzeug: Spielzeug,
    pub einstellungen: Einstellungen,
    pub register: Register,
    pub version: String,
}

/// Serialisiert eine Nachricht einmal für viele Empfänger.
pub fn text(n: &ServerNachricht) -> Utf8Bytes {
    Utf8Bytes::from(serde_json::to_string(n).unwrap_or_else(|e| format!(r#"{{"domaene":"fehler","nachricht":{{"text":"{e}"}}}}"#)))
}

impl Office {
    /// Die Person hinter einer Verbindung (Kopie).
    pub fn person(&self, id: &str) -> Option<Person> {
        self.verbindungen.lock().unwrap().get(id).map(|v| v.person.clone())
    }

    /// Ändert die Person hinter einer Verbindung; gibt die neue Fassung zurück.
    pub fn person_aendern(&self, id: &str, f: impl FnOnce(&mut Person)) -> Option<Person> {
        let mut v = self.verbindungen.lock().unwrap();
        let p = &mut v.get_mut(id)?.person;
        f(p);
        Some(p.clone())
    }

    pub fn etage_von(&self, id: &str) -> Option<String> {
        self.verbindungen.lock().unwrap().get(id).and_then(|v| v.person.etage.clone())
    }

    pub fn sitzung_von(&self, id: &str) -> Option<Sitzung> {
        self.verbindungen.lock().unwrap().get(id).map(|v| v.sitzung.clone())
    }

    pub fn ist_admin(&self, id: &str) -> bool {
        self.sitzung_von(id).is_some_and(|s| self.zugang.ich(&s).admin)
    }

    /// Alle Menschen im Office.
    pub fn personen(&self) -> Vec<Person> {
        self.verbindungen.lock().unwrap().values().map(|v| v.person.clone()).collect()
    }

    /// Wie viele Menschen auf jeder Etage sind.
    pub fn personen_je_etage(&self) -> HashMap<String, u32> {
        let mut m = HashMap::new();
        for v in self.verbindungen.lock().unwrap().values() {
            if let Some(e) = &v.person.etage {
                *m.entry(e.clone()).or_insert(0) += 1;
            }
        }
        m
    }

    /// Konten, die gerade im Office sind.
    pub fn konten_online(&self) -> HashSet<String> {
        self.verbindungen.lock().unwrap().values().filter_map(|v| v.sitzung.konto.clone()).collect()
    }

    pub fn an(&self, id: &str, n: &ServerNachricht) {
        self.an_roh(id, text(n));
    }

    pub fn an_roh(&self, id: &str, t: Utf8Bytes) {
        if let Some(v) = self.verbindungen.lock().unwrap().get(id) {
            let _ = v.tx.send(Ausgang::Text(t));
        }
    }

    pub fn an_viele(&self, ids: &[String], n: &ServerNachricht) {
        let t = text(n);
        let v = self.verbindungen.lock().unwrap();
        for id in ids {
            if let Some(v) = v.get(id) {
                let _ = v.tx.send(Ausgang::Text(t.clone()));
            }
        }
    }

    /// An alle, für die `filter` gilt (außer `ausser`).
    pub fn an_wen(&self, n: &ServerNachricht, ausser: Option<&str>, filter: impl Fn(&Verbindung) -> bool) {
        let t = text(n);
        for (id, v) in self.verbindungen.lock().unwrap().iter() {
            if Some(id.as_str()) != ausser && filter(v) {
                let _ = v.tx.send(Ausgang::Text(t.clone()));
            }
        }
    }

    pub fn an_alle(&self, n: &ServerNachricht, ausser: Option<&str>) {
        self.an_wen(n, ausser, |_| true);
    }

    /// An alle auf einer Etage (`None` = Gebäude ohne Etagen).
    pub fn an_etage(&self, etage: Option<&str>, n: &ServerNachricht, ausser: Option<&str>) {
        self.an_wen(n, ausser, |v| v.person.etage.as_deref() == etage);
    }

    /// An alle auf derselben Etage wie `id` (ohne `id` selbst).
    pub fn an_nachbarn(&self, id: &str, n: &ServerNachricht) {
        let etage = self.etage_von(id);
        self.an_etage(etage.as_deref(), n, Some(id));
    }

    pub fn an_admins(&self, n: &ServerNachricht) {
        let t = text(n);
        for v in self.verbindungen.lock().unwrap().values() {
            if self.zugang.ich(&v.sitzung).admin {
                let _ = v.tx.send(Ausgang::Text(t.clone()));
            }
        }
    }

    /// Eine Meldung an einen Browser.
    pub fn hinweis(&self, id: &str, text: impl Into<String>, stufe: protokoll::praesenz::Stufe) {
        self.an(id, &ServerNachricht::Praesenz(protokoll::praesenz::Server::Hinweis { text: text.into(), stufe }));
    }

    pub fn fehler(&self, id: &str, text: impl Into<String>) {
        self.an(id, &ServerNachricht::Fehler { text: text.into() });
    }

    pub fn chat_merken(&self, z: ChatZeile) {
        let mut c = self.chat.lock().unwrap();
        c.push_back(z);
        while c.len() > CHAT_LAENGE {
            c.pop_front();
        }
    }

    pub fn chat_letzte(&self, n: usize) -> Vec<ChatZeile> {
        let c = self.chat.lock().unwrap();
        c.iter().skip(c.len().saturating_sub(n)).cloned().collect()
    }

    /// Schließt alle Verbindungen, deren Sitzung nicht mehr gilt (Konto entzogen, Passwort aus).
    pub fn ungueltige_abmelden(&self) {
        for v in self.verbindungen.lock().unwrap().values() {
            if !self.zugang.gueltig(&v.sitzung) {
                let _ = v.tx.send(Ausgang::Schliessen("Abgemeldet"));
            }
        }
    }
}

pub type OfficeRef = Arc<Office>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emote_bremse() {
        let mut e = EmoteEimer::default();
        assert!(e.nehmen(1000) && e.nehmen(1000) && e.nehmen(1000));
        assert!(!e.nehmen(1000));
        assert!(e.nehmen(3000));
    }
}
