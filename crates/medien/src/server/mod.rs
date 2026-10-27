//! Die Server-Seite von `medien` (Feature `server`): eine `Medien`-Fassade, die der Server einbindet.
//!
//! - `router()` unter `/medien` einhängen; davor muss die Anmeldeprüfung laufen und einen
//!   [`Absender`] in die Request-Extensions legen (ohne ihn antworten alle Routen mit 401).
//! - WS-Nachrichten der Domänen `medien` und `dach` an `nachricht_medien`/`nachricht_dach` geben.
//! - Was verschickt werden soll, kommt als [`Ereignis`] aus `ereignisse()`.

mod bildschirme;
mod routen;
mod speicher;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use protokoll::ServerNachricht;
use protokoll::dach::{Client as DachClient, Server as DachServer};
use protokoll::medien::{Client as MedienClient, IceServer, Server as MedienServer};
use tokio::sync::broadcast;

pub use bildschirme::{Bildschirme, MAX_BILD, STILLE_MS};
pub use routen::{GRENZE, mjpeg_teil};

use crate::jukebox::Jukebox;
use crate::leinwand::Leinwand;

/// Unter diesem Pfad hängt der Server `router()` ein.
pub const PFAD: &str = "/medien";

/// Wer eine Nachricht schickt oder eine Route aufruft (vom Server gefüllt).
#[derive(Debug, Clone, PartialEq)]
pub struct Absender {
    /// Die Verbindungs-/Spieler-ID, die auch die anderen Browser kennen (Ziel für RTC).
    pub verbindung: String,
    pub name: String,
    /// Die Etage, auf der er ist (`None` in der Lobby).
    pub etage: Option<String>,
    pub konto: Option<String>,
    /// Steht er gerade auf dem Dach?
    pub auf_dach: bool,
}

/// Wohin ein Ereignis geht.
#[derive(Debug, Clone, PartialEq)]
pub enum Ziel {
    Etage(String),
    Verbindung(String),
    Alle,
    /// Alle, die gerade auf dem Dach stehen.
    Dach,
}

/// Eine Nachricht, die der Server verschicken soll.
#[derive(Debug, Clone)]
pub struct Ereignis {
    pub ziel: Ziel,
    pub nachricht: ServerNachricht,
}

/// Wer teilt (für die Bildschirm-Verwaltung).
#[derive(Debug, Clone, PartialEq)]
pub struct Sitzung {
    pub spieler: String,
    pub name: String,
    pub etage: String,
}

/// Einstellungen beim Start.
#[derive(Debug, Clone)]
pub struct MedienEinstellungen {
    /// Hier landen `jukebox-<etage>.json` und `leinwand.json` (Unterordner `medien`).
    pub datenordner: PathBuf,
    /// STUN/TURN-Server für Voice; leer heißt: Standard plus `OFFICE_TURN`.
    pub ice: Vec<IceServer>,
}

struct Innen {
    bildschirme: Bildschirme,
    leinwand: Mutex<Leinwand>,
    jukeboxen: Mutex<HashMap<String, Jukebox>>,
    ereignisse: broadcast::Sender<Ereignis>,
    speicher: speicher::Speicher,
    ice: Vec<IceServer>,
}

/// Bildschirm teilen, Leinwand, Jukebox und Voice-Signalisierung. Billig zu klonen.
#[derive(Clone)]
pub struct Medien(Arc<Innen>);

impl Medien {
    pub fn neu(e: MedienEinstellungen) -> Self {
        let speicher = speicher::Speicher::neu(e.datenordner.join("medien"));
        let ice = if e.ice.is_empty() { ice_aus_umgebung() } else { e.ice };
        let leinwand = Mutex::new(speicher.leinwand_laden());
        Self(Arc::new(Innen {
            bildschirme: Bildschirme::default(),
            leinwand,
            jukeboxen: Mutex::new(HashMap::new()),
            ereignisse: broadcast::channel(256).0,
            speicher,
            ice,
        }))
    }

