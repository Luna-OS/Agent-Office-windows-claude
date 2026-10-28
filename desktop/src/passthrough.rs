//! Durchleitung: reicht `http://127.0.0.1:48600` (auch WebSockets) an eine Lobby weiter.
//! Für den Browser ist das dann „der eigene Computer“, also gehen Sprache und Bildschirm teilen auch
//! bei einer Lobby über `http://` auf einem anderen Gerät.
//!
//! Die Lobby soll sich selbst gemeint sehen, nicht `127.0.0.1:48600`: Ein lokaler Host mit fremdem Port
//! ist für das Office ein Dienste-Tunnel zum Server eines Arbeiters. Darum bekommt jede Verbindung im
//! ersten Anfragekopf `Host` (und ein `Origin` von hier) mit der Adresse der Lobby, und außer
//! WebSockets endet sie nach der Antwort (`Connection: close`), damit jede Anfrage so einen Kopf hat.
//! Anfragen an einen anderen Namen als 127.0.0.1 oder localhost (DNS-Rebinding) werden abgewiesen.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

pub const PORT: u16 = 48600;

/// So lang darf ein Anfragekopf höchstens sein.
const KOPF_MAX: usize = 64 * 1024;

pub struct Durchleitung {
    pub ziel: String,
    port: u16,
    aktiv: Arc<AtomicBool>,
}

impl Durchleitung {
    /// Startet die Durchleitung zu `ziel` (`host:port`). Ein Fehler heißt meist: Port ist belegt.
    pub fn starten(ziel: &str) -> io::Result<Self> {
        Self::starten_auf(PORT, ziel)
    }

    fn starten_auf(port: u16, ziel: &str) -> io::Result<Self> {
        let hoerer = TcpListener::bind(("127.0.0.1", port))?;
        let port = hoerer.local_addr()?.port();
        let aktiv = Arc::new(AtomicBool::new(true));
        let (flag, ziel_kopie) = (aktiv.clone(), ziel.to_string());
        thread::spawn(move || {
            for eingang in hoerer.incoming() {
                if !flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(eingang) = eingang else { continue };
                let ziel = ziel_kopie.clone();
                thread::spawn(move || weiterreichen(eingang, &ziel, port));
            }
        });
        Ok(Self {
            ziel: ziel.to_string(),
            port,
            aktiv,
        })
    }
}

