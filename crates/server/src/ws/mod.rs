//! Der WebSocket des Spiels: ein Register von Domänen, jede in ihrer eigenen Datei.
//!
//! Eine neue Domäne implementiert [`Domaene`] und wird in [`Register::standard`] eingetragen.
//! Die Reihenfolge zählt: `praesenz` begrüßt zuerst (Willkommen), `etagen` schickt beim Betreten
//! zuerst `Betreten`, danach folgen die Stände der übrigen Domänen.

pub mod anmeldungen;
pub mod eingabe;
pub mod einstellungen;
pub mod etagen;
pub mod konten;
pub mod praesenz;
pub mod spielzeug;
pub mod verbindung;

use std::future::Future;
use std::pin::Pin;

use protokoll::{ClientNachricht, ServerNachricht};

use crate::office::OfficeRef;

pub type Zukunft<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Eine Domäne des Protokolls auf dem Server.
pub trait Domaene: Send + Sync {
    /// Für Logs.
    fn name(&self) -> &'static str;
    /// Ist diese Nachricht meine?
    fn zustaendig(&self, n: &ClientNachricht) -> bool;
    /// Bearbeitet eine Nachricht von Verbindung `von`.
    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a>;
    /// Einmal direkt nach dem Verbinden.
    fn begruessung(&self, _o: &OfficeRef, _von: &str) -> Vec<ServerNachricht> {
        Vec::new()
    }
    /// Nach dem Verbinden und bei jedem Etagenwechsel (die Person steht schon auf der neuen Etage).
    fn etage_betreten(&self, _o: &OfficeRef, _von: &str) -> Vec<ServerNachricht> {
        Vec::new()
    }
    /// Vor einem Etagenwechsel: lässt los, was `von` auf `etage` hatte.
    fn etage_verlassen(&self, _o: &OfficeRef, _von: &str, _etage: Option<&str>) {}
    /// Die Verbindung ist zu (die Person ist schon ausgetragen; `etage` war ihre letzte).
    fn weg(&self, _o: &OfficeRef, _von: &str, _etage: Option<&str>) {}
}

#[derive(Default)]
pub struct Register {
    domaenen: Vec<Box<dyn Domaene>>,
}

impl Register {
    /// Die eingebauten Domänen des Servers, in ihrer Reihenfolge.
    pub fn standard() -> Self {
        Self::standard_mit(Vec::new())
    }

    /// Die eingebauten Domänen, danach `weitere` (die Anbindungen agenten und medien).
    pub fn standard_mit(weitere: Vec<Box<dyn Domaene>>) -> Self {
        let mut r = Self::default();
        r.eintragen(praesenz::Praesenz);
        r.eintragen(konten::KontenDomaene);
        r.eintragen(anmeldungen::AnmeldungenDomaene);
        r.eintragen(einstellungen::EinstellungenDomaene);
        r.eintragen(etagen::EtagenDomaene);
        r.eintragen(spielzeug::SpielzeugDomaene);
        r.domaenen.extend(weitere);
        r
    }

    pub fn eintragen(&mut self, d: impl Domaene + 'static) {
        self.domaenen.push(Box::new(d));
    }

    pub fn alle(&self) -> impl Iterator<Item = &dyn Domaene> {
        self.domaenen.iter().map(|d| d.as_ref())
    }

    /// Gibt eine Nachricht an die zuständige Domäne; `false`, wenn keine sie will.
    pub async fn verteilen(&self, o: &OfficeRef, von: &str, n: ClientNachricht) -> bool {
        match self.domaenen.iter().find(|d| d.zustaendig(&n)) {
            Some(d) => {
                d.nachricht(o, von, n).await;
                true
            }
            None => false,
        }
    }

    /// Begrüßung aller Domänen, in Reihenfolge direkt an `von` (so kommt `Willkommen` sicher zuerst,
    /// auch wenn eine spätere Domäne nebenbei schon Ereignisse an die Verbindung schickt).
    pub fn begruessen(&self, o: &OfficeRef, von: &str) {
        for d in &self.domaenen {
            for n in d.begruessung(o, von) {
                o.an(von, &n);
            }
        }
    }

    /// Die Etagen-Stände aller Domänen, in Reihenfolge direkt an `von`.
    pub fn etage_betreten(&self, o: &OfficeRef, von: &str) {
        for d in &self.domaenen {
            for n in d.etage_betreten(o, von) {
                o.an(von, &n);
            }
        }
    }

    pub fn etage_verlassen(&self, o: &OfficeRef, von: &str, etage: Option<&str>) {
        for d in &self.domaenen {
            d.etage_verlassen(o, von, etage);
        }
    }

    pub fn weg(&self, o: &OfficeRef, von: &str, etage: Option<&str>) {
        for d in &self.domaenen {
            d.weg(o, von, etage);
        }
    }
}

/// Kurzform: eine synchrone Bearbeitung als fertige Zukunft.
pub fn fertig<'a>() -> Zukunft<'a> {
    Box::pin(std::future::ready(()))
}
