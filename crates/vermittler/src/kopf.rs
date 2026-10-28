//! Der erste Anfragekopf einer Verbindung: lesen, deuten und für den Gastgeber umschreiben.
//!
//! Der Vermittler schaut nur in den ersten Kopf jeder Verbindung: Geht er an `/vermittler/…`, ist er
//! für den Vermittler selbst, sonst wird die Verbindung zum Gastgeber durchgereicht. Damit jede Anfrage
//! so einen Kopf hat, endet eine durchgereichte Verbindung nach der Antwort (`Connection: close`),
//! außer bei WebSockets.

use std::io;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// So lang darf ein Anfragekopf höchstens sein.
pub const KOPF_MAX: usize = 64 * 1024;
/// So lange darf der erste Kopf auf sich warten lassen.
const KOPF_ZEIT: Duration = Duration::from_secs(30);

/// Ein gelesener Anfragekopf: die Anfragezeile, die Kopfzeilen und was danach schon mitkam.
#[derive(Debug, Clone)]
pub struct Kopf {
    pub methode: String,
    pub pfad: String,
    pub version: String,
    pub zeilen: Vec<(String, String)>,
    /// Der Kopf, wie er kam (bis einschließlich der Leerzeile).
    pub roh: Vec<u8>,
    /// Bytes nach dem Kopf, die schon gelesen wurden (der Anfang des Körpers).
    pub rest: Vec<u8>,
}

impl Kopf {
    /// Der Wert der ersten Kopfzeile `name` (Groß- und Kleinschreibung egal).
    pub fn wert(&self, name: &str) -> Option<&str> {
        self.zeilen
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, w)| w.as_str())
    }

    /// Ob die Anfrage eine Verbindung zu einem WebSocket (oder einem anderen Protokoll) aufmachen will.
    pub fn ist_upgrade(&self) -> bool {
        self.wert("upgrade").is_some_and(|w| !w.trim().is_empty())
    }

    /// Alles, was von der Verbindung schon gelesen wurde: der Kopf und was danach kam.
    pub fn gelesen(&self) -> Vec<u8> {
        let mut alles = self.roh.clone();
        alles.extend_from_slice(&self.rest);
        alles
    }

    /// Der Pfad ohne Abfrage (`/vermittler/status?x` → `/vermittler/status`).
    pub fn pfad_ohne_abfrage(&self) -> &str {
        self.pfad.split(['?', '#']).next().unwrap_or("")
    }

    /// Der Kopf, wie er an den Gastgeber geht: Weiterleitungs-Kopfzeilen vom Browser fliegen raus
    /// (sonst könnte er sich eine andere Adresse geben), `X-Forwarded-For` sagt, von wo er kam, und
    /// außer bei WebSockets heißt es `Connection: close`.
    pub fn fuer_gastgeber(&self, von: &str) -> Vec<u8> {
        let mut raus = format!("{} {} {}\r\n", self.methode, self.pfad, self.version);
        for (name, wert) in &self.zeilen {
            let klein = name.to_ascii_lowercase();
            if matches!(
                klein.as_str(),
                "connection"
                    | "keep-alive"
                    | "proxy-connection"
                    | "forwarded"
                    | "x-forwarded-for"
                    | "x-forwarded-host"
                    | "x-forwarded-proto"
                    | "x-real-ip"
            ) {
                continue;
            }
            raus.push_str(name);
            raus.push_str(": ");
            raus.push_str(wert);
            raus.push_str("\r\n");
        }
        raus.push_str("X-Forwarded-For: ");
        raus.push_str(von);
        raus.push_str(if self.ist_upgrade() {
            "\r\nConnection: Upgrade\r\n\r\n"
        } else {
            "\r\nConnection: close\r\n\r\n"
        });
        let mut bytes = raus.into_bytes();
        bytes.extend_from_slice(&self.rest);
        bytes
    }
}

