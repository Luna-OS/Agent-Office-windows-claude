//! Adapter für `medien` (lyra): Bildschirm teilen über HTTP, Voice-Signalisierung, Jukebox, Dach-Leinwand.

use ::medien::server::{Absender, Medien, MedienEinstellungen, PFAD, Ziel};
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use protokoll::praesenz::Stufe;
use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::broadcast::error::RecvError;

use super::Anfrager;
use crate::konfig::Konfig;
use crate::office::{DACH, OfficeRef};
use crate::ws::{self, Zukunft};

pub fn starten(k: &Konfig) -> Medien {
    Medien::neu(MedienEinstellungen {
        datenordner: k.daten.clone(),
        ice: Vec::new(),
    })
}

fn absender(o: &OfficeRef, von: &str) -> Absender {
    let p = o.person(von);
    let etage = p.as_ref().and_then(|p| p.etage.clone());
    Absender {
        verbindung: von.into(),
        name: p.map(|p| p.name).unwrap_or_default(),
        auf_dach: etage.as_deref() == Some(DACH),
        etage,
        konto: o.sitzung_von(von).and_then(|s| s.konto),
    }
}

pub struct Domaene(pub Medien);

impl ws::Domaene for Domaene {
    fn name(&self) -> &'static str {
        "medien"
    }

    fn zustaendig(&self, n: &ClientNachricht) -> bool {
        matches!(n, ClientNachricht::Medien(_) | ClientNachricht::Dach(_))
    }

    fn nachricht<'a>(&'a self, o: &'a OfficeRef, von: &'a str, n: ClientNachricht) -> Zukunft<'a> {
        let a = absender(o, von);
        let r = match n {
            ClientNachricht::Medien(m) => self.0.nachricht_medien(&a, m),
            ClientNachricht::Dach(d) => self.0.nachricht_dach(&a, d),
            _ => Ok(()),
        };
        if let Err(f) = r {
            o.hinweis(von, f, Stufe::Warnung);
        }
        ws::fertig()
    }

    fn begruessung(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        self.0.begruessung(&absender(o, von))
    }

    fn etage_betreten(&self, o: &OfficeRef, von: &str) -> Vec<ServerNachricht> {
        self.0.etage_betreten(&absender(o, von))
    }

    fn weg(&self, _o: &OfficeRef, von: &str, _etage: Option<&str>) {
        self.0.verbindung_weg(von);
    }
}

/// Macht aus dem [`Anfrager`] den Absender, den `medien` in den Extensions erwartet.
async fn absender_einsetzen(mut req: Request, next: Next) -> Response {
    if let Some(a) = req.extensions().get::<Anfrager>().cloned() {
        let etage = a.person.as_ref().and_then(|p| p.etage.clone());
        req.extensions_mut().insert(Absender {
            verbindung: a.verbindung.unwrap_or_default(),
            name: a.person.map(|p| p.name).unwrap_or_default(),
            auf_dach: etage.as_deref() == Some(DACH),
            etage,
            konto: a.sitzung.konto,
        });
    }
    next.run(req).await
}

pub fn router(m: &Medien) -> axum::Router {
    axum::Router::new()
        .nest(PFAD, m.router())
        .layer(middleware::from_fn(absender_einsetzen))
}

/// Ereignisse von `medien` an die Browser verteilen.
pub fn verbinden(o: &OfficeRef, m: Medien) {
    let mut rx = m.ereignisse();
    let o = o.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(e) => match e.ziel {
                    Ziel::Etage(etage) => o.an_etage(Some(&etage), &e.nachricht, None),
                    Ziel::Verbindung(v) => o.an(&v, &e.nachricht),
                    Ziel::Alle => o.an_alle(&e.nachricht, None),
                    Ziel::Dach => o.an_etage(Some(DACH), &e.nachricht, None),
                },
                Err(RecvError::Lagged(n)) => tracing::warn!("{n} Medien-Ereignisse verpasst"),
                Err(RecvError::Closed) => break,
            }
        }
    });
}
