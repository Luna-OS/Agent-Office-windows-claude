//! Meldungen nach außen (Team-Webhook des Servers): ein Arbeiter wartet auf einen Menschen, ist
//! fertig, sein PR ist gemergt, er ist heimgegangen. Der Server meldet sich mit einem Rückruf an.

use std::sync::Arc;

/// Was passiert ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeldungArt {
    Wartet,
    Fertig,
    PrGemergt,
    Heimgegangen,
}

/// Eine Meldung für den Webhook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meldung {
    pub etage: String,
    pub arbeiter_name: String,
    pub art: MeldungArt,
    /// Fertiger Satz auf Deutsch.
    pub text: String,
}

pub type MeldungsEmpfaenger = Arc<dyn Fn(Meldung) + Send + Sync>;

/// Rückruf: die Ports aller Dienste, wenn sich etwas ändert.
pub type DiensteBeobachter = Arc<dyn Fn(Vec<u16>) + Send + Sync>;

impl crate::Innen {
    pub(crate) fn melden(&self, etage: &str, name: &str, art: MeldungArt, text: String) {
        let f = self
            .meldungen
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(f) = f {
            let m = Meldung {
                etage: etage.to_string(),
                arbeiter_name: name.to_string(),
                art,
                text,
            };
            // Nie unter dem Zustands-Schloss in fremden Code: eigener Task.
            self.laufzeit.spawn(async move { f(m) });
        }
    }

    pub(crate) fn dienste_geaendert(&self, ports: Vec<u16>) {
        let f = self
            .dienste_beobachter
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(f) = f {
            self.laufzeit.spawn(async move { f(ports) });
        }
    }
}
