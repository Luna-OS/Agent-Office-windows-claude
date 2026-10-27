//! Der PTY-Wirt: ein eigener kleiner Prozess (`office-pty-wirt`), der die Terminals aller Arbeiter
//! besitzt. Startet der Server neu, arbeiten die Agenten im Wirt weiter, und der neue Server holt
//! jedes Terminal dort ab, wo es war. Server und Wirt sprechen zeilenweise JSON über TCP auf
//! 127.0.0.1; nur wer das Token kennt, kommt hinein. Ohne Wirt laufen die Terminals im Server.
//!
//! `<daten>/pty-wirt.json` hält Port, PID und Protokollversion des laufenden Wirts; das Token steht
//! nur in `<daten>/pty-wirt.token` (nur für diesen Benutzer lesbar, wie der Rest des Datenordners).

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use crate::pty::{Antworter, Befehl, Pty};

/// Erhöhen, wenn sich die Nachrichten ändern: ein Server, der einen älteren Wirt findet, beendet ihn.
pub const VERSION: u32 = 1;
pub const INFO_DATEI: &str = "pty-wirt.json";
pub const TOKEN_DATEI: &str = "pty-wirt.token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WirtInfo {
    pub port: u16,
    pub pid: u32,
    pub version: u32,
}

/// Nachrichten an den Wirt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum AnWirt {
    Hallo { token: String, version: u32 },
    Starten { id: String, befehl: Befehl, spalten: u16, zeilen: u16 },
    /// Die Ausgabe einer Sitzung, die schon lief, wieder an diesen Server.
    Anhaengen { id: String },
    Schreiben { id: String, daten: String },
    Groesse { id: String, spalten: u16, zeilen: u16 },
    Beenden { id: String },
    /// Alle Terminals beenden, Wirt beenden.
    Stopp,
}

/// Nachrichten vom Wirt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "typ", rename_all = "snake_case")]
pub enum VomWirt {
    Bereit { version: u32, sitzungen: Vec<String> },
    Gestartet { id: String, pid: Option<u32> },
    Angehaengt { id: String, pid: Option<u32>, spalten: u16, zeilen: u16, bildschirm: String, titel: Option<String>, arbeitet: Option<bool> },
    /// Die Sitzung gibt es nicht (mehr).
    Weg { id: String },
    Daten { id: String, daten: String },
    Ende { id: String, code: Option<i32>, fehler: Option<String> },
}

type BeiDaten = Box<dyn FnMut(&[u8], &Antworter) + Send>;
type BeiEnde = Box<dyn FnOnce(Option<i32>) + Send>;

struct Rueckrufe {
    /// Während des Aufrufs herausgenommen: der Rückruf sperrt den Server-Zustand, und der Server
    /// startet Terminals unter diesem Schloss – nie beide Schlösser in umgekehrter Reihenfolge.
    daten: Option<BeiDaten>,
    ende: Option<BeiEnde>,
    pid: Arc<AtomicU32>,
}

/// Was der Wirt über eine übernommene Sitzung weiß.
#[derive(Debug, Clone)]
pub struct Uebernommen {
    pub spalten: u16,
    pub zeilen: u16,
    pub bildschirm: String,
    pub titel: Option<String>,
    pub arbeitet: Option<bool>,
}

/// Die Verbindung des Servers zum Wirt.
pub struct Wirt {
    tx: mpsc::UnboundedSender<AnWirt>,
    sitzungen: Arc<Mutex<HashMap<String, Rueckrufe>>>,
    wartend: Arc<Mutex<HashMap<String, oneshot::Sender<Option<Uebernommen>>>>>,
    /// Sitzungen, die der Wirt beim Verbinden noch hatte.
    pub vorhanden: Vec<String>,
}

/// Ein Terminal im Wirt, wie es der Server hält.
pub struct WirtPty {
    pub id: String,
    pid: Arc<AtomicU32>,
    tx: mpsc::UnboundedSender<AnWirt>,
}

