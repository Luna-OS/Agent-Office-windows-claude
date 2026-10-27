//! Eine WebSocket-Verbindung: Anmeldung prüfen, Person anlegen, begrüßen, Nachrichten verteilen, aufräumen.
//!
//! Query-Parameter von `/ws` (alle freiwillig): `etage`, `name`, `farbe`, `haut`, `haare`, `frisur`,
//! `x`, `y`, `z`, `rot_y` (wo man vor dem Neuladen stand), `lite=1`.

use std::collections::HashMap;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use protokoll::praesenz::{self, Aussehen, Ort, Person};
use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::mpsc;

use super::eingabe;
use crate::anmeldung::Sitzung;
use crate::office::{Ausgang, DACH, EmoteEimer, OfficeRef, Verbindung};
use crate::speicher::zufalls_id;

/// Wo man im Aufzug ankommt (Mitte der Kabine, wie im Original).
pub const AUFZUG: Ort = Ort { x: 8.5, y: 0.0, z: -11.8, rot_y: 0.0 };
const MAX_FRAME: usize = 256 * 1024;

pub async fn upgrade(State(o): State<OfficeRef>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>, ws: WebSocketUpgrade) -> Response {
    let cookies = headers.get(header::COOKIE).and_then(|v| v.to_str().ok());
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let Some(sitzung) = o.zugang.aus_cookies(cookies, host) else {
        return (StatusCode::UNAUTHORIZED, "Bitte zuerst anmelden").into_response();
    };
    ws.max_message_size(MAX_FRAME).on_upgrade(move |socket| verbunden(o, socket, sitzung, q))
}

/// Die Ankunfts-Etage: die gewünschte, wenn es sie gibt, sonst die erste.
fn ankunft(o: &OfficeRef, gewuenscht: Option<&str>) -> (Option<String>, bool) {
    match gewuenscht {
        Some(DACH) if o.etagen.anzahl() > 0 => (Some(DACH.into()), true),
        Some(e) if o.etagen.gibt_es(e) => (Some(e.into()), true),
        _ => (o.etagen.erste().map(|e| e.id), false),
    }
}

fn neue_person(o: &OfficeRef, id: &str, sitzung: &Sitzung, q: &HashMap<String, String>) -> Person {
    let (etage, zurueck) = ankunft(o, q.get("etage").map(String::as_str));
    let f = |k: &str| q.get(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
    let ort = match (zurueck, f("x"), f("z")) {
        (true, Some(x), Some(z)) => Ort { x: eingabe::zahl(x, 200.0), y: eingabe::zahl(f("y").unwrap_or(0.0), 100.0), z: eingabe::zahl(z, 200.0), rot_y: f("rot_y").unwrap_or(0.0) },
        _ => AUFZUG,
    };
    let konto_name = o.zugang.konto_name(sitzung);
    let name = konto_name.clone().unwrap_or_else(|| {
        let n = crate::anmeldung::konten::name_saeubern(q.get("name").map(String::as_str).unwrap_or(""));
        if n.is_empty() { format!("Gast {}", &id[..3]) } else { n }
    });
    let u = |k: &str| q.get(k).and_then(|v| v.parse::<u8>().ok());
    let saat = id.bytes().fold(0u32, |h, b| h.wrapping_mul(16777619) ^ b as u32);
    let aussehen = Aussehen {
        haut: u("haut").unwrap_or((saat % 8) as u8),
        haare: u("haare").unwrap_or(((saat >> 8) % 10) as u8),
        frisur: u("frisur").unwrap_or(((saat >> 16) % 7) as u8),
    }
    .bereinigt();
    let farbe = q.get("farbe").filter(|f| eingabe::ist_farbe(f)).cloned().unwrap_or_else(|| "#4f86f7".into());
    Person {
        id: id.into(),
        name,
        farbe,
        aussehen,
        x: ort.x,
        y: ort.y,
        z: ort.z,
        rot_y: ort.rot_y,
        laeuft: false,
        raucht: false,
        golft: false,
        wirft: None,
        sitz: None,
        traegt: None,
        getraenk: None,
        konto: konto_name.is_some(),
        etage,
        tut: None,
        liest: false,
        lite: q.get("lite").is_some_and(|v| v == "1"),
    }
}

async fn verbunden(o: OfficeRef, socket: WebSocket, sitzung: Sitzung, q: HashMap<String, String>) {
    let id = zufalls_id(10);
    let person = neue_person(&o, &id, &sitzung, &q);
    let (tx, mut rx) = mpsc::unbounded_channel::<Ausgang>();
    let (mut senden, mut empfangen) = socket.split();

    if let Some(k) = &sitzung.konto {
        o.zugang.konten.lock().unwrap().gesehen(k);
    }
    o.verbindungen.lock().unwrap().insert(id.clone(), Verbindung { tx: tx.clone(), person: person.clone(), sitzung, emotes: EmoteEimer::default() });
    tracing::info!("{} ist hereingekommen ({id})", person.name);

    o.register.begruessen(&o, &id);
    o.register.etage_betreten(&o, &id);
    o.an_alle(&ServerNachricht::Praesenz(praesenz::Server::PersonDa { person: person.clone() }), Some(&id));
    super::etagen::liste_an_alle(&o);
    super::konten::konten_an_admins(&o);

    let schreiber = tokio::spawn(async move {
        let mut puls = tokio::time::interval(Duration::from_secs(25));
        puls.tick().await;
        loop {
            tokio::select! {
                aus = rx.recv() => match aus {
                    Some(Ausgang::Text(t)) => if senden.send(Message::Text(t)).await.is_err() { break },
                    Some(Ausgang::Schliessen(grund)) => {
                        let _ = senden.send(Message::Close(Some(CloseFrame { code: 4001, reason: grund.into() }))).await;
                        break;
                    }
                    None => break,
                },
                _ = puls.tick() => if senden.send(Message::Ping(Default::default())).await.is_err() { break },
            }
        }
    });

    while let Some(Ok(m)) = empfangen.next().await {
        let roh = match m {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        match serde_json::from_str::<ClientNachricht>(&roh) {
            Ok(n) => {
                if !o.register.verteilen(&o, &id, n).await {
                    o.fehler(&id, "Diese Nachricht versteht der Server noch nicht");
                }
            }
            Err(e) => {
                tracing::debug!("Unlesbare Nachricht von {id}: {e}");
                o.fehler(&id, format!("Unlesbare Nachricht: {e}"));
            }
        }
        if schreiber.is_finished() {
            break;
        }
    }

    let weg = o.verbindungen.lock().unwrap().remove(&id);
    schreiber.abort();
    let etage = weg.as_ref().and_then(|v| v.person.etage.clone());
    o.register.weg(&o, &id, etage.as_deref());
    o.an_alle(&ServerNachricht::Praesenz(praesenz::Server::PersonWeg { id: id.clone() }), None);
    super::etagen::liste_an_alle(&o);
    super::konten::konten_an_admins(&o);
    if let Some(v) = weg {
        tracing::info!("{} ist gegangen ({id})", v.person.name);
    }
}
