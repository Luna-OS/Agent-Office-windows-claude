//! Das Office eines Gastgebers am Vermittler: eine WebSocket-Verbindung (`/vermittler/knoten`),
//! über die alle durchgereichten Browser-Verbindungen laufen.
//!
//! Steuernachrichten sind JSON-Text (`t` sagt, welche), Daten sind Binär-Nachrichten: vier Bytes
//! Verbindungsnummer (big-endian), dann die Bytes. Das Office meldet sich mit `hallo` an (Version,
//! Lobby-Passwort, Name) und bekommt `willkommen` (ob es jetzt Gastgeber ist) oder `abgelehnt`.
//! Danach kündigt der Vermittler jede neue Verbindung mit `auf` an; `zu` beendet eine, von beiden Seiten.

use crate::lobby::{Anmeldung, Ausgang, Knoten, Lobby};
use crate::vorspann::MitVorspann;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

/// Die Version des Protokolls zwischen Office und Vermittler.
pub const PROTOKOLL: u32 = 1;
/// So lange darf das `hallo` auf sich warten lassen.
const HALLO_ZEIT: Duration = Duration::from_secs(10);
/// Kommt so lange nichts (auch keine Antwort auf ein Ping), ist das Office weg.
const STILLE_MAX: Duration = Duration::from_secs(60);
const PING_ALLE: Duration = Duration::from_secs(20);
/// So lange darf eine langsame Browser-Verbindung den Gastgeber aufhalten, dann wird sie beendet.
const BROWSER_WARTEN: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum VomKnoten {
    Hallo {
        version: u32,
        passwort: String,
        name: String,
    },
    Zu {
        strom: u32,
    },
}

#[derive(Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ZumKnoten<'a> {
    Willkommen {
        version: u32,
        aktiv: bool,
        gastgeber: &'a str,
    },
    Abgelehnt {
        grund: &'a str,
    },
    /// Der bisherige Gastgeber ist gegangen: Jetzt ist dieses Office dran.
    Aktiv,
    /// Eine neue Verbindung von `von` (der Adresse des Browsers).
    Auf {
        strom: u32,
        von: &'a str,
    },
    Zu {
        strom: u32,
    },
}

pub fn text(n: &ZumKnoten<'_>) -> String {
    serde_json::to_string(n).unwrap_or_default()
}

/// Ein Name, wie ihn andere sehen: ohne Steuerzeichen, höchstens 40 Zeichen.
pub fn name_bereinigen(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(40).collect();
    let n = n.trim();
    if n.is_empty() {
        "Gastgeber".into()
    } else {
        n.to_string()
    }
}

/// Bedient die Verbindung eines Office, bis sie endet.
pub async fn bedienen(lobby: Arc<Lobby>, tcp: TcpStream, vorspann: Vec<u8>) {
    let Ok(ws) = tokio_tungstenite::accept_async(MitVorspann::neu(vorspann, tcp)).await else {
        return;
    };
    let (mut schreiben, mut lesen) = ws.split();
    let hallo = match timeout(HALLO_ZEIT, lesen.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<VomKnoten>(t.as_str()).ok(),
        _ => None,
    };
    let ablehnen = |grund: &str| Message::text(text(&ZumKnoten::Abgelehnt { grund }));
    let Some(VomKnoten::Hallo {
        version,
        passwort,
        name,
    }) = hallo
    else {
        return;
    };
    if version != PROTOKOLL {
        let _ = schreiben
            .send(ablehnen(
                "Dieser Vermittler spricht eine andere Version. Bitte App und Umbrel-App auf den neuesten Stand bringen.",
            ))
            .await;
        return;
    }
    if !lobby.passwort_stimmt(&passwort) {
        // Etwas warten, damit niemand schnell viele Passwörter durchprobiert.
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = schreiben.send(ablehnen("Falsches Lobby-Passwort.")).await;
        return;
    }
    let (tx, mut rx) = mpsc::channel::<Ausgang>(256);
    let Anmeldung::Da {
        knoten,
        aktiv,
        gastgeber,
    } = lobby.anmelden(&name_bereinigen(&name), tx)
    else {
        let _ = schreiben
            .send(ablehnen(
                "Es haben sich schon zu viele Gastgeber angemeldet.",
            ))
            .await;
        return;
    };
    let willkommen = text(&ZumKnoten::Willkommen {
        version: PROTOKOLL,
        aktiv,
        gastgeber: &gastgeber,
    });
    if schreiben.send(Message::text(willkommen)).await.is_err() {
        lobby.abmelden(knoten.id);
        return;
    }

    let schreiber = tokio::spawn(async move {
        let mut ping = tokio::time::interval(PING_ALLE);
        loop {
            let nachricht = tokio::select! {
                a = rx.recv() => match a {
                    Some(Ausgang::Text(t)) => Message::text(t),
                    Some(Ausgang::Daten(id, b)) => {
                        let mut v = Vec::with_capacity(4 + b.len());
                        v.extend_from_slice(&id.to_be_bytes());
                        v.extend_from_slice(&b);
                        Message::binary(v)
                    }
                    None => break,
                },
                _ = ping.tick() => Message::Ping(Bytes::new()),
            };
            if schreiben.send(nachricht).await.is_err() {
                break;
            }
        }
        let _ = schreiben.close().await;
    });

    while let Ok(Some(Ok(m))) = timeout(STILLE_MAX, lesen.next()).await {
        match m {
            Message::Binary(b) if b.len() >= 4 => {
                let id = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                weiterreichen(&knoten, id, b.slice(4..)).await;
            }
            Message::Text(t) => {
                if let Ok(VomKnoten::Zu { strom }) = serde_json::from_str(t.as_str())
                    && let Some(zum_browser) = knoten.strom_weg(strom)
                {
                    let _ = zum_browser.send(None).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    schreiber.abort();
    if let Some(neu) = lobby.abmelden(knoten.id) {
        let _ = neu
            .ausgang
            .send(Ausgang::Text(text(&ZumKnoten::Aktiv)))
            .await;
    }
}

/// Daten vom Gastgeber an die Browser-Verbindung `id`. Kommt sie nicht hinterher, wird sie beendet.
async fn weiterreichen(knoten: &Knoten, id: u32, daten: Bytes) {
    let Some(zum_browser) = knoten.strom(id) else {
        return;
    };
    let geschafft = matches!(
        timeout(BROWSER_WARTEN, zum_browser.send(Some(daten))).await,
        Ok(Ok(()))
    );
    if !geschafft {
        knoten.strom_weg(id);
        let _ = knoten
            .ausgang
            .try_send(Ausgang::Text(text(&ZumKnoten::Zu { strom: id })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen_werden_bereinigt() {
        assert_eq!(name_bereinigen("  Luna \u{7}"), "Luna");
        assert_eq!(name_bereinigen(""), "Gastgeber");
        assert_eq!(name_bereinigen(&"x".repeat(100)).len(), 40);
    }

    #[test]
    fn nachrichten_sehen_aus_wie_beschrieben() {
        assert_eq!(
            text(&ZumKnoten::Auf {
                strom: 7,
                von: "1.2.3.4"
            }),
            r#"{"t":"auf","strom":7,"von":"1.2.3.4"}"#
        );
        assert_eq!(text(&ZumKnoten::Aktiv), r#"{"t":"aktiv"}"#);
    }
}
