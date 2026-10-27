//! HTTP: die Routen-Tabelle. Jede Route sagt, ob sie öffentlich ist oder eine Sitzung braucht.
//! Neue Routen kommen als eigenes Modul mit einer `routen()`-Funktion und werden in [`tabelle`] eingetragen.

pub mod anmeldung;
pub mod buecher;
pub mod connector;
pub mod dateien;
pub mod relay;
pub mod statisch;
pub mod suche;

use std::net::{IpAddr, SocketAddr};

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;

use crate::anmeldung::Sitzung;
use crate::office::OfficeRef;

/// Wer eine Route aufrufen darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zugriff {
    /// Jeder (Anmeldeseite, Gesundheit, die Dateien des Clients).
    Oeffentlich,
    /// Nur mit gültiger Sitzung (sonst 401).
    Sitzung,
}

/// Eine Zeile der Routen-Tabelle.
pub struct Route {
    pub pfad: &'static str,
    pub zugriff: Zugriff,
    pub methoden: MethodRouter<OfficeRef>,
}

pub fn route(pfad: &'static str, zugriff: Zugriff, methoden: MethodRouter<OfficeRef>) -> Route {
    Route {
        pfad,
        zugriff,
        methoden,
    }
}

/// Alle Routen des Servers.
pub fn tabelle() -> Vec<Route> {
    let mut t = anmeldung::routen();
    t.extend(connector::routen());
    t.extend(dateien::routen());
    t.extend(buecher::routen());
    t.extend(suche::routen());
    t.push(route(
        "/ws",
        Zugriff::Oeffentlich,
        axum::routing::get(crate::ws::verbindung::upgrade),
    )); // prüft die Sitzung selbst (401)
    t
}

/// Baut den Router: Tabelle, angebundene Router (mit Sitzung) und zuletzt der Client aus `dist/`.
pub fn router(o: OfficeRef, angebunden: Vec<Router>) -> Router {
    let mut r: Router<OfficeRef> = Router::new();
    for z in tabelle() {
        let m = match z.zugriff {
            Zugriff::Oeffentlich => z.methoden,
            Zugriff::Sitzung => z
                .methoden
                .route_layer(middleware::from_fn_with_state(o.clone(), sitzung_noetig)),
        };
        r = r.route(z.pfad, m);
    }
    let mut r: Router = r.with_state(o.clone());
    // Angebundene Router (agenten, medien): immer mit Sitzung, dazu der Anfrager.
    for a in angebunden {
        let a = a
            .route_layer(middleware::from_fn_with_state(
                o.clone(),
                crate::anbindung::anfrager,
            ))
            .route_layer(middleware::from_fn_with_state(o.clone(), sitzung_noetig));
        r = r.merge(a);
    }
    r.fallback_service(statisch::client(&o.konfig.client))
        // Ganz außen: Anfragen über einen Dienste-Tunnel gehen an den Server des Arbeiters.
        .layer(middleware::from_fn_with_state(
            o.clone(),
            relay::weiterleiten,
        ))
}

/// Die Sitzung zu einer Anfrage, falls gültig.
pub fn sitzung(o: &OfficeRef, headers: &HeaderMap) -> Option<Sitzung> {
    let cookies = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    o.zugang.aus_cookies(cookies, host)
}

/// Middleware: ohne Sitzung 401, mit Sitzung liegt sie in den Extensions.
pub async fn sitzung_noetig(State(o): State<OfficeRef>, mut req: Request, next: Next) -> Response {
    match sitzung(&o, req.headers()) {
        Some(s) => {
            req.extensions_mut().insert(s);
            next.run(req).await
        }
        None => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "fehler": "Bitte zuerst anmelden" })),
        )
            .into_response(),
    }
}

/// Die Adresse der TCP-Verbindung, falls bekannt (fehlt z. B. in Tests ohne echten Socket).
pub struct Adresse(pub Option<SocketAddr>);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for Adresse {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Adresse(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0),
        ))
    }
}

/// Die IP des Aufrufers (hinter einem Proxy aus X-Forwarded-For).
pub fn client_ip(o: &OfficeRef, headers: &HeaderMap, verbindung: Option<SocketAddr>) -> IpAddr {
    if o.konfig.proxy_vertrauen
        && let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|v| v.trim().parse().ok())
    {
        return ip;
    }
    verbindung
        .map(|c| c.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

/// Kommt ein POST vom Office selbst? (Origin passt zum Host; ohne Origin nur von Programmen wie dem Connector,
/// die keinen Browser-Cookie fremder Seiten mitschicken können – die Sitzung ist trotzdem nötig.)
pub fn gleicher_ursprung(o: &OfficeRef, headers: &HeaderMap) -> bool {
    let host = o
        .konfig
        .proxy_vertrauen
        .then(|| {
            headers
                .get("x-forwarded-host")
                .and_then(|v| v.to_str().ok())
        })
        .flatten()
        .or_else(|| headers.get(header::HOST).and_then(|v| v.to_str().ok()));
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        None => headers.get("sec-fetch-site").is_none(),
        Some(origin) => origin.split_once("://").map(|(_, rest)| rest) == host,
    }
}

/// Kam die Anfrage über https (direkt oder laut Proxy)?
pub fn sicher(o: &OfficeRef, headers: &HeaderMap) -> bool {
    o.konfig.proxy_vertrauen
        && headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            == Some("https")
}
