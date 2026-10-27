//! Angebundene Bibliotheken: `agenten` (Arbeiter, Board, GitHub, Agent-Bus) und `medien`
//! (Bildschirm teilen, Voice, Jukebox, Dach-Leinwand). Jede hat hier einen Adapter mit
//! WebSocket-Domäne, HTTP-Router und dem Weiterleiten ihrer Ereignisse an die Browser.

pub mod agenten;
pub mod medien;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use protokoll::praesenz::Person;

use crate::anmeldung::Sitzung;
use crate::konfig::Konfig;
use crate::office::OfficeRef;
use crate::ws::Domaene;

/// Was beim Start der Anbindungen entsteht.
pub struct Gestartet {
    pub domaenen: Vec<Box<dyn Domaene>>,
    pub routen: Vec<axum::Router>,
    agenten: Option<::agenten::Agenten>,
    medien: Option<::medien::server::Medien>,
}

/// Wer eine angebundene HTTP-Route aufruft: die Sitzung und (falls angegeben und passend)
/// die WebSocket-Verbindung samt Person. Die Verbindung kommt aus dem Header
/// `x-office-verbindung` oder dem Query-Parameter `verbindung` (für `<img src>`).
#[derive(Debug, Clone)]
pub struct Anfrager {
    pub sitzung: Sitzung,
    pub verbindung: Option<String>,
    pub person: Option<Person>,
}

/// Startet agenten und medien (vor dem Office, damit ihre Domänen ins Register kommen).
pub async fn starten(k: &Konfig) -> anyhow::Result<Gestartet> {
    let agenten = agenten::starten(k).await?;
    let medien = medien::starten(k);
    let mut domaenen: Vec<Box<dyn Domaene>> = Vec::new();
    let mut routen = Vec::new();
    domaenen.push(Box::new(agenten::Domaene(agenten.clone())));
    routen.push(agenten::router(&agenten));
    domaenen.push(Box::new(medien::Domaene(medien.clone())));
    routen.push(medien::router(&medien));
    Ok(Gestartet {
        domaenen,
        routen,
        agenten: Some(agenten),
        medien: Some(medien),
    })
}

/// Nach dem Bau des Office: Ereignisse weiterleiten und als [`crate::office::Anbindung`] eintragen.
pub fn verbinden(o: &OfficeRef, g: &mut Gestartet) {
    if let Some(a) = g.agenten.take() {
        agenten::verbinden(o, a);
    }
    if let Some(m) = g.medien.take() {
        medien::verbinden(o, m);
    }
}

pub async fn beenden(o: &OfficeRef) {
    if let Some(a) = o.agenten.get() {
        a.beenden().await;
    }
}

/// Middleware für angebundene Router: legt den [`Anfrager`] in die Extensions (Sitzung ist schon geprüft).
pub async fn anfrager(
    axum::extract::State(o): axum::extract::State<OfficeRef>,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(sitzung) = req.extensions().get::<Sitzung>().cloned() else {
        return next.run(req).await;
    };
    let aus_query = req.uri().query().and_then(|q| {
        q.split('&')
            .find_map(|t| t.strip_prefix("verbindung="))
            .map(String::from)
    });
    let verbindung = req
        .headers()
        .get("x-office-verbindung")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or(aus_query);
    // Nur eine Verbindung derselben Sitzung (gleiches Konto) zählt, sonst könnte man sich als andere ausgeben.
    let passend = verbindung.filter(|v| o.sitzung_von(v).is_some_and(|s| s.konto == sitzung.konto));
    let person = passend.as_deref().and_then(|v| o.person(v));
    req.extensions_mut().insert(Anfrager {
        sitzung,
        verbindung: passend,
        person,
    });
    next.run(req).await
}