/// Liest den ersten Anfragekopf von `strom`. `None`, wenn keiner kommt, er zu lang ist oder kein HTTP ist.
pub async fn lesen(strom: &mut TcpStream) -> io::Result<Option<Kopf>> {
    let mut gelesen = Vec::new();
    let mut puffer = [0u8; 8192];
    let ende = tokio::time::timeout(KOPF_ZEIT, async {
        loop {
            if let Some(i) = gelesen.windows(4).position(|w| w == b"\r\n\r\n") {
                return io::Result::Ok(Some(i + 4));
            }
            if gelesen.len() > KOPF_MAX {
                return Ok(None);
            }
            let n = strom.read(&mut puffer).await?;
            if n == 0 {
                return Ok(None);
            }
            gelesen.extend_from_slice(&puffer[..n]);
        }
    })
    .await
    .unwrap_or(Ok(None))?;
    Ok(ende.and_then(|e| deuten(&gelesen[..e], gelesen[e..].to_vec())))
}

/// Deutet einen Kopf (bis einschließlich der Leerzeile).
pub fn deuten(kopf: &[u8], rest: Vec<u8>) -> Option<Kopf> {
    let text = std::str::from_utf8(kopf).ok()?;
    let mut zeilen = text.trim_end_matches("\r\n").split("\r\n");
    let mut anfrage = zeilen.next()?.split(' ');
    let (methode, pfad, version) = (anfrage.next()?, anfrage.next()?, anfrage.next()?);
    if anfrage.next().is_some() || !version.starts_with("HTTP/1.") || !pfad.starts_with('/') {
        return None;
    }
    let mut kopfzeilen = Vec::new();
    for zeile in zeilen {
        let (name, wert) = zeile.split_once(':')?;
        if name.is_empty() || name.contains(' ') {
            return None;
        }
        kopfzeilen.push((name.to_string(), wert.trim().to_string()));
    }
    Some(Kopf {
        methode: methode.to_string(),
        pfad: pfad.to_string(),
        version: version.to_string(),
        zeilen: kopfzeilen,
        roh: kopf.to_vec(),
        rest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kopf(text: &str) -> Kopf {
        deuten(text.as_bytes(), b"abc".to_vec()).unwrap()
    }

    #[test]
    fn deutet_eine_anfrage() {
        let k = kopf(
            "GET /vermittler/status?x=1 HTTP/1.1\r\nHost: umbrel.local:4600\r\nUpgrade: websocket\r\n\r\n",
        );
        assert_eq!(k.methode, "GET");
        assert_eq!(k.pfad_ohne_abfrage(), "/vermittler/status");
        assert_eq!(k.wert("host"), Some("umbrel.local:4600"));
        assert!(k.ist_upgrade());
        assert_eq!(k.rest, b"abc");
    }

    #[test]
    fn kein_http_wird_nichts() {
        assert!(deuten(b"\x16\x03\x01 kaputt\r\n\r\n", vec![]).is_none());
        assert!(deuten(b"GET http://x/ HTTP/1.1\r\n\r\n", vec![]).is_none());
        assert!(deuten(b"GET / HTTP/1.1\r\nkaputte zeile\r\n\r\n", vec![]).is_none());
    }

    #[test]
    fn fuer_den_gastgeber_ohne_fremde_weiterleitungen() {
        let k = kopf(
            "POST /api/login HTTP/1.1\r\nHost: 127.0.0.1:48600\r\nX-Forwarded-For: 6.6.6.6\r\nX-Forwarded-Host: boese.example\r\nConnection: keep-alive\r\nContent-Length: 3\r\n\r\n",
        );
        let neu = String::from_utf8(k.fuer_gastgeber("100.64.0.7")).unwrap();
        assert_eq!(
            neu,
            "POST /api/login HTTP/1.1\r\nHost: 127.0.0.1:48600\r\nContent-Length: 3\r\nX-Forwarded-For: 100.64.0.7\r\nConnection: close\r\n\r\nabc"
        );
    }

    #[test]
    fn websockets_bleiben_offen() {
        let k = kopf(
            "GET /ws HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        let neu = String::from_utf8(k.fuer_gastgeber("1.2.3.4")).unwrap();
        assert!(neu.contains("Upgrade: websocket\r\n"));
        assert!(neu.contains("Connection: Upgrade\r\n\r\n"));
        assert!(!neu.contains("close"));
    }
}
