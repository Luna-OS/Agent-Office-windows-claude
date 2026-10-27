//! Der Nachrichten-Bus einer Etage: Agenten und Menschen schreiben einander direkt oder an alle.
//! Jeder Arbeiter hat einen Lesezeiger, so bekommt er über `office-agenten posteingang` genau das,
//! was er noch nicht gesehen hat.

use std::collections::{HashMap, VecDeque};

use protokoll::agenten::BusNachricht;
use serde::{Deserialize, Serialize};

/// So viele Nachrichten behält eine Etage.
pub const MAX_NACHRICHTEN: usize = 500;
/// So lang darf eine Nachricht sein (Zeichen).
pub const MAX_TEXT: usize = 8000;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Bus {
    nachrichten: VecDeque<BusNachricht>,
    naechste: u64,
    /// Arbeiter-ID → höchste gelesene Nachrichten-ID.
    gelesen: HashMap<String, u64>,
}

/// `alle`, `*` oder nichts heißt: an alle.
pub fn empfaenger(an: Option<&str>) -> Option<String> {
    let an = an?.trim().trim_start_matches('@');
    if an.is_empty() || an == "*" || an.eq_ignore_ascii_case("alle") || an.eq_ignore_ascii_case("all") {
        None
    } else {
        Some(an.to_string())
    }
}

impl Bus {
    /// Legt eine Nachricht auf den Bus und gibt sie zurück.
    pub fn senden(&mut self, von: &str, von_arbeiter: Option<&str>, an: Option<&str>, text: &str, um: u64) -> BusNachricht {
        self.naechste += 1;
        let text: String = text.trim().chars().take(MAX_TEXT).collect();
        let n = BusNachricht {
            id: self.naechste,
            von: von.to_string(),
            von_arbeiter: von_arbeiter.map(str::to_string),
            an: empfaenger(an),
            text,
            um,
        };
        // Der Absender hat seine eigene Nachricht schon gelesen.
        if let Some(id) = von_arbeiter {
            self.gelesen.insert(id.to_string(), n.id);
        }
        self.nachrichten.push_back(n.clone());
        while self.nachrichten.len() > MAX_NACHRICHTEN {
            self.nachrichten.pop_front();
        }
        n
    }

    /// Geht `n` an den Arbeiter `id` mit Namen `name`?
    pub fn ist_fuer(n: &BusNachricht, id: &str, name: &str) -> bool {
        if n.von_arbeiter.as_deref() == Some(id) {
            return false;
        }
        match &n.an {
            None => true,
            Some(an) => an.eq_ignore_ascii_case(name) || an == id,
        }
    }

    /// Die ungelesenen Nachrichten für einen Arbeiter; danach gelten sie als gelesen.
    pub fn posteingang(&mut self, id: &str, name: &str) -> Vec<BusNachricht> {
        let ab = self.gelesen.get(id).copied().unwrap_or(0);
        let neu: Vec<_> = self.nachrichten.iter().filter(|n| n.id > ab && Self::ist_fuer(n, id, name)).cloned().collect();
        self.gelesen.insert(id.to_string(), self.naechste);
        neu
    }

    /// Markiert alles bis `bis` für einen Arbeiter als gelesen (z. B. nach Zustellung ins Terminal).
    pub fn gelesen_bis(&mut self, id: &str, bis: u64) {
        let eintrag = self.gelesen.entry(id.to_string()).or_insert(0);
        *eintrag = (*eintrag).max(bis);
    }

    /// Ein neuer Arbeiter liest nur, was nach seiner Ankunft kommt.
    pub fn ankunft(&mut self, id: &str) {
        self.gelesen.insert(id.to_string(), self.naechste);
    }

    pub fn vergessen(&mut self, id: &str) {
        self.gelesen.remove(id);
    }

    /// Die letzten `max` Nachrichten, älteste zuerst.
    pub fn verlauf(&self, max: usize) -> Vec<BusNachricht> {
        let start = self.nachrichten.len().saturating_sub(max);
        self.nachrichten.iter().skip(start).cloned().collect()
    }

    /// Die letzten `max` Nachrichten, die einen Arbeiter betreffen (an ihn, an alle oder von ihm).
    pub fn verlauf_fuer(&self, id: &str, name: &str, max: usize) -> Vec<BusNachricht> {
        let alle: Vec<_> = self
            .nachrichten
            .iter()
            .filter(|n| n.von_arbeiter.as_deref() == Some(id) || Self::ist_fuer(n, id, name))
            .cloned()
            .collect();
        let start = alle.len().saturating_sub(max);
        alle[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direkt_und_an_alle() {
        let mut bus = Bus::default();
        bus.ankunft("a");
        bus.ankunft("b");
        bus.ankunft("c");
        bus.senden("Ada", Some("a"), Some("Bob"), "Hallo Bob", 1);
        bus.senden("Ada", Some("a"), Some("alle"), "Hallo zusammen", 2);
        bus.senden("Luna", None, None, "Vom Menschen", 3);

        let b: Vec<_> = bus.posteingang("b", "Bob").into_iter().map(|n| n.text).collect();
        assert_eq!(b, ["Hallo Bob", "Hallo zusammen", "Vom Menschen"]);
        let c: Vec<_> = bus.posteingang("c", "Cleo").into_iter().map(|n| n.text).collect();
        assert_eq!(c, ["Hallo zusammen", "Vom Menschen"]);
        // Ada sieht ihre eigenen Nachrichten nicht im Posteingang.
        let a: Vec<_> = bus.posteingang("a", "Ada").into_iter().map(|n| n.text).collect();
        assert_eq!(a, ["Vom Menschen"]);
        // Zweimal lesen liefert nichts Neues.
        assert!(bus.posteingang("b", "Bob").is_empty());
    }

    #[test]
    fn neuer_arbeiter_liest_nur_neues() {
        let mut bus = Bus::default();
        bus.senden("Ada", Some("a"), None, "alt", 1);
        bus.ankunft("d");
        assert!(bus.posteingang("d", "Dora").is_empty());
        bus.senden("Ada", Some("a"), Some("dora"), "neu", 2);
        assert_eq!(bus.posteingang("d", "Dora")[0].text, "neu");
    }

    #[test]
    fn begrenzt() {
        let mut bus = Bus::default();
        for i in 0..(MAX_NACHRICHTEN + 10) {
            bus.senden("x", None, None, &format!("{i}"), i as u64);
        }
        assert_eq!(bus.verlauf(usize::MAX).len(), MAX_NACHRICHTEN);
        assert_eq!(bus.verlauf(2).len(), 2);
        assert_eq!(empfaenger(Some("@Bob")), Some("Bob".into()));
        assert_eq!(empfaenger(Some("*")), None);
    }
}
