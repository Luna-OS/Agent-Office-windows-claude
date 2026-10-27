//! Arbeiter-Verwaltung: PTYs, Git-Worktrees, Agent-Provider (Claude, Codex, ...), Hook-Server und
//! die Kommunikation zwischen Agenten.
//!
//! Der Server (atlas) hält ein [`Agenten`], meldet Etagen mit [`Agenten::etage_anmelden`] an,
//! reicht Browser-Nachrichten an [`Agenten::nachricht`] weiter und verteilt die [`Ereignis`]se aus
//! [`Agenten::ereignisse`] an die WebSocket-Verbindungen. Die Agenten selbst reden über den
//! Hook-Server auf 127.0.0.1 (siehe `server.rs`) und die CLI `office-agenten` mit dem Office.

pub mod board;
pub mod bus;
pub mod provider;
pub mod pty;
pub mod terminal;
pub mod worktree;

mod aenderungen;
mod anleitung;
mod arbeiter;
mod browser;
mod dienste;
mod etage;
mod github;
mod hooks;
mod kommunikation;
mod cli_api;
mod server;
mod speicher;
mod nach_merge;
mod nutzung;
mod prompts;
mod start;
mod steuerung;
mod verbrauch;
mod wache;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use protokoll::{ClientNachricht, ServerNachricht};
use tokio::sync::broadcast;

use crate::etage::Etage;

/// Wie das Agenten-Modul startet.
#[derive(Debug, Clone, Default)]
pub struct AgentenEinstellungen {
    /// Wo `etagen/<etage>/arbeiter.json`, Bus, Board und Hook-Einstellungen liegen.
    pub daten_verzeichnis: PathBuf,
    /// Port des Hook-Servers auf 127.0.0.1; 0 nimmt den vom letzten Start oder einen freien.
    pub hook_port: u16,
    /// Pfad zur CLI `office-agenten`; ohne: neben dem laufenden Programm.
    pub cli_pfad: Option<PathBuf>,
}

/// Wer eine Browser-Nachricht geschickt hat.
#[derive(Debug, Clone, Default)]
pub struct Absender {
    /// ID der WebSocket-Verbindung.
    pub verbindung: String,
    /// Anzeigename des Menschen.
    pub name: String,
    /// Die Etage, auf der er steht (ohne: die Standard-Etage).
    pub etage: Option<String>,
    pub konto: Option<String>,
}

/// An wen ein Ereignis geht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ziel {
    /// Alle Verbindungen auf dieser Etage.
    Etage(String),
    Verbindung(String),
    Verbindungen(Vec<String>),
    Alle,
}

/// Eine Nachricht an Browser.
#[derive(Debug, Clone)]
pub struct Ereignis {
    pub ziel: Ziel,
    pub nachricht: ServerNachricht,
}

/// Rückruf: Konto → Umgebungsvariablen für seine Arbeiter.
pub type KontoUmgebung = Arc<dyn Fn(&str) -> Vec<(String, String)> + Send + Sync>;

/// Die Etage für Absender ohne Etage.
pub const STANDARD_ETAGE: &str = "haupt";

/// Das Agenten-Modul. Günstig zu klonen.
#[derive(Clone)]
pub struct Agenten {
    pub(crate) innen: Arc<Innen>,
}

pub(crate) struct Innen {
    zustand: Mutex<Zustand>,
    sender: broadcast::Sender<Ereignis>,
    pub(crate) daten: PathBuf,
    /// `http://127.0.0.1:<port>`
    pub(crate) hook_url: String,
    /// Pfad zur CLI.
    pub(crate) cli: PathBuf,
    pub(crate) laufzeit: tokio::runtime::Handle,
    beenden: tokio::sync::Notify,
    selbst: std::sync::Weak<Innen>,
    pub(crate) konto_umgebung: Mutex<Option<KontoUmgebung>>,
}

#[derive(Default)]
pub(crate) struct Zustand {
    pub(crate) etagen: HashMap<String, Etage>,
    /// Das Office fährt herunter: endende Prozesse sind gestoppt, nicht abgestürzt.
    pub(crate) schliesst: bool,
    pub(crate) nach_merge: nach_merge::NachMerge,
    pub(crate) nutzung: protokoll::agenten::Nutzung,
    pub(crate) limits: protokoll::agenten::Limits,
    /// Wann die Limits zuletzt gefragt wurden (ms).
    pub(crate) limits_gefragt: u64,
    /// Die Limits werden regelmäßig gelesen (erst, sobald jemand da ist).
    pub(crate) limits_laufen: bool,
    /// Umgeschriebene Prompts (ID → Text).
    pub(crate) prompts: std::collections::HashMap<String, String>,
}

pub(crate) fn jetzt_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub(crate) fn zufall_hex(laenge: usize) -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    (0..laenge).map(|_| char::from_digit(rng.random_range(0..16), 16).unwrap_or('0')).collect()
}

impl Innen {
    pub(crate) fn zustand(&self) -> MutexGuard<'_, Zustand> {
        self.zustand.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn senden(&self, ziel: Ziel, nachricht: ServerNachricht) {
        let _ = self.sender.send(Ereignis { ziel, nachricht });
    }

    pub(crate) fn selbst(&self) -> std::sync::Weak<Innen> {
        self.selbst.clone()
    }

    /// Die Umgebung eines Kontos für seine Arbeiter (leer ohne Konto oder Rückruf).
    pub(crate) fn umgebung_von(&self, konto: Option<&str>) -> Vec<(String, String)> {
        let f = self.konto_umgebung.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match (konto, f) {
            (Some(k), Some(f)) => f(k),
            _ => Vec::new(),
        }
    }

