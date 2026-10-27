//! Die HTTP-API für die CLI `office-agenten` auf dem Hook-Server: wer ist da, Nachrichten,
//! eigener Status, Aufgabenboard und einen weiteren Arbeiter anlegen.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::arbeiter::status_wort;
use crate::board::{Aenderung, Neu, status_lesen};
use crate::server::{ausweis, fehler};
use crate::{Agenten, hooks, kommunikation, start};

/// Wer fragt: (Etage, Arbeiter-ID, Name).
fn wer_fragt(ag: &Agenten, kopf: &HeaderMap) -> Result<(String, String, String), Box<Response>> {
    let (id, token) = ausweis(kopf);
    let Some(etage) = hooks::ausweisen(ag, &id, &token) else {
        return Err(Box::new(fehler(
            StatusCode::UNAUTHORIZED,
            "Nicht ausgewiesen: OFFICE_ARBEITER_ID als X-Office-Arbeiter und OFFICE_HOOK_TOKEN als Bearer-Token mitschicken",
        )));
    };
    let name = ag.innen.zustand().etagen.get(&etage).and_then(|e| e.arbeiter.get(&id)).map(|a| a.info.name.clone());
    match name {
        Some(name) => Ok((etage, id, name)),
        None => Err(Box::new(fehler(StatusCode::GONE, "Diesen Arbeiter gibt es nicht mehr"))),
    }
}

macro_rules! ausweisen {
    ($ag:expr, $kopf:expr) => {
        match wer_fragt(&$ag, &$kopf) {
            Ok(w) => w,
            Err(r) => return *r,
        }
    };
}

pub async fn wer(State(ag): State<Agenten>, kopf: HeaderMap) -> Response {
    let (etage, id, _) = ausweisen!(ag, kopf);
    let z = ag.innen.zustand();
    let Some(e) = z.etagen.get(&etage) else { return fehler(StatusCode::GONE, "Diese Etage gibt es nicht mehr") };
    let arbeiter: Vec<_> = e
        .arbeiter
        .values()
        .map(|a| {
            let mut eintrag = serde_json::to_value(a.feed_eintrag()).unwrap_or_default();
            eintrag["status_text"] = json!(status_wort(a.info.status));
            eintrag["tisch"] = json!(a.info.tisch);
            eintrag["du"] = json!(a.info.id == id);
            eintrag
        })
        .collect();
    Json(json!({ "etage": etage, "du": id, "arbeiter": arbeiter })).into_response()
}

#[derive(Deserialize)]
pub struct SendenAnfrage {
    #[serde(default)]
    an: Option<String>,
    text: String,
}

pub async fn senden(State(ag): State<Agenten>, kopf: HeaderMap, Json(a): Json<SendenAnfrage>) -> Response {
    let (etage, id, name) = ausweisen!(ag, kopf);
    if let Some(an) = crate::bus::empfaenger(a.an.as_deref()) {
        let z = ag.innen.zustand();
        if z.etagen.get(&etage).and_then(|e| e.finden(&an)).is_some_and(|x| x.info.id == id) {
            return fehler(StatusCode::BAD_REQUEST, "Das bist du selbst");
        }
    }
    match kommunikation::senden(&ag, &etage, &name, Some(&id), a.an.as_deref(), &a.text) {
        Ok(g) => {
            let hinweis = match (&g.nachricht.an, g.empfaenger.is_empty()) {
                (Some(an), true) => Some(format!("Kein Agent namens „{an}“ auf der Etage – die Nachricht steht im Verlauf für die Menschen.")),
                _ => None,
            };
            Json(json!({ "ok": true, "id": g.nachricht.id, "empfaenger": g.empfaenger, "hinweis": hinweis })).into_response()
        }
        Err(f) => fehler(StatusCode::BAD_REQUEST, f),
    }
}

pub async fn posteingang(State(ag): State<Agenten>, kopf: HeaderMap) -> Response {
    let (etage, id, _) = ausweisen!(ag, kopf);
    Json(kommunikation::posteingang(&ag, &etage, &id)).into_response()
}

pub async fn verlauf(State(ag): State<Agenten>, kopf: HeaderMap) -> Response {
    let (etage, id, name) = ausweisen!(ag, kopf);
    let z = ag.innen.zustand();
    Json(z.etagen.get(&etage).map(|e| e.bus.verlauf_fuer(&id, &name, 30)).unwrap_or_default()).into_response()
}

#[derive(Deserialize)]
pub struct StatusAnfrage {
    text: String,
}

pub async fn status(State(ag): State<Agenten>, kopf: HeaderMap, Json(a): Json<StatusAnfrage>) -> Response {
    let (etage, id, _) = ausweisen!(ag, kopf);
    match kommunikation::status_melden(&ag, &etage, &id, &a.text) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(f) => fehler(StatusCode::BAD_REQUEST, f),
    }
}

