//! Was der Vermittler selbst beantwortet: seinen Stand, und eine Seite, solange kein Gastgeber da ist.

use crate::knoten::PROTOKOLL;
use crate::kopf::Kopf;
use crate::lobby::Lobby;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[derive(Serialize)]
struct Stand<'a> {
    dienst: &'a str,
    version: &'a str,
    protokoll: u32,
    /// Wessen Office gerade die Lobby ist, oder `null`.
    gastgeber: Option<String>,
    /// Wie viele Offices als Ersatz-Gastgeber warten.
    ersatz: usize,
}

async fn antworten(tcp: &mut TcpStream, status: &str, typ: &str, koerper: &[u8], kopf_nur: bool) {
    let mut a = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {typ}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        koerper.len()
    )
    .into_bytes();
    if !kopf_nur {
        a.extend_from_slice(koerper);
    }
    let _ = tcp.write_all(&a).await;
    let _ = tcp.shutdown().await;
}

/// `GET /vermittler/stand`: wer Gastgeber ist, als JSON (für die App und Gesundheitsprüfungen).
pub async fn stand(tcp: &mut TcpStream, kopf: &Kopf, lobby: &Lobby) {
    let (gastgeber, ersatz) = lobby.stand();
    let json = serde_json::to_vec(&Stand {
        dienst: "agent-office-vermittler",
        version: env!("CARGO_PKG_VERSION"),
        protokoll: PROTOKOLL,
        gastgeber,
        ersatz,
    })
    .unwrap_or_default();
    antworten(
        tcp,
        "200 OK",
        "application/json",
        &json,
        kopf.methode == "HEAD",
    )
    .await;
}

pub async fn nicht_gefunden(tcp: &mut TcpStream) {
    antworten(
        tcp,
        "404 Not Found",
        "text/plain; charset=utf-8",
        "Gibt es hier nicht.".as_bytes(),
        false,
    )
    .await;
}

/// Solange kein Gastgeber da ist, sieht jeder Browser diese Seite.
pub async fn geschlossen(tcp: &mut TcpStream, kopf: &Kopf) {
    let ist_seite = kopf.wert("accept").is_some_and(|a| a.contains("text/html"));
    if !ist_seite || kopf.ist_upgrade() {
        let text = "Die Lobby ist gerade geschlossen: Kein Gastgeber ist online.";
        antworten(
            tcp,
            "503 Service Unavailable",
            "text/plain; charset=utf-8",
            text.as_bytes(),
            false,
        )
        .await;
        return;
    }
    antworten(
        tcp,
        "503 Service Unavailable",
        "text/html; charset=utf-8",
        GESCHLOSSEN.as_bytes(),
        kopf.methode == "HEAD",
    )
    .await;
}

const GESCHLOSSEN: &str = r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="refresh" content="10"><title>Lobby geschlossen – Agent Office</title>
<style>
body{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:#fff6ea;color:#2b2d42;font:16px/1.5 system-ui,sans-serif}
main{max-width:34rem;margin:1rem;padding:2rem;border:3px solid #2b2d42;border-radius:18px;background:#fffdf8;box-shadow:0 6px 0 #2b2d42}
h1{margin:0 0 .5rem;font-size:1.4rem}p{margin:.6rem 0}small{color:#6b6f80}
</style></head><body><main>
<h1>🏢 Die Lobby ist gerade geschlossen</h1>
<p>Gerade ist kein Gastgeber online. Die Lobby läuft im Office eines Spielers, nicht hier: Dieser Server vermittelt nur.</p>
<p>Sobald jemand in der App <b>Agent Office</b> unter <b>🛰️ Über den Vermittler gastgeben</b> die Lobby öffnet, geht es hier weiter.</p>
<p><small>Diese Seite schaut alle 10 Sekunden von selbst wieder nach.</small></p>
</main></body></html>
"#;
