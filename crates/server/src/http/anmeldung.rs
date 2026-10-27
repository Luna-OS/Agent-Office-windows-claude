//! An- und Abmelden über HTTP: gemeinsames Passwort, eigenes Konto, Einladungslink, Einmal-Link,
//! Beanspruchen des erzeugten Passworts. Alle Antworten sind JSON, Fehler als `{"fehler": "…"}` auf Deutsch.

use std::net::SocketAddr;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Adresse, Route, Zugriff, client_ip, route, sicher, sitzung};
use crate::anmeldung::Sitzung;
use crate::anmeldung::sitzung::{cookie_loeschen, cookie_name, cookie_setzen};
use crate::office::OfficeRef;

const ZU_VIELE: &str = "Zu viele Versuche. Bitte in ein paar Minuten noch einmal.";

pub fn routen() -> Vec<Route> {
    vec![
        route(
            "/api/gesundheit",
            Zugriff::Oeffentlich,
            get(|| async { Json(json!({ "ok": true })) }),
        ),
        route(
            "/api/health",
            Zugriff::Oeffentlich,
            get(|| async { Json(json!({ "ok": true })) }),
        ),
        route(
            "/api/anmeldung",
            Zugriff::Oeffentlich,
            get(optionen).post(anmelden),
        ),
        route("/api/abmelden", Zugriff::Oeffentlich, post(abmelden)),
        route("/api/beitreten", Zugriff::Oeffentlich, post(beitreten)),
        route("/api/link", Zugriff::Oeffentlich, post(link)),
        route(
            "/api/beanspruchen",
            Zugriff::Oeffentlich,
            get(beanspruchbar).post(beanspruchen),
        ),
        route("/api/ich", Zugriff::Sitzung, get(ich)),
    ]
}

fn fehler(code: StatusCode, text: &str) -> Response {
    (code, Json(json!({ "fehler": text }))).into_response()
}

/// Antwort mit frischem Sitzungs-Cookie.
fn angemeldet(o: &OfficeRef, headers: &HeaderMap, konto: Option<&str>, koerper: Value) -> Response {
    let name = cookie_name(headers.get(header::HOST).and_then(|v| v.to_str().ok()));
    let wert = o.zugang.sitzungen.ausstellen(konto);
    let mut r = Json(koerper).into_response();
    if let Ok(v) = HeaderValue::from_str(&cookie_setzen(&name, &wert, sicher(o, headers))) {
        r.headers_mut().insert(header::SET_COOKIE, v);
    }
    r
}

/// Zählt einen Rate-Versuch und liest den kleinen JSON-Körper.
fn raten<T: for<'a> Deserialize<'a>>(
    o: &OfficeRef,
    headers: &HeaderMap,
    info: Option<SocketAddr>,
    body: &Bytes,
) -> Result<(std::net::IpAddr, T), Box<Response>> {
    let ip = client_ip(o, headers, info);
    if !o.zugang.sitzungen.versuch_erlaubt(ip) {
        return Err(Box::new(fehler(StatusCode::TOO_MANY_REQUESTS, ZU_VIELE)));
    }
    if body.len() > 4096 {
        return Err(Box::new(fehler(StatusCode::PAYLOAD_TOO_LARGE, "Zu groß")));
    }
    serde_json::from_slice(body)
        .map(|t| (ip, t))
        .map_err(|_| Box::new(fehler(StatusCode::BAD_REQUEST, "Ungültige Anfrage")))
}

async fn optionen(State(o): State<OfficeRef>, headers: HeaderMap) -> Json<Value> {
    // Erst die Sitzung prüfen (sperrt selbst die Konten), dann die Konten lesen – nie verschachtelt,
    // sonst sperrt sich der Thread selbst.
    let angemeldet = sitzung(&o, &headers).is_some();
    let (konten, gemeinsam) = {
        let k = o.zugang.konten.lock().unwrap();
        (k.gibt_es_welche(), k.gemeinsames_passwort())
    };
    Json(json!({
        "angemeldet": angemeldet,
        "konten": konten,
        "gemeinsam": gemeinsam,
        "offen": o.zugang.offen(),
    }))
}

#[derive(Deserialize)]
struct Anmelden {
    #[serde(default)]
    name: String,
    #[serde(default)]
    passwort: String,
}

