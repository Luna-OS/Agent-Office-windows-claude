//! Adapter für `agenten` (vega): Arbeiter, Aufgaben-Board, GitHub, Agent-zu-Agent-Bus.

use std::path::Path;
use std::sync::Arc;

use ::agenten::{Absender, Agenten, AgentenEinstellungen, Ziel};
use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::broadcast::error::RecvError;

use crate::konfig::Konfig;
use crate::office::{Anbindung, DACH, OfficeRef};
use crate::ws::{self, Zukunft};

pub async fn starten(k: &Konfig) -> anyhow::Result<Agenten> {
    Agenten::starten(AgentenEinstellungen { daten_verzeichnis: k.daten.clone(), hook_port: 0, cli_pfad: None }).await
}

/// Der Absender für `agenten`: wer, auf welcher Etage (das Dach zählt als keine Projekt-Etage).
fn absender(o: &OfficeRef, von: &str) -> Absender {
    let p = o.person(von);
    Absender {
        verbindung: von.into(),
        name: p.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        etage: p.and_then(|p| p.etage).filter(|e| e != DACH),
        konto: o.sitzung_von(von).and_then(|s| s.konto),
    }
}

pub struct Domaene(pub Agenten);

impl ws::Domaene for Domaene {
    fn name(&self) -> &'static str {
        "agenten"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Agenten(_) | ClientNachricht::Arbeiter(_) | ClientNachricht::Aufgaben(_) | ClientNachricht::Github(_))
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        let a = absender(o, von);
        Box::pin(async move { self.0.nachricht(&a, n).await })
    }

    fn begruessung(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        self.0.begruessung(&absender(o, von))
    }

    fn etage_betreten(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        if o.etage_von(von).as_deref() == Some(DACH) {
            return Vec::new();
        }
        self.0.etage_betreten(&absender(o, von))
    }

    fn etage_verlassen(&self, _o: &OfficeRef, von: &str, _etage: Option<&str>) {
        // Wer die Etage wechselt, schaut kein Terminal der alten mehr an.
        self.0.verbindung_weg(von);
    }

    fn weg(&self, _o: &OfficeRef, von: &str, _etage: Option<&str>) {
        self.0.verbindung_weg(von);
    }
}

pub fn router(a: &Agenten) -> axum::Router {
    a.router()
}

struct AgentenAnbindung(Agenten);

impl Anbindung for AgentenAnbindung {
    fn etage_angemeldet(&self, id: &str, ordner: &Path) {
        self.0.etage_anmelden(id, ordner.to_path_buf());
    }

    fn etage_abgemeldet(&self, id: &str) {
        self.0.etage_abmelden(id);
    }

    fn zahlen(&self, etage: &str) -> Option<crate::etagen::Zahlen> {
        let (arbeiter, beschaeftigt, wartend) = self.0.zahlen(etage);
        Some(crate::etagen::Zahlen { arbeiter: arbeiter as u32, beschaeftigt: beschaeftigt as u32, wartend: wartend as u32 })
    }

    fn fluegel_geaendert(&self, id: &str, fluegel: u32) {
        self.0.tische_setzen(id, crate::etagen::arbeitsplaetze(fluegel));
    }
}

/// Ereignisse von `agenten` an die Browser verteilen.
pub fn verbinden(o: &OfficeRef, a: Agenten) {
    let mut rx = a.ereignisse();
    o.anbinden(Arc::new(AgentenAnbindung(a.clone())));
    // Arbeiter eines Kontos laufen mit dessen eigenen Anmeldungen (leerer Wert = Variable entfernen).
    let schwach = Arc::downgrade(o);
    a.konto_umgebung_setzen(Arc::new(move |konto: &str| schwach.upgrade().map(|o| crate::ws::anmeldungen::umgebung(&o, konto, None)).unwrap_or_default()));
    let _ = o.agenten.set(a);
    let o = o.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(e) => match e.ziel {
                    Ziel::Etage(etage) => o.an_etage(Some(&etage), &e.nachricht, None),
                    Ziel::Verbindung(v) => o.an(&v, &e.nachricht),
                    Ziel::Verbindungen(vs) => o.an_viele(&vs, &e.nachricht),
                    Ziel::Alle => o.an_alle(&e.nachricht, None),
                },
                Err(RecvError::Lagged(n)) => tracing::warn!("{n} Agenten-Ereignisse verpasst"),
                Err(RecvError::Closed) => break,
            }
        }
    });
}