impl Drop for Durchleitung {
    fn drop(&mut self) {
        self.aktiv.store(false, Ordering::Relaxed);
        // Den wartenden `accept` wecken, damit der Thread endet und der Port frei wird.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Eine Verbindung vom Browser: ersten Kopf lesen und umschreiben, dann beide Seiten verbinden.
fn weiterreichen(mut eingang: TcpStream, ziel: &str, port: u16) {
    let mut gelesen = Vec::new();
    let mut puffer = [0u8; 8192];
    let kopf_ende = loop {
        if let Some(i) = gelesen.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if gelesen.len() > KOPF_MAX {
            return;
        }
        match eingang.read(&mut puffer) {
            Ok(0) | Err(_) => return,
            Ok(n) => gelesen.extend_from_slice(&puffer[..n]),
        }
    };
    let Ok(kopf) = std::str::from_utf8(&gelesen[..kopf_ende]) else {
        return;
    };
    let Some(neu) = kopf_umschreiben(kopf, ziel, port) else {
        let _ = eingang.write_all(
            b"HTTP/1.1 421 Misdirected Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    };
    let Ok(mut aus) = TcpStream::connect(ziel) else {
        let _ = eingang.write_all(
            "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\nDie Lobby antwortet nicht. Läuft sie, und ist dieser Computer im selben Netz?"
                .as_bytes(),
        );
        return;
    };
    if aus.write_all(neu.as_bytes()).is_err() || aus.write_all(&gelesen[kopf_ende..]).is_err() {
        return;
    }
    verbinden(eingang, aus);
}

/// Der Anfragekopf, wie er an die Lobby geht, oder `None`, wenn er nicht an diese Durchleitung ging.
/// `Host` wird zur Lobby (`ziel`), ein `Origin` von hier ebenso; außer bei WebSockets
/// `Connection: close`, damit die nächste Anfrage wieder hier durch muss.
fn kopf_umschreiben(kopf: &str, ziel: &str, port: u16) -> Option<String> {
    let hier = |wert: &str| {
        let w = wert.trim().to_ascii_lowercase();
        w == format!("127.0.0.1:{port}") || w == format!("localhost:{port}")
    };
    let mut zeilen = kopf.trim_end_matches("\r\n").split("\r\n");
    let anfrage = zeilen.next()?;
    let mut raus = vec![anfrage.to_string()];
    let (mut host_ok, mut upgrade) = (false, false);
    for zeile in zeilen {
        let Some((name, wert)) = zeile.split_once(':') else {
            raus.push(zeile.to_string());
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => {
                host_ok = hier(wert);
                raus.push(format!("Host: {ziel}"));
            }
            "origin" => match wert.trim().split_once("://") {
                Some((schema, rest)) if hier(rest) => {
                    raus.push(format!("Origin: {schema}://{ziel}"))
                }
                _ => raus.push(zeile.to_string()),
            },
            "upgrade" => {
                upgrade = true;
                raus.push(zeile.to_string());
            }
            "connection" | "keep-alive" => {}
            _ => raus.push(zeile.to_string()),
        }
    }
    if !host_ok {
        return None;
    }
    raus.push(if upgrade {
        "Connection: Upgrade".into()
    } else {
        "Connection: close".into()
    });
    Some(raus.join("\r\n") + "\r\n\r\n")
}

fn verbinden(a: TcpStream, b: TcpStream) {
    let (Ok(mut a_lesen), Ok(mut b_lesen)) = (a.try_clone(), b.try_clone()) else {
        return;
    };
    let (mut a_schreiben, mut b_schreiben) = (a, b);
    let hin = thread::spawn(move || {
        let _ = io::copy(&mut a_lesen, &mut b_schreiben);
        let _ = b_schreiben.shutdown(Shutdown::Write);
    });
    let _ = io::copy(&mut b_lesen, &mut a_schreiben);
    let _ = a_schreiben.shutdown(Shutdown::Write);
    let _ = hin.join();
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZIEL: &str = "100.78.107.162:4600";

    #[test]
    fn host_und_origin_werden_zur_lobby() {
        let kopf = "POST /api/anmeldung HTTP/1.1\r\nHost: 127.0.0.1:48600\r\nOrigin: http://127.0.0.1:48600\r\nConnection: keep-alive\r\nContent-Length: 2\r\n\r\n";
        let neu = kopf_umschreiben(kopf, ZIEL, PORT).unwrap();
        assert_eq!(
            neu,
            "POST /api/anmeldung HTTP/1.1\r\nHost: 100.78.107.162:4600\r\nOrigin: http://100.78.107.162:4600\r\nContent-Length: 2\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn websockets_bleiben_offen() {
        let kopf = "GET /ws HTTP/1.1\r\nhost: localhost:48600\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: abc\r\n\r\n";
        let neu = kopf_umschreiben(kopf, ZIEL, PORT).unwrap();
        assert!(neu.contains("Host: 100.78.107.162:4600\r\n"));
        assert!(neu.contains("Upgrade: websocket\r\n"));
        assert!(neu.ends_with("Connection: Upgrade\r\n\r\n"));
        assert!(!neu.contains("Connection: close"));
    }

    #[test]
    fn fremde_namen_und_origins_bleiben_draussen() {
        // DNS-Rebinding: eine fremde Seite, die auf 127.0.0.1 zeigt, kommt nicht durch.
        assert_eq!(
            kopf_umschreiben(
                "GET / HTTP/1.1\r\nHost: boese.example:48600\r\n\r\n",
                ZIEL,
                PORT
            ),
            None
        );
        assert_eq!(kopf_umschreiben("GET / HTTP/1.1\r\n\r\n", ZIEL, PORT), None);
        // Ein Origin von woanders bleibt, wie er ist: das Office weist ihn dann selbst ab.
        let neu = kopf_umschreiben(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1:48600\r\nOrigin: https://boese.example\r\n\r\n",
            ZIEL,
            PORT,
        )
        .unwrap();
        assert!(neu.contains("Origin: https://boese.example\r\n"));
    }

    #[test]
    fn die_lobby_bekommt_den_umgeschriebenen_kopf() {
        // Eine Lobby zum Schein: liest den Kopf, antwortet, schließt.
        let lobby = TcpListener::bind("127.0.0.1:0").unwrap();
        let ziel = lobby.local_addr().unwrap().to_string();
        let gesehen = thread::spawn(move || {
            let (mut s, _) = lobby.accept().unwrap();
            let mut kopf = Vec::new();
            let mut b = [0u8; 1024];
            while !kopf.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = s.read(&mut b).unwrap();
                kopf.extend_from_slice(&b[..n]);
            }
            s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8(kopf).unwrap()
        });
        let d = Durchleitung::starten_auf(0, &ziel).unwrap();
        let mut browser = TcpStream::connect(("127.0.0.1", d.port)).unwrap();
        write!(
            browser,
            "GET /anmeldung HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\n\r\n",
            d.port, d.port
        )
        .unwrap();
        let mut antwort = String::new();
        browser.read_to_string(&mut antwort).unwrap();
        assert!(antwort.ends_with("ok"), "{antwort}");
        let kopf = gesehen.join().unwrap();
        assert!(kopf.contains(&format!("Host: {ziel}\r\n")), "{kopf}");
        assert!(
            kopf.contains(&format!("Origin: http://{ziel}\r\n")),
            "{kopf}"
        );
        assert!(kopf.contains("Connection: close\r\n"), "{kopf}");
    }
}
