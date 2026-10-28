//! Sitzungs-Cookies (signiert mit HMAC), Einmal-Anmeldelinks und die Bremse gegen Passwort-Raten.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::passwort::gleich;
use crate::speicher::{geheimnis, jetzt_ms};

pub const COOKIE: &str = "oao_sitzung";
pub const SITZUNG_DAUER_MS: u64 = 14 * 24 * 60 * 60_000;
const MAX_VERSUCHE: u32 = 10;
const VERSUCHS_FENSTER: Duration = Duration::from_secs(5 * 60);
const MAX_LINKS: usize = 32;
const UEBERGABE_DAUER_MS: u64 = 5 * 60_000;

/// Ein angemeldeter Browser: mit eigenem Konto oder (ohne) mit dem gemeinsamen Passwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sitzung {
    pub konto: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Inhalt {
    exp: u64,
    n: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    u: Option<String>,
}

struct Link {
    konto: Option<String>,
    bis: Option<u64>,
}

pub struct Sitzungen {
    geheimnis: Vec<u8>,
    /// Ändert sich mit dem gemeinsamen Passwort, damit ein neues Passwort alle alten Sitzungen abmeldet.
    passwort_salz: Mutex<String>,
    versuche: Mutex<HashMap<IpAddr, (u32, Instant)>>,
    links: Mutex<Vec<(String, Link)>>,
}

fn hmac(schluessel: &[u8], teile: &[&[u8]]) -> Vec<u8> {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(schluessel).expect("HMAC nimmt jede Länge");
    for t in teile {
        m.update(t);
    }
    m.finalize().into_bytes().to_vec()
}

fn sha(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Sitzungen {
    /// Lädt das Geheimnis aus `<daten>/sitzung.key` oder legt es an.
    pub fn laden(daten: &Path, passwort_hash: &str) -> Self {
        let datei = daten.join("sitzung.key");
        let geheimnis = match std::fs::read_to_string(&datei) {
            Ok(s) if s.trim().len() >= 32 => s.trim().to_string(),
            _ => {
                let s = geheimnis(32);
                let _ = std::fs::create_dir_all(daten);
                if let Err(e) = std::fs::write(&datei, &s) {
                    tracing::warn!("Konnte {} nicht schreiben: {e}", datei.display());
                }
                s
            }
        };
        Self {
            geheimnis: geheimnis.into_bytes(),
            passwort_salz: Mutex::new(passwort_hash.to_string()),
            versuche: Mutex::default(),
            links: Mutex::default(),
        }
    }

    /// Das gemeinsame Passwort hat sich geändert: alte Sitzungen ohne Konto gelten nicht mehr.
    pub fn passwort_geaendert(&self, passwort_hash: &str) {
        *self.passwort_salz.lock().unwrap() = passwort_hash.to_string();
    }

    fn schluessel(&self, konto: bool) -> Vec<u8> {
        if konto {
            hmac(&self.geheimnis, &[b"konto-sitzung:"])
        } else {
            let salz = self.passwort_salz.lock().unwrap().clone();
            hmac(&self.geheimnis, &[b"sitzung:", salz.as_bytes()])
        }
    }

    fn signieren(&self, inhalt: &str, konto: bool) -> String {
        B64.encode(hmac(&self.schluessel(konto), &[inhalt.as_bytes()]))
    }

    /// Ein neuer Cookie-Wert.
    pub fn ausstellen(&self, konto: Option<&str>) -> String {
        let inhalt = Inhalt {
            exp: jetzt_ms() + SITZUNG_DAUER_MS,
            n: geheimnis(8),
            u: konto.map(String::from),
        };
        let inhalt = B64.encode(serde_json::to_vec(&inhalt).expect("serialisierbar"));
        let sig = self.signieren(&inhalt, konto.is_some());
        format!("{inhalt}.{sig}")
    }

    /// Wen ein Cookie-Wert anmeldet (nur Signatur und Ablauf; ob das Konto noch existiert, prüft der Aufrufer).
    pub fn pruefen(&self, wert: &str) -> Option<Sitzung> {
        let (inhalt, sig) = wert.split_once('.')?;
        let roh = B64.decode(inhalt).ok()?;
        let i: Inhalt = serde_json::from_slice(&roh).ok()?;
        let erwartet = self.signieren(inhalt, i.u.is_some());
        if !gleich(sig.as_bytes(), erwartet.as_bytes()) || i.exp <= jetzt_ms() {
            return None;
        }
        Some(Sitzung { konto: i.u })
    }

    /// Zählt einen Rate-Versuch; `false`, wenn diese IP ihr Fenster aufgebraucht hat.
    pub fn versuch_erlaubt(&self, ip: IpAddr) -> bool {
        let mut v = self.versuche.lock().unwrap();
        let jetzt = Instant::now();
        if v.len() > 10_000 {
            v.retain(|_, (_, seit)| jetzt.duration_since(*seit) < VERSUCHS_FENSTER);
        }
        let e = v.entry(ip).or_insert((0, jetzt));
        if jetzt.duration_since(e.1) >= VERSUCHS_FENSTER {
            *e = (0, jetzt);
        }
        e.0 += 1;
        e.0 <= MAX_VERSUCHE
    }

    pub fn erfolg(&self, ip: IpAddr) {
        self.versuche.lock().unwrap().remove(&ip);
    }

    /// Ein Einmal-Anmeldelink. Ohne `uebergabe` für den, der das Office im Terminal gestartet hat
    /// (meldet wie das gemeinsame Passwort an); mit einer Sitzung für einen anderen Browser, fünf Minuten lang.
    pub fn link_schluessel(&self, uebergabe: Option<&Sitzung>) -> String {
        let s = geheimnis(24);
        let mut links = self.links.lock().unwrap();
        if links.len() >= MAX_LINKS {
            links.remove(0);
        }
        let link = match uebergabe {
            Some(u) => Link {
                konto: u.konto.clone(),
                bis: Some(jetzt_ms() + UEBERGABE_DAUER_MS),
            },
            None => Link {
                konto: None,
                bis: None,
            },
        };
        links.push((sha(&s), link));
        s
    }

    /// Löst einen Link ein (nur einmal, nur rechtzeitig): wen er anmeldet.
    pub fn link_einloesen(&self, schluessel: &str) -> Option<Sitzung> {
        if schluessel.is_empty() {
            return None;
        }
        let h = sha(schluessel);
        let mut links = self.links.lock().unwrap();
        let i = links
            .iter()
            .position(|(k, _)| gleich(k.as_bytes(), h.as_bytes()))?;
        let (_, link) = links.remove(i);
        if link.bis.is_some_and(|b| b < jetzt_ms()) {
            return None;
        }
        Some(Sitzung { konto: link.konto })
    }
}

/// Der Cookie-Name je Port: Cookies kennen keine Ports, zwei Offices auf einem Host bekommen je ihren.
pub fn cookie_name(host: Option<&str>) -> String {
    match host
        .and_then(|h| h.rsplit_once(':'))
        .map(|(_, p)| p)
        .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        Some(port) => format!("{COOKIE}_{port}"),
        None => COOKIE.to_string(),
    }
}

