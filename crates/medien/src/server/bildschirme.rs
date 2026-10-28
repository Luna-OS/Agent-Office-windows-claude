//! Wer teilt gerade seinen Bildschirm, und sein jeweils letztes Bild.
//!
//! Jeder Teilende hat einen `watch`-Kanal mit dem letzten JPEG. Zuschauer hängen daran; wer zu langsam
//! ist, bekommt einfach das neueste Bild (keine Warteschlange, die vollläuft). Hört jemand auf zu teilen,
//! wird der Sender fallen gelassen und alle Streams enden von selbst.

use std::collections::HashMap;
use std::sync::Mutex;

use bytes::Bytes;
use protokoll::medien::Bildschirm;
use tokio::sync::{broadcast, watch};

use super::{PFAD, Sitzung, jetzt_ms, pfad_teil};

/// Größtes erlaubtes Einzelbild.
pub const MAX_BILD: usize = 2 * 1024 * 1024;
/// Kommt so lange kein Bild, gilt das Teilen als beendet (Tab zu, Netz weg).
pub const STILLE_MS: u64 = 15_000;

struct Teilung {
    info: Bildschirm,
    bild: watch::Sender<Option<Bytes>>,
    letztes_ms: u64,
}

/// Alle geteilten Bildschirme des Servers.
pub struct Bildschirme {
    teilungen: Mutex<HashMap<String, Teilung>>,
    /// Die Etage, deren Liste sich geändert hat.
    aenderungen: broadcast::Sender<String>,
}

impl Default for Bildschirme {
    fn default() -> Self {
        Self {
            teilungen: Mutex::new(HashMap::new()),
            aenderungen: broadcast::channel(64).0,
        }
    }
}

impl Bildschirme {
    /// Fängt an zu teilen (oder teilt schon). Gibt den Eintrag zurück.
    pub fn starten(&self, s: &Sitzung) -> Bildschirm {
        let jetzt = jetzt_ms();
        let mut t = self.teilungen.lock().unwrap();
        if let Some(alt) = t.get_mut(&s.spieler) {
            alt.letztes_ms = jetzt;
            if alt.info.etage == s.etage {
                return alt.info.clone();
            }
            let alte_etage = alt.info.etage.clone();
            t.remove(&s.spieler);
            let _ = self.aenderungen.send(alte_etage);
        }
        let id = pfad_teil(&s.spieler);
        let info = Bildschirm {
            spieler: s.spieler.clone(),
            name: s.name.clone(),
            etage: s.etage.clone(),
            stream_url: format!("{PFAD}/bildschirm/{id}/stream"),
            bild_url: format!("{PFAD}/bildschirm/{id}/bild"),
            seit_ms: jetzt,
        };
        t.insert(
            s.spieler.clone(),
            Teilung {
                info: info.clone(),
                bild: watch::channel(None).0,
                letztes_ms: jetzt,
            },
        );
        drop(t);
        let _ = self.aenderungen.send(s.etage.clone());
        info
    }

    /// Ein neues Bild. Startet das Teilen, falls nötig.
    pub fn bild(&self, s: &Sitzung, jpeg: Bytes) -> Result<(), String> {
        if jpeg.len() > MAX_BILD {
            return Err("Das Bild ist zu groß".into());
        }
        if !jpeg.starts_with(&[0xFF, 0xD8]) {
            return Err("Das ist kein JPEG-Bild".into());
        }
        if !self.teilungen.lock().unwrap().contains_key(&s.spieler) {
            self.starten(s);
        }
        let mut t = self.teilungen.lock().unwrap();
        let teilung = t.get_mut(&s.spieler).ok_or("Teilen wurde gerade beendet")?;
        teilung.letztes_ms = jetzt_ms();
        teilung.bild.send_replace(Some(jpeg));
        Ok(())
    }

    /// Hört auf zu teilen. `true`, wenn geteilt wurde. Der Server ruft das auch, wenn jemand geht.
    pub fn beenden(&self, spieler: &str) -> bool {
        let weg = self.teilungen.lock().unwrap().remove(spieler);
        match weg {
            Some(t) => {
                let _ = self.aenderungen.send(t.info.etage);
                true
            }
            None => false,
        }
    }

