//! Lobby-Adressen: Eingaben deuten, Erreichbarkeit prüfen, Links für den Browser bauen.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const STANDARD_PORT: u16 = 4600;

/// Eine gedeutete Eingabe: die Lobby selbst und ein optionaler Einladungslink (`/beitreten#…`).
#[derive(Debug, PartialEq, Clone)]
pub struct Eingabe {
    /// z. B. `http://100.78.107.162:4600` (ohne Pfad)
    pub basis: String,
    /// Pfad und Fragment, wenn ein Link eingefügt wurde, z. B. `/beitreten#abc`
    pub rest: String,
}

pub fn deuten(text: &str) -> Option<Eingabe> {
    let t = text.trim();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return None;
    }
    let (schema, ohne) = match t.split_once("://") {
        Some((s, r)) if s == "http" || s == "https" => (s, r),
        Some(_) => return None,
        None => ("http", t),
    };
    let ende = ohne.find(['/', '#', '?']).unwrap_or(ohne.len());
    let (host, rest) = ohne.split_at(ende);
    if host.is_empty() {
        return None;
    }
    let hat_port = host.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok());
    let host = if hat_port || schema == "https" { host.to_string() } else { format!("{host}:{STANDARD_PORT}") };
    let rest = if rest == "/" { "" } else { rest };
    Some(Eingabe { basis: format!("{schema}://{host}"), rest: rest.to_string() })
}

/// `host:port` für die TCP-Verbindung.
pub fn host_port(basis: &str) -> Option<String> {
    let ohne = basis.split_once("://")?.1;
    if ohne.contains(':') {
        Some(ohne.to_string())
    } else if basis.starts_with("https") {
        Some(format!("{ohne}:443"))
    } else {
        Some(format!("{ohne}:80"))
    }
}

pub fn erreichbar(basis: &str) -> bool {
    let Some(hp) = host_port(basis) else { return false };
    let Ok(mut adressen) = hp.to_socket_addrs() else { return false };
    adressen.any(|a| TcpStream::connect_timeout(&a, Duration::from_secs(4)).is_ok())
}

/// Gehört die Adresse zu diesem Computer? Dann ist der Browser dort ohnehin im „sicheren Kontext“.
pub fn ist_lokal(basis: &str) -> bool {
    let h = host_port(basis).unwrap_or_default();
    h.starts_with("127.") || h.starts_with("localhost") || h.starts_with("[::1]")
}

/// Braucht die Lobby die Durchleitung? Ja bei `http://` auf einem anderen Gerät:
/// Mikrofon und Bildschirm teilen gehen im Browser nur auf `https://` und auf dem eigenen Computer.
pub fn braucht_durchleitung(basis: &str) -> bool {
    basis.starts_with("http://") && !ist_lokal(basis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_ohne_schema_und_port() {
        let e = deuten("100.78.107.162").unwrap();
        assert_eq!(e.basis, "http://100.78.107.162:4600");
        assert_eq!(e.rest, "");
    }

    #[test]
    fn einladungslink() {
        let e = deuten("http://umbrel.local:4600/beitreten#abc123").unwrap();
        assert_eq!(e.basis, "http://umbrel.local:4600");
        assert_eq!(e.rest, "/beitreten#abc123");
    }

    #[test]
    fn https_ohne_port_bleibt() {
        assert_eq!(deuten("https://office.example.com/").unwrap().basis, "https://office.example.com");
    }

    #[test]
    fn unsinn_wird_abgelehnt() {
        assert!(deuten("").is_none());
        assert!(deuten("zwei woerter").is_none());
        assert!(deuten("ftp://x").is_none());
    }

    #[test]
    fn durchleitung_nur_fuer_fremdes_http() {
        assert!(braucht_durchleitung("http://192.168.1.5:4600"));
        assert!(!braucht_durchleitung("http://127.0.0.1:4600"));
        assert!(!braucht_durchleitung("https://office.example.com"));
    }
}
