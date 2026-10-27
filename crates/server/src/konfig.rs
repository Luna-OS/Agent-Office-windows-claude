//! Kommandozeile und Umgebungsvariablen: wie das Office gestartet wird.

use std::env;
use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};

/// Alles, was beim Start festgelegt wird.
#[derive(Debug, Clone)]
pub struct Konfig {
    pub host: IpAddr,
    pub port: u16,
    /// Gemeinsames Office-Passwort von der Kommandozeile/Umgebung; ohne gilt das gespeicherte.
    pub passwort: Option<String>,
    /// Datenordner (Konten, Etagen, Spielzeug-Zustände).
    pub daten: PathBuf,
    /// Wohin neue Etagen geklont werden; `None` = Standard unter dem Datenordner.
    pub projekte: Option<PathBuf>,
    /// Der gebaute WASM-Client (Trunk-Ausgabe).
    pub client: PathBuf,
    /// Das Projekt, in dem das Office gestartet wurde (wird die erste Etage).
    pub projekt_ordner: Option<PathBuf>,
    /// Ein Repository, das beim Start geklont wird, wenn es noch keine Etage gibt.
    pub projekt_repo: Option<String>,
    pub oeffnen: bool,
    /// Hinter einem Proxy: Client-IP aus X-Forwarded-For, Cookies mit Secure.
    pub proxy_vertrauen: bool,
    pub passwort_zuruecksetzen: bool,
    pub beanspruch_token: Option<String>,
    pub max_arbeiter: Option<u32>,
    pub stadt: Option<String>,
    pub selbstupdate: bool,
}

/// Was die Kommandozeile verlangt.
#[derive(Debug)]
pub enum Befehl {
    Starten(Box<Konfig>),
    Hilfe,
    Version,
    /// `konten …`: Konten ohne laufendes Office verwalten.
    Konten { daten: PathBuf, argumente: Vec<String> },
}

pub const HILFE: &str = "\
OpenAgenticOffice – ein 3D-Büro für deine KI-Arbeiter

Aufruf:
  open-agentic-office [OPTIONEN] [PROJEKTORDNER]
  open-agentic-office konten <liste | einladen [NAME] [--admin] | entziehen NAME |
                             rolle NAME admin|mitglied | passwort an|aus> [--daten ORDNER]

Optionen:
  --port PORT               Port (Umgebung: PORT, Standard 4600)
  --host ADRESSE            Adresse zum Lauschen (OFFICE_HOST, Standard 127.0.0.1)
  --passwort PASSWORT       Gemeinsames Office-Passwort (OFFICE_PASSWORT)
  --passwort-zuruecksetzen  Ein neues gemeinsames Passwort erzeugen und anzeigen
  --daten ORDNER            Datenordner (OFFICE_HOME, Standard ~/open-agentic-office)
  --projekte ORDNER         Wohin neue Etagen geklont werden (OFFICE_PROJEKTE)
  --projekt OWNER/NAME      Dieses Repository beim Start als erste Etage klonen
  --client ORDNER           Ordner des gebauten Browser-Clients (OFFICE_CLIENT)
  --no-open                 Den Browser nicht öffnen (OFFICE_NO_OPEN=1)
  --proxy-vertrauen         Hinter einem Reverse-Proxy (X-Forwarded-*, sichere Cookies)
  --beanspruch-token TOKEN  Einmal-Link /beanspruchen?t=TOKEN zeigt das Passwort (OFFICE_BEANSPRUCH_TOKEN)
  --max-arbeiter ANZAHL     Höchstens so viele Arbeiter gleichzeitig
  --stadt STADT             Stadt für Sonne und Wetter (OFFICE_STADT)
  -h, --hilfe               Diese Hilfe
  -V, --version             Version
";

