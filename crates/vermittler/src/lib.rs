//! Der Vermittler von Agent Office: eine Server-App ohne Oberfläche, die eine Lobby vermittelt.
//!
//! Die Lobby (das Office mit Etagen, Arbeitern, Board und allem) läuft in der App eines Spielers,
//! dem Gastgeber. Sein Office meldet sich über einen WebSocket beim Vermittler an
//! ([`knoten`]), und jede andere Verbindung zum Vermittler wird zu ihm durchgereicht ([`tunnel`]).
//! Der Vermittler rechnet dabei nichts, er reicht nur Bytes weiter: Dafür reicht ein kleiner Server.

pub mod knoten;
pub mod konfig;
pub mod kopf;
pub mod lobby;
pub mod seiten;
pub mod tunnel;
mod vorspann;

use lobby::Lobby;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

/// So viele Verbindungen hält der Vermittler höchstens gleichzeitig offen.
pub const VERBINDUNGEN_MAX: usize = 1024;

/// Startet den Vermittler. Gibt die Adresse zurück, auf der er lauscht, und seine Aufgabe.
pub async fn starten(konfig: konfig::Konfig) -> io::Result<(SocketAddr, JoinHandle<()>)> {
    let hoerer = TcpListener::bind((konfig.host.as_str(), konfig.port)).await?;
    let adresse = hoerer.local_addr()?;
    let lobby = Lobby::neu(&konfig.passwort);
    let hinter_proxy = konfig.hinter_proxy;
    let frei = Arc::new(Semaphore::new(VERBINDUNGEN_MAX));
    let aufgabe = tokio::spawn(async move {
        loop {
            let (tcp, von) = match hoerer.accept().await {
                Ok(v) => v,
                Err(_) => {
                    // Etwa keine freien Dateien mehr: kurz warten statt im Kreis zu laufen.
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            let Ok(erlaubnis) = frei.clone().try_acquire_owned() else {
                continue; // zu viele: die Verbindung wird gleich wieder geschlossen
            };
            let lobby = lobby.clone();
            tokio::spawn(async move {
                verbindung(lobby, tcp, von, hinter_proxy).await;
                drop(erlaubnis);
            });
        }
    });
    Ok((adresse, aufgabe))
}

async fn verbindung(lobby: Arc<Lobby>, mut tcp: TcpStream, von: SocketAddr, hinter_proxy: bool) {
    let _ = tcp.set_nodelay(true);
    let Ok(Some(kopf)) = kopf::lesen(&mut tcp).await else {
        return;
    };
    let ip = browser_adresse(&kopf, von, hinter_proxy);
    match kopf.pfad_ohne_abfrage() {
        "/vermittler/knoten" if kopf.ist_upgrade() => {
            knoten::bedienen(lobby, tcp, kopf.gelesen()).await
        }
        "/vermittler/stand" => seiten::stand(&mut tcp, &kopf, &lobby).await,
        p if p.starts_with("/vermittler/") => seiten::nicht_gefunden(&mut tcp).await,
        _ => tunnel::durchreichen(lobby, tcp, kopf, ip).await,
    }
}

/// Von wo der Browser kommt: die Gegenstelle, oder hinter einem Proxy die Adresse, die er in
/// `X-Forwarded-For` ganz rechts angehängt hat (was links davon steht, kann der Browser selbst schreiben).
pub fn browser_adresse(kopf: &kopf::Kopf, von: SocketAddr, hinter_proxy: bool) -> String {
    let vom_proxy = hinter_proxy
        .then(|| kopf.wert("x-forwarded-for"))
        .flatten()
        .and_then(|w| w.rsplit(',').next())
        .and_then(|a| a.trim().parse::<std::net::IpAddr>().ok());
    vom_proxy
        .unwrap_or_else(|| von.ip())
        .to_canonical()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_adresse_des_browsers() {
        let kopf = kopf::deuten(
            b"GET / HTTP/1.1\r\nX-Forwarded-For: 6.6.6.6, 192.168.1.20\r\n\r\n",
            vec![],
        )
        .unwrap();
        let proxy: SocketAddr = "[::ffff:172.17.0.3]:5000".parse().unwrap();
        assert_eq!(browser_adresse(&kopf, proxy, true), "192.168.1.20");
        assert_eq!(browser_adresse(&kopf, proxy, false), "172.17.0.3");
        let kaputt = kopf::deuten(
            b"GET / HTTP/1.1\r\nX-Forwarded-For: quatsch\r\n\r\n",
            vec![],
        )
        .unwrap();
        assert_eq!(browser_adresse(&kaputt, proxy, true), "172.17.0.3");
    }
}