impl WirtPty {
    pub fn pid(&self) -> Option<u32> {
        Some(self.pid.load(Ordering::Relaxed)).filter(|p| *p != 0)
    }
    pub fn schreiben(&self, daten: &[u8]) {
        let _ = self.tx.send(AnWirt::Schreiben { id: self.id.clone(), daten: String::from_utf8_lossy(daten).into_owned() });
    }
    pub fn groesse(&self, spalten: u16, zeilen: u16) {
        let _ = self.tx.send(AnWirt::Groesse { id: self.id.clone(), spalten, zeilen });
    }
    pub fn beenden(&self) {
        let _ = self.tx.send(AnWirt::Beenden { id: self.id.clone() });
    }
}

fn sperre<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Wirt {
    /// Verbindet sich mit dem laufenden Wirt oder startet einen neuen (`programm`). `None`: ohne Wirt.
    pub async fn verbinden_oder_starten(daten: &Path, programm: &Path) -> Option<Arc<Wirt>> {
        if std::env::var("OFFICE_PTY_WIRT").is_ok_and(|v| v == "aus" || v == "0") || !programm.is_file() {
            return None;
        }
        if let Some(w) = Self::verbinden(daten).await {
            return Some(w);
        }
        let token = crate::zufall_hex(32);
        let _ = std::fs::remove_file(daten.join(INFO_DATEI));
        std::fs::write(daten.join(TOKEN_DATEI), &token).ok()?;
        if let Err(e) = abgekoppelt_starten(programm, daten) {
            tracing::warn!("PTY-Wirt startet nicht, Terminals laufen im Server: {e}");
            return None;
        }
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Some(w) = Self::verbinden(daten).await {
                return Some(w);
            }
        }
        tracing::warn!("PTY-Wirt meldet sich nicht, Terminals laufen im Server");
        None
    }

    async fn verbinden(daten: &Path) -> Option<Arc<Wirt>> {
        let info: WirtInfo = serde_json::from_str(&std::fs::read_to_string(daten.join(INFO_DATEI)).ok()?).ok()?;
        let token = std::fs::read_to_string(daten.join(TOKEN_DATEI)).ok()?;
        let strom = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(("127.0.0.1", info.port))).await.ok()?.ok()?;
        let (lesen, mut schreiben) = strom.into_split();
        let mut zeilen = BufReader::new(lesen).lines();
        let hallo = serde_json::to_string(&AnWirt::Hallo { token: token.trim().to_string(), version: VERSION }).ok()?;
        schreiben.write_all(format!("{hallo}\n").as_bytes()).await.ok()?;
        let antwort = tokio::time::timeout(Duration::from_secs(3), zeilen.next_line()).await.ok()?.ok()??;
        let Ok(VomWirt::Bereit { version, sitzungen }) = serde_json::from_str(&antwort) else { return None };
        if version != VERSION {
            // Ein älterer Wirt: beenden, ein neuer wird gestartet.
            let stopp = serde_json::to_string(&AnWirt::Stopp).ok()?;
            let _ = schreiben.write_all(format!("{stopp}\n").as_bytes()).await;
            return None;
        }
        let (tx, mut rx) = mpsc::unbounded_channel::<AnWirt>();
        tokio::spawn(async move {
            while let Some(n) = rx.recv().await {
                let Ok(text) = serde_json::to_string(&n) else { continue };
                if schreiben.write_all(format!("{text}\n").as_bytes()).await.is_err() {
                    break;
                }
            }
        });
        let wirt = Arc::new(Wirt { tx, sitzungen: Default::default(), wartend: Default::default(), vorhanden: sitzungen });
        let (s, w) = (wirt.sitzungen.clone(), wirt.wartend.clone());
        tokio::spawn(async move {
            while let Ok(Some(z)) = zeilen.next_line().await {
                if let Ok(n) = serde_json::from_str::<VomWirt>(&z) {
                    verteilen(&s, &w, n);
                }
            }
            // Der Wirt ist weg: seine Terminals auch.
            let alle: Vec<Rueckrufe> = sperre(&s).drain().map(|(_, r)| r).collect();
            for r in alle {
                if let Some(ende) = r.ende {
                    ende(None);
                }
            }
        });
        Some(wirt)
    }

    /// Startet ein Terminal im Wirt. Ein Fehler beim Start kommt als Ende über `bei_ende`.
    pub fn starten(
        &self,
        befehl: &Befehl,
        spalten: u16,
        zeilen: u16,
        bei_daten: impl FnMut(&[u8], &Antworter) + Send + 'static,
        bei_ende: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> Pty {
        let id = crate::zufall_hex(12);
        let pid = Arc::new(AtomicU32::new(0));
        sperre(&self.sitzungen).insert(id.clone(), Rueckrufe { daten: Some(Box::new(bei_daten)), ende: Some(Box::new(bei_ende)), pid: pid.clone() });
        let _ = self.tx.send(AnWirt::Starten { id: id.clone(), befehl: befehl.clone(), spalten, zeilen });
        Pty::im_wirt(WirtPty { id, pid, tx: self.tx.clone() })
    }

    /// Übernimmt eine Sitzung, die schon im Wirt läuft. `None`: die gibt es nicht mehr.
    pub async fn anhaengen(
        &self,
        id: &str,
        bei_daten: impl FnMut(&[u8], &Antworter) + Send + 'static,
        bei_ende: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> Option<(Pty, Uebernommen)> {
        let pid = Arc::new(AtomicU32::new(0));
        let (otx, orx) = oneshot::channel();
        sperre(&self.wartend).insert(id.to_string(), otx);
        sperre(&self.sitzungen).insert(id.to_string(), Rueckrufe { daten: Some(Box::new(bei_daten)), ende: Some(Box::new(bei_ende)), pid: pid.clone() });
        let _ = self.tx.send(AnWirt::Anhaengen { id: id.to_string() });
        let ergebnis = tokio::time::timeout(Duration::from_secs(5), orx).await.ok().flatten().flatten();
        match ergebnis {
            Some(u) => Some((Pty::im_wirt(WirtPty { id: id.to_string(), pid, tx: self.tx.clone() }), u)),
            None => {
                sperre(&self.sitzungen).remove(id);
                None
            }
        }
    }

    /// Beendet alle Terminals im Wirt und ihn selbst.
    pub fn stoppen(&self) {
        let _ = self.tx.send(AnWirt::Stopp);
    }
}

fn verteilen(s: &Mutex<HashMap<String, Rueckrufe>>, w: &Mutex<HashMap<String, oneshot::Sender<Option<Uebernommen>>>>, n: VomWirt) {
    match n {
        VomWirt::Daten { id, daten } => {
            let rueckruf = sperre(s).get_mut(&id).and_then(|r| r.daten.take());
            if let Some(mut f) = rueckruf {
                f(daten.as_bytes(), &Antworter::stumm());
                if let Some(r) = sperre(s).get_mut(&id) {
                    r.daten = Some(f);
                }
            }
        }
        VomWirt::Gestartet { id, pid } => {
            if let Some(r) = sperre(s).get(&id) {
                r.pid.store(pid.unwrap_or(0), Ordering::Relaxed);
            }
        }
        VomWirt::Angehaengt { id, pid, spalten, zeilen, bildschirm, titel, arbeitet } => {
            if let Some(r) = sperre(s).get(&id) {
                r.pid.store(pid.unwrap_or(0), Ordering::Relaxed);
            }
            if let Some(tx) = sperre(w).remove(&id) {
                let _ = tx.send(Some(Uebernommen { spalten, zeilen, bildschirm, titel, arbeitet }));
            }
        }
        VomWirt::Weg { id } => {
            sperre(s).remove(&id);
            if let Some(tx) = sperre(w).remove(&id) {
                let _ = tx.send(None);
            }
        }
        VomWirt::Ende { id, code, fehler } => {
            if let Some(f) = fehler {
                tracing::warn!("Terminal {id} startet nicht: {f}");
            }
            let r = sperre(s).remove(&id);
            if let Some(ende) = r.and_then(|r| r.ende) {
                ende(code);
            }
        }
        VomWirt::Bereit { .. } => {}
    }
}

/// Startet den Wirt so, dass er den Server überlebt.
fn abgekoppelt_starten(programm: &Path, daten: &Path) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(programm);
    cmd.arg(daten).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP, möglichst auch aus dem Job des Servers heraus.
        if cmd.creation_flags(0x0000_0008 | 0x0000_0200 | 0x0100_0000).spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}