pub async fn aufgaben(State(ag): State<Agenten>, kopf: HeaderMap) -> Response {
    let (etage, _, _) = ausweisen!(ag, kopf);
    let z = ag.innen.zustand();
    Json(z.etagen.get(&etage).map(|e| e.board.board.clone()).unwrap_or_default()).into_response()
}

#[derive(Deserialize)]
pub struct NeuAnfrage {
    titel: String,
    #[serde(default)]
    notiz: Option<String>,
    #[serde(default)]
    besitzer: Option<String>,
    /// Das Office gibt sie selbst einem neuen Arbeiter; `notiz` ist dann der Auftrag.
    #[serde(default)]
    automatisch: bool,
}

pub async fn aufgabe_neu(State(ag): State<Agenten>, kopf: HeaderMap, Json(a): Json<NeuAnfrage>) -> Response {
    let (etage, _, name) = ausweisen!(ag, kopf);
    let neu = if a.automatisch {
        Neu { titel: a.titel, prompt: a.notiz, automatisch: true, ..Default::default() }
    } else {
        Neu { titel: a.titel, notiz: a.notiz, besitzer: a.besitzer, ..Default::default() }
    };
    match kommunikation::aufgabe_neu(&ag, &etage, &name, neu) {
        Ok(a) => Json(a).into_response(),
        Err(f) => fehler(StatusCode::BAD_REQUEST, f),
    }
}

#[derive(Deserialize)]
pub struct AendernAnfrage {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    notiz: Option<String>,
    #[serde(default)]
    besitzer: Option<String>,
    #[serde(default)]
    titel: Option<String>,
}

pub async fn aufgabe_aendern(State(ag): State<Agenten>, kopf: HeaderMap, Path(nr): Path<u64>, Json(a): Json<AendernAnfrage>) -> Response {
    let (etage, _, name) = ausweisen!(ag, kopf);
    let status = match a.status.as_deref() {
        None => None,
        Some(s) => match status_lesen(s) {
            Some(s) => Some(s),
            None => return fehler(StatusCode::BAD_REQUEST, format!("Unbekannter Status „{s}“ (offen, in_arbeit, erledigt, blockiert)")),
        },
    };
    // Wer eine Aufgabe in Arbeit nimmt und keinen Besitzer nennt, nimmt sie selbst.
    let besitzer = a.besitzer.or_else(|| (status == Some(protokoll::aufgaben::AufgabenStatus::InArbeit)).then(|| name.clone()));
    let aenderung = Aenderung { titel: a.titel, status, besitzer, notiz: a.notiz };
    match kommunikation::aufgabe_aendern(&ag, &etage, &name, nr, aenderung) {
        Ok(a) => Json(a).into_response(),
        Err(f) => fehler(StatusCode::BAD_REQUEST, f),
    }
}

/// Nimmt eine Aufgabe vom Board, die noch niemand bearbeitet.
pub async fn aufgabe_weg(State(ag): State<Agenten>, kopf: HeaderMap, Path(nr): Path<u64>) -> Response {
    let (etage, _, _) = ausweisen!(ag, kopf);
    let ergebnis = kommunikation::board_aendern(&ag, &etage, |b| match b.holen(nr) {
        None => Err(format!("Keine Aufgabe #{nr}")),
        Some(a) if a.status == protokoll::aufgaben::AufgabenStatus::InArbeit => Err(format!("Aufgabe #{nr} ist schon in Arbeit")),
        Some(_) => Ok(b.entfernen(nr)),
    });
    match ergebnis.and_then(|r| r) {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(f) => fehler(StatusCode::BAD_REQUEST, f),
    }
}

#[derive(Deserialize)]
pub struct AnlegenAnfrage {
    prompt: String,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    modell: Option<String>,
    #[serde(default)]
    tisch: Option<String>,
}

/// Ein Agent holt sich Verstärkung: ein neuer Arbeiter mit einem Auftrag.
pub async fn anlegen(State(ag): State<Agenten>, kopf: HeaderMap, Json(a): Json<AnlegenAnfrage>) -> Response {
    let (etage, ich, name) = ausweisen!(ag, kopf);
    let konto = ag.innen.zustand().arbeiter_mut(&etage, &ich).and_then(|x| x.info.konto.clone());
    if a.prompt.trim().is_empty() {
        return fehler(StatusCode::BAD_REQUEST, "Sag, was der neue Arbeiter tun soll");
    }
    let wunsch = start::Wunsch { tisch: a.tisch, prompt: Some(a.prompt), provider: a.provider, modell: a.modell, konto, ..Default::default() };
    match start::anlegen(&ag, &etage, &name, wunsch).await {
        Ok(id) => {
            let z = ag.innen.zustand();
            let name = z.etagen.get(&etage).and_then(|e| e.arbeiter.get(&id)).map(|a| a.info.name.clone());
            Json(json!({ "ok": true, "id": id, "name": name })).into_response()
        }
        Err(f) => fehler(StatusCode::BAD_REQUEST, f),
    }
}