    pub(crate) fn fehler(&self, verbindung: &str, text: impl Into<String>) {
        self.senden(Ziel::Verbindung(verbindung.to_string()), ServerNachricht::Fehler { text: text.into() });
    }
}

impl Agenten {
    /// Startet das Modul samt Hook-Server auf 127.0.0.1.
    pub async fn starten(e: AgentenEinstellungen) -> anyhow::Result<Agenten> {
        std::fs::create_dir_all(&e.daten_verzeichnis)?;
        let (listener, port) = server::lauschen(&e.daten_verzeichnis, e.hook_port).await?;
        let cli = e.cli_pfad.clone().unwrap_or_else(cli_neben_programm);
        let (sender, _) = broadcast::channel(4096);
        let innen = Arc::new_cyclic(|selbst| Innen {
            zustand: Mutex::new(Zustand {
                nach_merge: nach_merge::laden(&e.daten_verzeichnis),
                nutzung: nutzung::laden(&e.daten_verzeichnis),
                prompts: prompts::laden(&e.daten_verzeichnis),
                ..Default::default()
            }),
            sender,
            daten: e.daten_verzeichnis.clone(),
            hook_url: format!("http://127.0.0.1:{port}"),
            cli,
            laufzeit: tokio::runtime::Handle::current(),
            beenden: tokio::sync::Notify::new(),
            selbst: selbst.clone(),
            konto_umgebung: Mutex::new(None),
        });
        let agenten = Agenten { innen };
        server::hook_server_starten(agenten.clone(), listener);
        wache::starten(&agenten);
        dienste::starten(&agenten);
        tracing::info!("Hook-Server lauscht auf {}", agenten.innen.hook_url);
        Ok(agenten)
    }

    /// Die Adresse des Hook-Servers (`http://127.0.0.1:<port>`).
    pub fn hook_url(&self) -> &str {
        &self.innen.hook_url
    }

    /// Eine Etage (ein Projekt) ist da: ihre gespeicherten Arbeiter, Bus und Board werden geladen.
    pub fn etage_anmelden(&self, etage: &str, projekt_verzeichnis: PathBuf) {
        etage::anmelden(self, etage, projekt_verzeichnis);
    }

    /// Die Tische (Sitzplätze) einer Etage, damit Agenten selbst einen freien finden können.
    pub fn tische_setzen(&self, etage: &str, tische: Vec<String>) {
        if let Some(e) = self.innen.zustand().etagen.get_mut(etage) {
            e.tische = tische;
        }
    }

    /// Eine Etage geht: ihre Arbeiter werden gestoppt und gespeichert.
    pub fn etage_abmelden(&self, etage: &str) {
        etage::abmelden(self, etage);
    }

    /// Der Ereignis-Strom für die Browser.
    pub fn ereignisse(&self) -> broadcast::Receiver<Ereignis> {
        self.innen.sender.subscribe()
    }

    /// Eine Nachricht aus dem Browser. Domänen, die nicht zu den Agenten gehören, werden ignoriert.
    pub async fn nachricht(&self, von: &Absender, n: ClientNachricht) {
        browser::verarbeiten(self, von, n).await;
    }

    /// Was fürs ganze Gebäude gilt, einmal beim Verbinden (Provider, Einstellungen).
    pub fn begruessung(&self, _von: &Absender) -> Vec<ServerNachricht> {
        browser::begruessung(self)
    }

    /// Alles einer Etage (Arbeiter, Bus, Feed, Board, GitHub), bei jedem Betreten.
    pub fn etage_betreten(&self, von: &Absender) -> Vec<ServerNachricht> {
        browser::etage_betreten(self, von)
    }

    /// Zahlen für die Aufzugtafel: (Arbeiter, beschäftigt, wartend auf einen Menschen).
    pub fn zahlen(&self, etage: &str) -> (usize, usize, usize) {
        use protokoll::arbeiter::ArbeiterStatus as S;
        let z = self.innen.zustand();
        let Some(e) = z.etagen.get(etage) else { return (0, 0, 0) };
        let status = || e.arbeiter.values().map(|a| a.info.status);
        (
            e.arbeiter.len(),
            status().filter(|s| matches!(s, S::Startet | S::Arbeitet)).count(),
            status().filter(|s| *s == S::Wartet).count(),
        )
    }

    /// Wie ein Konto seine Anmeldungen an Arbeiter weitergibt: `f(konto)` liefert Umgebungsvariablen
    /// (z. B. `GH_CONFIG_DIR`, `CLAUDE_CONFIG_DIR`), die jeder Arbeiter dieses Kontos beim Start bekommt.
    pub fn konto_umgebung_setzen(&self, f: KontoUmgebung) {
        *self.innen.konto_umgebung.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }

    /// Eine Browser-Verbindung ist weg: sie schaut in kein Terminal mehr.
    pub fn verbindung_weg(&self, verbindung: &str) {
        steuerung::verbindung_weg(self, verbindung);
    }

    /// HTTP-Routen für den Hauptserver (unter `/api/agenten/...`).
    pub fn router(&self) -> axum::Router {
        server::haupt_router(self.clone())
    }

    /// Beim Herunterfahren: alles speichern, Terminals beenden.
    pub async fn beenden(&self) {
        let ids: Vec<String> = {
            let mut z = self.innen.zustand();
            z.schliesst = true;
            z.etagen.keys().cloned().collect()
        };
        for id in ids {
            etage::abmelden(self, &id);
        }
        self.innen.beenden.notify_waiters();
    }
}

/// `office-agenten(.exe)` im Ordner des laufenden Programms.
fn cli_neben_programm() -> PathBuf {
    let name = if cfg!(windows) { "office-agenten.exe" } else { "office-agenten" };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}