    /// Beendet alle, von denen zu lange kein Bild kam.
    pub fn aufraeumen(&self, jetzt: u64) -> Vec<String> {
        let mut t = self.teilungen.lock().unwrap();
        let alt: Vec<String> = t
            .iter()
            .filter(|(_, v)| jetzt.saturating_sub(v.letztes_ms) > STILLE_MS)
            .map(|(k, _)| k.clone())
            .collect();
        let mut etagen = Vec::new();
        for k in &alt {
            if let Some(v) = t.remove(k) {
                etagen.push(v.info.etage);
            }
        }
        drop(t);
        for e in &etagen {
            let _ = self.aenderungen.send(e.clone());
        }
        alt
    }

    /// Wer teilt (auf einer Etage oder überall), nach Startzeit sortiert.
    pub fn liste(&self, etage: Option<&str>) -> Vec<Bildschirm> {
        let t = self.teilungen.lock().unwrap();
        let mut l: Vec<Bildschirm> = t
            .values()
            .filter(|v| etage.is_none_or(|e| v.info.etage == e))
            .map(|v| v.info.clone())
            .collect();
        l.sort_by(|a, b| a.seit_ms.cmp(&b.seit_ms).then(a.spieler.cmp(&b.spieler)));
        l
    }

    /// Zum Zuschauen: der Kanal mit dem jeweils letzten Bild.
    pub fn zuschauen(&self, spieler: &str) -> Option<watch::Receiver<Option<Bytes>>> {
        self.teilungen
            .lock()
            .unwrap()
            .get(spieler)
            .map(|t| t.bild.subscribe())
    }

    /// Das letzte Bild.
    pub fn letztes_bild(&self, spieler: &str) -> Option<Bytes> {
        self.teilungen
            .lock()
            .unwrap()
            .get(spieler)
            .and_then(|t| t.bild.borrow().clone())
    }

    /// Meldet jede Etage, deren Liste sich geändert hat. Der Server schickt dann
    /// `medien::Server::Bildschirme { liste: liste(Some(etage)) }` an alle auf der Etage.
    pub fn aenderungen(&self) -> broadcast::Receiver<String> {
        self.aenderungen.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sitzung(id: &str, etage: &str) -> Sitzung {
        Sitzung {
            spieler: id.into(),
            name: format!("Name {id}"),
            etage: etage.into(),
        }
    }

    #[test]
    fn starten_bild_beenden() {
        let b = Bildschirme::default();
        let mut aen = b.aenderungen();
        let info = b.starten(&sitzung("p1", "e1"));
        assert_eq!(info.stream_url, "/medien/bildschirm/p1/stream");
        assert_eq!(aen.try_recv().unwrap(), "e1");
        let mut rx = b.zuschauen("p1").unwrap();
        assert!(
            b.bild(&sitzung("p1", "e1"), Bytes::from_static(b"kein jpeg"))
                .is_err()
        );
        b.bild(
            &sitzung("p1", "e1"),
            Bytes::from_static(&[0xFF, 0xD8, 1, 2]),
        )
        .unwrap();
        assert!(rx.has_changed().unwrap());
        assert_eq!(
            rx.borrow_and_update().as_deref(),
            Some(&[0xFF, 0xD8, 1, 2][..])
        );
        assert_eq!(b.liste(Some("e1")).len(), 1);
        assert!(b.liste(Some("e2")).is_empty());
        assert!(b.beenden("p1"));
        assert!(rx.has_changed().is_err(), "Stream endet mit dem Teilen");
        assert!(!b.beenden("p1"));
    }

    #[test]
    fn stille_beendet() {
        let b = Bildschirme::default();
        b.starten(&sitzung("p1", "e1"));
        assert!(b.aufraeumen(jetzt_ms()).is_empty());
        assert_eq!(
            b.aufraeumen(jetzt_ms() + STILLE_MS + 1),
            vec!["p1".to_string()]
        );
        assert!(b.liste(None).is_empty());
    }

    #[test]
    fn etagenwechsel() {
        let b = Bildschirme::default();
        b.starten(&sitzung("p1", "e1"));
        b.starten(&sitzung("p1", "e2"));
        assert!(b.liste(Some("e1")).is_empty());
        assert_eq!(b.liste(Some("e2")).len(), 1);
    }
}