/// Liest ein Cookie aus dem Cookie-Header.
pub fn cookie_lesen<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header
        .split(';')
        .filter_map(|t| t.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.trim())
}

/// Set-Cookie-Wert für eine Sitzung.
pub fn cookie_setzen(name: &str, wert: &str, sicher: bool) -> String {
    format!(
        "{name}={wert}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SITZUNG_DAUER_MS / 1000,
        if sicher { "; Secure" } else { "" }
    )
}

pub fn cookie_loeschen(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausstellen_und_pruefen() {
        let dir = tempfile::tempdir().unwrap();
        let s = Sitzungen::laden(dir.path(), "pw1");
        let c = s.ausstellen(None);
        assert_eq!(s.pruefen(&c), Some(Sitzung { konto: None }));
        let k = s.ausstellen(Some("abc"));
        assert_eq!(s.pruefen(&k).unwrap().konto.as_deref(), Some("abc"));
        assert!(s.pruefen(&format!("{c}x")).is_none());
        s.passwort_geaendert("pw2");
        assert!(s.pruefen(&c).is_none(), "neues Passwort meldet ab");
        assert!(s.pruefen(&k).is_some(), "Konten bleiben");
    }

    #[test]
    fn links_nur_einmal() {
        let dir = tempfile::tempdir().unwrap();
        let s = Sitzungen::laden(dir.path(), "");
        let l = s.link_schluessel(Some(&Sitzung {
            konto: Some("k".into()),
        }));
        assert_eq!(s.link_einloesen(&l).unwrap().konto.as_deref(), Some("k"));
        assert!(s.link_einloesen(&l).is_none());
    }

    #[test]
    fn bremse() {
        let dir = tempfile::tempdir().unwrap();
        let s = Sitzungen::laden(dir.path(), "");
        let ip: IpAddr = [1, 2, 3, 4].into();
        for _ in 0..MAX_VERSUCHE {
            assert!(s.versuch_erlaubt(ip));
        }
        assert!(!s.versuch_erlaubt(ip));
        s.erfolg(ip);
        assert!(s.versuch_erlaubt(ip));
    }

    #[test]
    fn cookies() {
        assert_eq!(cookie_name(Some("localhost:4600")), "oao_sitzung_4600");
        assert_eq!(cookie_name(Some("office.example")), "oao_sitzung");
        assert_eq!(
            cookie_lesen("a=1; oao_sitzung_4600=xyz", "oao_sitzung_4600"),
            Some("xyz")
        );
    }
}