    /// Die HTTP-Routen (unter [`PFAD`] einhängen). Startet auch das Aufräumen stiller Teilungen
    /// und das Melden geänderter Bildschirm-Listen, braucht also eine laufende tokio-Laufzeit.
    pub fn router(&self) -> axum::Router {
        let m = self.clone();
        tokio::spawn(async move {
            let mut aen = m.0.bildschirme.aenderungen();
            let mut takt = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = takt.tick() => { m.0.bildschirme.aufraeumen(jetzt_ms()); }
                    e = aen.recv() => match e {
                        Ok(etage) => m.bildschirme_melden(&etage),
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    },
                }
            }
        });
        routen::routen(self.clone())
    }

    pub fn bildschirme(&self) -> &Bildschirme {
        &self.0.bildschirme
    }

    /// Alles, was verschickt werden soll.
    pub fn ereignisse(&self) -> broadcast::Receiver<Ereignis> {
        self.0.ereignisse.subscribe()
    }

    fn senden(&self, ziel: Ziel, nachricht: ServerNachricht) {
        let _ = self.0.ereignisse.send(Ereignis { ziel, nachricht });
    }

    fn bildschirme_melden(&self, etage: &str) {
        let liste = self.0.bildschirme.liste(Some(etage));
        self.senden(Ziel::Etage(etage.into()), ServerNachricht::Medien(MedienServer::Bildschirme { liste }));
    }

    /// Nach der Anmeldung: ICE-Server.
    pub fn begruessung(&self, _von: &Absender) -> Vec<ServerNachricht> {
        vec![ServerNachricht::Medien(MedienServer::IceServer { server: self.0.ice.clone() })]
    }

    /// Beim Betreten einer Etage (oder des Dachs): wer teilt, was die Jukebox spielt, die Leinwand.
    pub fn etage_betreten(&self, von: &Absender) -> Vec<ServerNachricht> {
        let mut aus = Vec::new();
        if let Some(etage) = &von.etage {
            let liste = self.0.bildschirme.liste(Some(etage));
            aus.push(ServerNachricht::Medien(MedienServer::Bildschirme { liste }));
            let zustand = self.mit_jukebox(etage, |j| j.zustand(jetzt_ms()));
            aus.push(ServerNachricht::Medien(MedienServer::Jukebox { zustand }));
        }
        if von.auf_dach {
            let zustand = self.0.leinwand.lock().unwrap().zustand().clone();
            aus.push(ServerNachricht::Dach(DachServer::Leinwand { zustand }));
        }
        aus
    }

    /// Eine Verbindung ist weg: ihr Teilen endet.
    pub fn verbindung_weg(&self, verbindung: &str) {
        self.0.bildschirme.beenden(verbindung);
    }

    /// Eine Nachricht der Domäne `medien`. `Err` ist eine deutsche Meldung nur für den Absender.
    pub fn nachricht_medien(&self, von: &Absender, n: MedienClient) -> Result<(), String> {
        match n {
            MedienClient::Rtc { an, daten } => {
                let n = MedienServer::Rtc { von: von.verbindung.clone(), daten };
                self.senden(Ziel::Verbindung(an), ServerNachricht::Medien(n));
            }
            MedienClient::Sprache { aktiv, stumm } => {
                let n = ServerNachricht::Medien(MedienServer::Sprache { spieler: von.verbindung.clone(), aktiv, stumm });
                self.senden(von.etage.clone().map_or(Ziel::Alle, Ziel::Etage), n);
            }
            MedienClient::JukeboxSpielen { stueck, url } => {
                let etage = etage_von(von)?;
                let jetzt = jetzt_ms();
                let geaendert = self.mit_jukebox(&etage, |j| j.spielen(stueck.as_deref(), url.as_deref(), &von.name, jetzt))?;
                if geaendert {
                    self.jukebox_geaendert(&etage);
                }
            }
            MedienClient::JukeboxWeiter => {
                let etage = etage_von(von)?;
                self.mit_jukebox(&etage, |j| j.weiter(&von.name, jetzt_ms()));
                self.jukebox_geaendert(&etage);
            }
            MedienClient::JukeboxStopp => {
                let etage = etage_von(von)?;
                if self.mit_jukebox(&etage, |j| j.stopp(&von.name)) {
                    self.jukebox_geaendert(&etage);
                }
            }
        }
        Ok(())
    }

    /// Eine Nachricht der Domäne `dach` (Leinwand). Nur wer auf dem Dach steht, darf sie bedienen.
    pub fn nachricht_dach(&self, von: &Absender, n: DachClient) -> Result<(), String> {
        if !von.auf_dach {
            return Err("Die Leinwand lässt sich nur auf dem Dach bedienen".into());
        }
        let zustand = {
            let mut l = self.0.leinwand.lock().unwrap();
            if !l.anwenden(&n, &von.name, jetzt_ms())? {
                return Ok(());
            }
            l.zustand().clone()
        };
        self.0.speicher.leinwand_speichern(&zustand);
        self.senden(Ziel::Dach, ServerNachricht::Dach(DachServer::Leinwand { zustand }));
        Ok(())
    }

    /// Was die Jukebox der Etage spielt, für Hinweise („🎵 Luna hat … aufgelegt“).
    pub fn jukebox_titel(&self, etage: &str) -> String {
        self.mit_jukebox(etage, |j| j.titel())
    }

    fn mit_jukebox<T>(&self, etage: &str, f: impl FnOnce(&mut Jukebox) -> T) -> T {
        let mut alle = self.0.jukeboxen.lock().unwrap();
        let j = alle.entry(etage.to_string()).or_insert_with(|| self.0.speicher.jukebox_laden(etage));
        f(j)
    }

    fn jukebox_geaendert(&self, etage: &str) {
        let (zustand, json) = self.mit_jukebox(etage, |j| (j.zustand(jetzt_ms()), j.als_json()));
        self.0.speicher.jukebox_speichern(etage, &json);
        self.senden(Ziel::Etage(etage.into()), ServerNachricht::Medien(MedienServer::Jukebox { zustand }));
    }
}

