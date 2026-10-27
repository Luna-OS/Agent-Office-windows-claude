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

/// Macht aus dem [`super::Anfrager`] den `agenten::Absender`, den vegas Routen als Extension erwarten.
async fn absender_einsetzen(mut req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    if let Some(a) = req.extensions().get::<super::Anfrager>().cloned() {
        req.extensions_mut().insert(Absender {
            verbindung: a.verbindung.unwrap_or_default(),
            name: a.person.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
            etage: a.person.and_then(|p| p.etage).filter(|e| e != DACH),
            konto: a.sitzung.konto,
        });
    }
    next.run(req).await
}

pub fn router(a: &Agenten) -> axum::Router {
    a.router().layer(axum::middleware::from_fn(absender_einsetzen))
}

struct AgentenAnbindung(Agenten);

impl Anbindung for AgentenAnbindung {
    fn etage_angemeldet(&self, id: &str, ordner: &Path) {
        self.0.etage_anmelden(id, ordner.to_path_buf());
        // Die Stühle am Besprechungstisch (Kopf zuerst) aus `welt`; einen Kerker hat die Karte noch nicht.
        let stuehle = welt::grundriss::sitze::besprechungs_tische().into_iter().map(|t| t.id).collect();
        self.0.besprechung_stuehle_setzen(id, stuehle);
        self.0.kerker_aktiv_setzen(id, false);
    }

    fn etage_abgemeldet(&self, id: &str) {
        self.0.etage_abmelden(id);
    }

    fn dienst_ports(&self) -> Option<Vec<u16>> {
        Some(self.0.dienste().into_iter().map(|d| d.port).collect())
    }

    fn zahlen(&self, etage: &str) -> Option<crate::etagen::Zahlen> {
        let (arbeiter, beschaeftigt, wartend) = self.0.zahlen(etage);
        Some(crate::etagen::Zahlen { arbeiter: arbeiter as u32, beschaeftigt: beschaeftigt as u32, wartend: wartend as u32 })
    }

    fn fluegel_geaendert(&self, id: &str, fluegel: u32) {
        self.0.tische_setzen(id, crate::etagen::arbeitsplaetze(fluegel));
    }
}

/// Meldungen der Arbeiter (wartet, fertig, PR gemergt …) gehen an den Team-Webhook, gedrosselt:
/// dieselbe Meldung eines Arbeiters höchstens einmal pro Minute, insgesamt höchstens eine alle 3 s.
fn meldungen_an_webhook(o: &OfficeRef, a: &Agenten) {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    let schwach = Arc::downgrade(o);
    let laufzeit = tokio::runtime::Handle::current();
    /// Wann jede Meldung zuletzt ging, und wann überhaupt zuletzt eine.
    type Zuletzt = (HashMap<String, Instant>, Option<Instant>);
    let zuletzt: Arc<Mutex<Zuletzt>> = Arc::default();
    a.benachrichtigung_setzen(Arc::new(move |m: ::agenten::Meldung| {
        let Some(o) = schwach.upgrade() else { return };
        if o.einstellungen.d.lock().unwrap().webhook_url.is_none() {
            return;
        }
        let schluessel = format!("{}|{}|{:?}", m.etage, m.arbeiter_name, m.art);
        {
            let mut z = zuletzt.lock().unwrap();
            let jetzt = Instant::now();
            if z.0.get(&schluessel).is_some_and(|t| jetzt.duration_since(*t) < Duration::from_secs(60)) || z.1.is_some_and(|t| jetzt.duration_since(t) < Duration::from_secs(3)) {
                return;
            }
            z.0.retain(|_, t| jetzt.duration_since(*t) < Duration::from_secs(600));
            z.0.insert(schluessel, jetzt);
            z.1 = Some(jetzt);
        }
        let etage = o.etagen.get(&m.etage).map(|e| e.name).unwrap_or(m.etage);
        let text = format!("🏢 {etage}: {}", m.text);
        laufzeit.spawn(async move {
            if let Err(f) = crate::webhook::senden(&o, &text).await {
                tracing::warn!("Webhook: {f}");
            }
        });
    }));
}

/// Ereignisse von `agenten` an die Browser verteilen.
pub fn verbinden(o: &OfficeRef, a: Agenten) {
    let mut rx = a.ereignisse();
    o.anbinden(Arc::new(AgentenAnbindung(a.clone())));
    // Arbeiter eines Kontos laufen mit dessen eigenen Anmeldungen (leerer Wert = Variable entfernen).
    let schwach = Arc::downgrade(o);
    a.konto_umgebung_setzen(Arc::new(move |konto: &str| schwach.upgrade().map(|o| crate::ws::anmeldungen::umgebung(&o, konto, None)).unwrap_or_default()));
    meldungen_an_webhook(o, &a);
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
