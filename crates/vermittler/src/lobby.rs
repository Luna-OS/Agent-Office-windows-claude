//! Die Lobby: wer Gastgeber ist, wer als Ersatz wartet, und die durchgereichten Verbindungen.
//!
//! Gastgeber ist das Office eines Spielers (seine App), nicht der Vermittler: Es meldet sich an,
//! und alle Browser der Lobby werden zu ihm durchgereicht. Melden sich mehrere an, ist der erste
//! Gastgeber und die anderen warten; geht er, übernimmt der nächste.

use bytes::Bytes;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// So viele Offices dürfen sich höchstens als Gastgeber anmelden.
pub const GASTGEBER_MAX: usize = 32;

/// Was an das Office eines Gastgebers geht: eine Steuernachricht oder Daten einer Verbindung.
#[derive(Debug)]
pub enum Ausgang {
    Text(String),
    Daten(u32, Bytes),
}

/// Was einer durchgereichten Verbindung vom Gastgeber kommt: Daten, oder `None` fürs Ende.
pub type ZumBrowser = mpsc::Sender<Option<Bytes>>;

/// Ein angemeldetes Office.
pub struct Knoten {
    pub id: u64,
    pub name: String,
    pub ausgang: mpsc::Sender<Ausgang>,
    /// Die Verbindungen, die gerade zu ihm durchgereicht werden.
    pub stroeme: Mutex<HashMap<u32, ZumBrowser>>,
}

impl Knoten {
    pub fn strom(&self, id: u32) -> Option<ZumBrowser> {
        self.stroeme.lock().ok()?.get(&id).cloned()
    }

    pub fn strom_weg(&self, id: u32) -> Option<ZumBrowser> {
        self.stroeme.lock().ok()?.remove(&id)
    }
}

pub struct Lobby {
    passwort: String,
    gastgeber: Mutex<Vec<Arc<Knoten>>>,
    naechster_knoten: AtomicU64,
    naechster_strom: AtomicU32,
}

/// Wie eine Anmeldung ausging.
pub enum Anmeldung {
    /// Angemeldet; `aktiv`, wenn dieses Office jetzt Gastgeber ist, sonst wer es ist.
    Da {
        knoten: Arc<Knoten>,
        aktiv: bool,
        gastgeber: String,
    },
    Voll,
}

impl Lobby {
    pub fn neu(passwort: &str) -> Arc<Self> {
        Arc::new(Self {
            passwort: passwort.to_string(),
            gastgeber: Mutex::new(Vec::new()),
            naechster_knoten: AtomicU64::new(1),
            naechster_strom: AtomicU32::new(1),
        })
    }

    /// Vergleicht in gleichbleibender Zeit, damit die Antwortzeit nichts über das Passwort verrät.
    pub fn passwort_stimmt(&self, versuch: &str) -> bool {
        let (a, b) = (self.passwort.as_bytes(), versuch.as_bytes());
        let mut unterschied = a.len() ^ b.len();
        for i in 0..a.len().max(b.len()) {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            unterschied |= usize::from(x ^ y);
        }
        unterschied == 0
    }

    pub fn anmelden(&self, name: &str, ausgang: mpsc::Sender<Ausgang>) -> Anmeldung {
        let Ok(mut liste) = self.gastgeber.lock() else {
            return Anmeldung::Voll;
        };
        if liste.len() >= GASTGEBER_MAX {
            return Anmeldung::Voll;
        }
        let knoten = Arc::new(Knoten {
            id: self.naechster_knoten.fetch_add(1, Ordering::Relaxed),
            name: name.to_string(),
            ausgang,
            stroeme: Mutex::new(HashMap::new()),
        });
        liste.push(knoten.clone());
        Anmeldung::Da {
            aktiv: liste.len() == 1,
            gastgeber: liste[0].name.clone(),
            knoten,
        }
    }

    /// Meldet ein Office ab. War es Gastgeber, gibt es den nächsten zurück, der jetzt übernimmt.
    pub fn abmelden(&self, id: u64) -> Option<Arc<Knoten>> {
        let mut liste = self.gastgeber.lock().ok()?;
        let i = liste.iter().position(|k| k.id == id)?;
        let weg = liste.remove(i);
        // Seine Verbindungen enden mit ihm: ohne Sender endet jede Browser-Seite.
        if let Ok(mut s) = weg.stroeme.lock() {
            s.clear();
        }
        if i == 0 { liste.first().cloned() } else { None }
    }

    pub fn aktiver(&self) -> Option<Arc<Knoten>> {
        self.gastgeber.lock().ok()?.first().cloned()
    }

    /// Wer Gastgeber ist und wie viele als Ersatz warten.
    pub fn stand(&self) -> (Option<String>, usize) {
        match self.gastgeber.lock() {
            Ok(l) => (l.first().map(|k| k.name.clone()), l.len().saturating_sub(1)),
            Err(_) => (None, 0),
        }
    }

    pub fn neuer_strom(&self) -> u32 {
        self.naechster_strom.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kanal() -> mpsc::Sender<Ausgang> {
        mpsc::channel(1).0
    }

    #[test]
    fn passwort_vergleich() {
        let l = Lobby::neu("geheim");
        assert!(l.passwort_stimmt("geheim"));
        assert!(!l.passwort_stimmt("geheim2"));
        assert!(!l.passwort_stimmt("gehei"));
        assert!(!l.passwort_stimmt(""));
    }

    #[test]
    fn der_erste_ist_gastgeber_und_der_naechste_uebernimmt() {
        let l = Lobby::neu("x");
        let Anmeldung::Da {
            knoten: a, aktiv, ..
        } = l.anmelden("Luna", kanal())
        else {
            panic!()
        };
        assert!(aktiv);
        let Anmeldung::Da {
            knoten: b,
            aktiv,
            gastgeber,
        } = l.anmelden("Kim", kanal())
        else {
            panic!()
        };
        assert!(!aktiv);
        assert_eq!(gastgeber, "Luna");
        assert_eq!(l.stand(), (Some("Luna".into()), 1));
        // Der Ersatz geht: niemand übernimmt.
        assert!(l.abmelden(b.id).is_none());
        let Anmeldung::Da { knoten: c, .. } = l.anmelden("Mo", kanal()) else {
            panic!()
        };
        // Der Gastgeber geht: Mo übernimmt.
        assert_eq!(l.abmelden(a.id).map(|k| k.id), Some(c.id));
        assert_eq!(l.stand(), (Some("Mo".into()), 0));
    }
}
