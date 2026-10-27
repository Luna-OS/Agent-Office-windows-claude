//! Was der Connector (Desktop-App) vom Office will, über einfaches HTTP: die eigenen Anmeldungen,
//! Einladungslinks und einen Einmal-Link, der den normalen Browser als dich anmeldet.
//! POSTs nur vom Office selbst (gleicher Origin), damit fremde Seiten mit deinem Cookie nichts anlegen.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use protokoll::konten::{AnmeldeArt, Rolle};
use serde::Deserialize;
use serde_json::json;

use super::{Route, Zugriff, gleicher_ursprung, route};
use crate::anmeldung::Sitzung;
use crate::anmeldungen::ablauf;
use crate::office::OfficeRef;
use crate::ws::anmeldungen::{darf_office, umgebung};

const GEMEINSAM: &str = "Mit dem gemeinsamen Office-Passwort laufen die Arbeiter mit den Anmeldungen des Office. Leg dir ein eigenes Konto an, um dein eigenes Claude und GitHub zu nutzen.";

pub fn routen() -> Vec<Route> {
    vec![
        route("/api/link/neu", Zugriff::Sitzung, post(link_neu)),
        route(
            "/api/anmeldungen",
            Zugriff::Sitzung,
            get(anmeldungen).post(anmeldung_tun),
        ),
        route("/api/konten/einladen", Zugriff::Sitzung, post(einladen)),
    ]
}

fn fehler(code: StatusCode, text: &str) -> Response {
    (code, Json(json!({ "fehler": text }))).into_response()
}

fn lesen<T: for<'a> Deserialize<'a>>(
    o: &OfficeRef,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<T, Box<Response>> {
    if !gleicher_ursprung(o, headers) {
        return Err(Box::new(fehler(StatusCode::FORBIDDEN, "Nicht erlaubt")));
    }
    let body: &[u8] = if body.is_empty() { b"{}" } else { body };
    if body.len() > 8192 {
        return Err(Box::new(fehler(StatusCode::PAYLOAD_TOO_LARGE, "Zu groß")));
    }
    serde_json::from_slice(body)
        .map_err(|_| Box::new(fehler(StatusCode::BAD_REQUEST, "Ungültige Anfrage")))
}

async fn link_neu(
    State(o): State<OfficeRef>,
    Extension(s): Extension<Sitzung>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(r) = lesen::<serde_json::Value>(&o, &headers, &body) {
        return *r;
    }
    Json(json!({ "schluessel": o.zugang.sitzungen.link_schluessel(Some(&s)) })).into_response()
}

fn stand(o: &OfficeRef, konto: &str) -> serde_json::Value {
    let (c, g) = darf_office(o, konto);
    json!({ "stand": o.anmeldungen.stand(konto, c, g) })
}

async fn anmeldungen(State(o): State<OfficeRef>, Extension(s): Extension<Sitzung>) -> Response {
    let Some(konto) = s.konto else {
        return fehler(StatusCode::CONFLICT, GEMEINSAM);
    };
    let _ = o.anmeldungen.geaendert.send(format!("nachsehen:{konto}"));
    Json(stand(&o, &konto)).into_response()
}

#[derive(Deserialize)]
struct Tun {
    tun: String,
    #[serde(default)]
    art: Option<AnmeldeArt>,
    #[serde(default)]
    code: String,
    #[serde(default)]
    token: String,
}

async fn anmeldung_tun(
    State(o): State<OfficeRef>,
    Extension(s): Extension<Sitzung>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let t: Tun = match lesen(&o, &headers, &body) {
        Ok(t) => t,
        Err(r) => return *r,
    };
    let Some(konto) = s.konto else {
        return fehler(StatusCode::CONFLICT, GEMEINSAM);
    };
    let art = t.art.unwrap_or(AnmeldeArt::Claude);
    let env = umgebung(&o, &konto, None);
    let a = o.anmeldungen.clone();
    let ergebnis = match t.tun.as_str() {
        "starten" if art == AnmeldeArt::Claude => {
            crate::anmeldungen::claude::claude_starten(&a, &konto, env)
        }
        "starten" => ablauf::github_starten(&a, &konto, env),
        "code" => crate::anmeldungen::claude::claude_code(&a, &konto, &t.code),
        "token" => ablauf::token(&a, &konto, art, &t.token, env).await,
        "office" => {
            let (c, g) = darf_office(&o, &konto);
            if (art == AnmeldeArt::Claude && c) || (art == AnmeldeArt::Github && g) {
                ablauf::office(&a, &konto, art);
                Ok(())
            } else {
                Err("Die Anmeldung des Office-Rechners darfst du nicht nutzen".into())
            }
        }
        "abbrechen" => {
            ablauf::abbrechen(&a, &konto, art);
            Ok(())
        }
        "abmelden" => {
            ablauf::abmelden(&a, &konto, art, env).await;
            Ok(())
        }
        _ => return fehler(StatusCode::BAD_REQUEST, "Unbekannte Aktion"),
    };
    let mut antwort = stand(&o, &konto);
    match ergebnis {
        Ok(()) => Json(antwort).into_response(),
        Err(f) => {
            antwort["fehler"] = json!(f);
            (StatusCode::BAD_REQUEST, Json(antwort)).into_response()
        }
    }
}

#[derive(Deserialize)]
struct Einladen {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    rolle: Option<Rolle>,
    #[serde(default)]
    von: Option<String>,
}

async fn einladen(
    State(o): State<OfficeRef>,
    Extension(s): Extension<Sitzung>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let e: Einladen = match lesen(&o, &headers, &body) {
        Ok(e) => e,
        Err(r) => return *r,
    };
    if !o.zugang.ich(&s).admin {
        return fehler(StatusCode::FORBIDDEN, "Nur Admins können Leute einladen");
    }
    let von = o
        .zugang
        .konto_name(&s)
        .or(e.von.map(|v| v.chars().take(24).collect()))
        .unwrap_or_else(|| "die App".into());
    let r = o.zugang.konten.lock().unwrap().einladen(
        &von,
        e.rolle.unwrap_or(Rolle::Mitglied),
        e.name.as_deref(),
    );
    match r {
        Ok(einladung) => {
            crate::ws::konten::konten_an_admins(&o);
            Json(json!({ "einladung": einladung, "adressen": crate::ws::konten::adressen(&o) }))
                .into_response()
        }
        Err(f) => fehler(StatusCode::BAD_REQUEST, &f),
    }
}