fn umgebung(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn an(name: &str) -> bool {
    umgebung(name).is_some_and(|v| v != "0")
}

/// Der Home-Ordner des Nutzers.
pub fn home() -> PathBuf {
    env::var_os("USERPROFILE").or_else(|| env::var_os("HOME")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn standard_daten() -> PathBuf {
    umgebung("OFFICE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join("open-agentic-office"))
}

/// Wo der gebaute Client liegt: `--client`, `OFFICE_CLIENT`, `dist/` neben der Binärdatei oder im Arbeitsordner.
fn standard_client() -> PathBuf {
    if let Some(p) = umgebung("OFFICE_CLIENT") {
        return PathBuf::from(p);
    }
    let mut kandidaten = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        kandidaten.push(dir.join("dist"));
    }
    kandidaten.push(PathBuf::from("dist"));
    kandidaten.push(PathBuf::from("crates/client/dist"));
    kandidaten.iter().find(|p| p.join("index.html").is_file()).cloned().unwrap_or_else(|| PathBuf::from("dist"))
}

/// Liest die Argumente (ohne den Programmnamen).
pub fn lesen(args: impl IntoIterator<Item = String>) -> Result<Befehl> {
    let mut args: Vec<String> = args.into_iter().collect();
    if args.first().map(String::as_str) == Some("konten") {
        args.remove(0);
        let mut daten = standard_daten();
        let mut rest = Vec::new();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            if a == "--daten" || a == "--home" {
                daten = PathBuf::from(it.next().ok_or_else(|| anyhow!("--daten braucht einen Ordner"))?);
            } else {
                rest.push(a);
            }
        }
        return Ok(Befehl::Konten { daten, argumente: rest });
    }

    let mut k = Konfig {
        host: umgebung("OFFICE_HOST").map(|h| h.parse()).transpose().context("OFFICE_HOST ist keine IP-Adresse")?.unwrap_or(IpAddr::from([127, 0, 0, 1])),
        port: umgebung("PORT").map(|p| p.parse()).transpose().context("PORT ist keine Zahl")?.unwrap_or(4600),
        passwort: umgebung("OFFICE_PASSWORT"),
        daten: standard_daten(),
        projekte: umgebung("OFFICE_PROJEKTE").map(PathBuf::from),
        client: standard_client(),
        projekt_ordner: None,
        projekt_repo: None,
        oeffnen: !an("OFFICE_NO_OPEN"),
        proxy_vertrauen: an("OFFICE_PROXY_VERTRAUEN"),
        passwort_zuruecksetzen: false,
        beanspruch_token: umgebung("OFFICE_BEANSPRUCH_TOKEN"),
        max_arbeiter: None,
        stadt: umgebung("OFFICE_STADT"),
        selbstupdate: an("OFFICE_SELBSTUPDATE"),
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut wert = |name: &str| it.next().ok_or_else(|| anyhow!("{name} braucht einen Wert"));
        match a.as_str() {
            "-h" | "--hilfe" | "--help" => return Ok(Befehl::Hilfe),
            "-V" | "--version" => return Ok(Befehl::Version),
            "--port" => k.port = wert("--port")?.parse().context("--port braucht eine Zahl")?,
            "--host" => k.host = wert("--host")?.parse().context("--host braucht eine IP-Adresse")?,
            "--passwort" | "--password" => k.passwort = Some(wert("--passwort")?),
            "--passwort-zuruecksetzen" | "--reset-password" => k.passwort_zuruecksetzen = true,
            "--daten" | "--home" => k.daten = PathBuf::from(wert("--daten")?),
            "--projekte" | "--projects" => k.projekte = Some(PathBuf::from(wert("--projekte")?)),
            "--projekt" => k.projekt_repo = Some(wert("--projekt")?),
            "--client" => k.client = PathBuf::from(wert("--client")?),
            "--no-open" | "--nicht-oeffnen" => k.oeffnen = false,
            "--proxy-vertrauen" | "--trust-proxy" => k.proxy_vertrauen = true,
            "--beanspruch-token" => k.beanspruch_token = Some(wert("--beanspruch-token")?),
            "--max-arbeiter" => k.max_arbeiter = Some(wert("--max-arbeiter")?.parse().context("--max-arbeiter braucht eine Zahl")?),
            "--stadt" => k.stadt = Some(wert("--stadt")?),
            s if s.starts_with('-') => bail!("Unbekannte Option {s} (siehe --hilfe)"),
            ordner => {
                if k.projekt_ordner.is_some() {
                    bail!("Nur ein Projektordner geht: {ordner}");
                }
                k.projekt_ordner = Some(PathBuf::from(ordner));
            }
        }
    }
    Ok(Befehl::Starten(Box::new(k)))
}

impl Konfig {
    /// Nur auf diesem Rechner erreichbar?
    pub fn nur_lokal(&self) -> bool {
        self.host.is_loopback()
    }

    /// Eine Konfiguration für Tests, mit Datenordner `daten`.
    pub fn fuer_test(daten: PathBuf) -> Self {
        let mut k = match lesen(Vec::new()).expect("leere Argumente") {
            Befehl::Starten(k) => *k,
            _ => unreachable!(),
        };
        k.daten = daten;
        k.oeffnen = false;
        k.port = 0;
        k.passwort = None;
        k.client = PathBuf::from("gibt-es-nicht");
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starten(a: &[&str]) -> Konfig {
        match lesen(a.iter().map(|s| s.to_string())).unwrap() {
            Befehl::Starten(k) => *k,
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn flags_und_ordner() {
        let k = starten(&["--port", "4700", "--no-open", "--passwort", "geheim", "projekt"]);
        assert_eq!(k.port, 4700);
        assert!(!k.oeffnen);
        assert_eq!(k.passwort.as_deref(), Some("geheim"));
        assert_eq!(k.projekt_ordner, Some(PathBuf::from("projekt")));
    }

    #[test]
    fn unbekannt_ist_fehler() {
        assert!(lesen(["--quatsch".to_string()]).is_err());
    }

    #[test]
    fn konten_unterbefehl() {
        match lesen(["konten", "liste", "--daten", "x"].map(String::from)).unwrap() {
            Befehl::Konten { daten, argumente } => {
                assert_eq!(daten, PathBuf::from("x"));
                assert_eq!(argumente, vec!["liste"]);
            }
            b => panic!("{b:?}"),
        }
    }
}