async fn anmelden(
    State(o): State<OfficeRef>,
    headers: HeaderMap,
    Adresse(info): Adresse,
    body: Bytes,
) -> Response {
    let (ip, a) = match raten::<Anmelden>(&o, &headers, info, &body) {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let name = a.name.trim();
    if !name.is_empty() {
        let konto = o
            .zugang
            .konten
            .lock()
            .unwrap()
            .pruefen(name, &a.passwort)
            .map(|k| k.id.clone());
        return match konto {
            Some(id) => {
                o.zugang.sitzungen.erfolg(ip);
                angemeldet(&o, &headers, Some(&id), json!({ "ok": true }))
            }
            None => fehler(StatusCode::UNAUTHORIZED, "Name oder Passwort stimmt nicht"),
        };
    }
    let (gemeinsam, konten) = {
        let k = o.zugang.konten.lock().unwrap();
        (k.gemeinsames_passwort(), k.gibt_es_welche())
    };
    if !gemeinsam {
        return fehler(
            StatusCode::UNAUTHORIZED,
            "Melde dich mit deinem Namen und deinem eigenen Passwort an",
        );
    }
    if o.zugang.offen() || o.zugang.passwort_pruefen(&a.passwort) {
        o.zugang.sitzungen.erfolg(ip);
        return angemeldet(&o, &headers, None, json!({ "ok": true }));
    }
    fehler(
        StatusCode::UNAUTHORIZED,
        if konten {
            "Falsches Passwort. Mit eigenem Konto bitte auch den Namen eingeben."
        } else {
            "Falsches Passwort"
        },
    )
}

async fn abmelden(headers: HeaderMap) -> Response {
    let name = cookie_name(headers.get(header::HOST).and_then(|v| v.to_str().ok()));
    let mut r = Json(json!({ "ok": true })).into_response();
    if let Ok(v) = HeaderValue::from_str(&cookie_loeschen(&name)) {
        r.headers_mut().insert(header::SET_COOKIE, v);
    }
    r
}

#[derive(Deserialize)]
struct Beitreten {
    #[serde(default)]
    token: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    passwort: String,
    #[serde(default)]
    nur_ansehen: bool,
}

async fn beitreten(
    State(o): State<OfficeRef>,
    headers: HeaderMap,
    Adresse(info): Adresse,
    body: Bytes,
) -> Response {
    let (ip, b) = match raten::<Beitreten>(&o, &headers, info, &body) {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let einladung = o
        .zugang
        .konten
        .lock()
        .unwrap()
        .einladung_finden(&b.token)
        .cloned();
    let Some(e) = einladung else {
        return fehler(
            StatusCode::GONE,
            "Dieser Einladungslink ist abgelaufen oder wurde schon benutzt. Bitte um einen neuen.",
        );
    };
    o.zugang.sitzungen.erfolg(ip);
    if b.nur_ansehen {
        let projekt = o
            .etagen
            .erste()
            .map(|e| e.name)
            .unwrap_or_else(|| "OpenAgenticOffice".into());
        return Json(
            json!({ "name": e.name, "rolle": e.rolle, "von": e.erstellt_von, "projekt": projekt }),
        )
        .into_response();
    }
    let r = o
        .zugang
        .konten
        .lock()
        .unwrap()
        .beitreten(&b.token, &b.name, &b.passwort);
    match r {
        Ok(k) => {
            tracing::info!(
                "{} ist mit einer Einladung von {} beigetreten",
                k.name,
                k.erstellt_von
            );
            crate::ws::konten::konten_an_admins(&o);
            angemeldet(
                &o,
                &headers,
                Some(&k.id),
                json!({ "ok": true, "name": k.name }),
            )
        }
        Err(f) => fehler(StatusCode::BAD_REQUEST, &f),
    }
}

#[derive(Deserialize)]
struct Link {
    #[serde(default)]
    schluessel: String,
}

async fn link(
    State(o): State<OfficeRef>,
    headers: HeaderMap,
    Adresse(info): Adresse,
    body: Bytes,
) -> Response {
    let (ip, l) = match raten::<Link>(&o, &headers, info, &body) {
        Ok(x) => x,
        Err(r) => return *r,
    };
    match o
        .zugang
        .sitzungen
        .link_einloesen(&l.schluessel)
        .filter(|s| o.zugang.gueltig(s))
    {
        Some(s) => {
            o.zugang.sitzungen.erfolg(ip);
            angemeldet(&o, &headers, s.konto.as_deref(), json!({ "ok": true }))
        }
        None => fehler(
            StatusCode::GONE,
            "Dieser Anmeldelink wurde schon benutzt oder ist zu alt. Melde dich mit deinem Passwort an oder öffne das Office noch einmal aus der App.",
        ),
    }
}

async fn beanspruchbar(State(o): State<OfficeRef>) -> Json<Value> {
    Json(json!({ "beanspruchbar": o.zugang.beanspruchbar() }))
}

#[derive(Deserialize)]
struct Beanspruchen {
    #[serde(default)]
    token: String,
}

async fn beanspruchen(
    State(o): State<OfficeRef>,
    headers: HeaderMap,
    Adresse(info): Adresse,
    body: Bytes,
) -> Response {
    let (ip, b) = match raten::<Beanspruchen>(&o, &headers, info, &body) {
        Ok(x) => x,
        Err(r) => return *r,
    };
    match o.zugang.beanspruchen(&b.token) {
        Ok(passwort) => {
            o.zugang.sitzungen.erfolg(ip);
            tracing::info!(
                "Das Office-Passwort wurde beansprucht – es wird nicht noch einmal gezeigt"
            );
            angemeldet(&o, &headers, None, json!({ "passwort": passwort }))
        }
        Err(f) => fehler(StatusCode::GONE, f),
    }
}

async fn ich(State(o): State<OfficeRef>, Extension(s): Extension<Sitzung>) -> Json<Value> {
    Json(json!({ "ok": true, "ich": o.zugang.ich(&s) }))
}