fn etage_von(von: &Absender) -> Result<String, String> {
    von.etage.clone().ok_or_else(|| "Bitte zuerst eine Etage betreten".into())
}

/// Server-Uhr in Unix-ms.
pub fn jetzt_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Standard-STUN plus TURN-Server aus `OFFICE_TURN` (kommagetrennt, `turn:nutzer:pass@host:3478`).
pub fn ice_aus_umgebung() -> Vec<IceServer> {
    ice_server(std::env::var("OFFICE_TURN").ok().as_deref().unwrap_or(""))
}

/// Wie [`ice_aus_umgebung`], aber mit der TURN-Liste als Text.
pub fn ice_server(turn: &str) -> Vec<IceServer> {
    let mut l = vec![IceServer {
        urls: vec!["stun:stun.l.google.com:19302".into(), "stun:stun1.l.google.com:19302".into()],
        username: None,
        credential: None,
    }];
    for t in turn.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let (schema, rest) = t.split_once(':').unwrap_or(("turn", t));
        let (zugang, wirt) = match rest.rsplit_once('@') {
            Some((z, w)) => (Some(z), w),
            None => (None, rest),
        };
        let (nutzer, pass) = zugang.and_then(|z| z.split_once(':')).map_or((None, None), |(n, p)| (Some(n), Some(p)));
        l.push(IceServer {
            urls: vec![format!("{schema}:{wirt}")],
            username: nutzer.map(Into::into),
            credential: pass.map(Into::into),
        });
    }
    l
}

/// Eine ID so umschreiben, dass sie gefahrlos in einem URL-Pfad steht.
pub(crate) fn pfad_teil(s: &str) -> String {
    let mut aus = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            aus.push(b as char);
        } else {
            aus.push_str(&format!("%{b:02X}"));
        }
    }
    aus
}

#[cfg(test)]
mod tests;
