//! Die Kommandozeile des Vermittlers.

pub const HILFE: &str = "\
agent-office-vermittler – vermittelt eine Agent-Office-Lobby zwischen den Spielern

Der Vermittler hat keine Oberfläche und rechnet nichts: Die Lobby läuft im Office eines Spielers
(dem Gastgeber, in seiner App), und der Vermittler reicht die Verbindungen aller anderen zu ihm
durch. Dafür reicht ein kleiner Server, etwa ein Umbrel.

Aufruf:
  agent-office-vermittler [Optionen]

Optionen:
  -H, --host <adresse>    Adresse zum Lauschen (Standard 0.0.0.0: alle Netze)
  -p, --port <port>       Port (Standard 4600)
      --passwort <pw>     Lobby-Passwort, das jedes Gastgeber-Office braucht
                          (oder die Umgebungsvariable VERMITTLER_PASSWORT)
      --hinter-proxy      Hinter einem Proxy (wie dem App-Proxy von Umbrel): die
                          Adresse des Browsers aus X-Forwarded-For nehmen, die der
                          Proxy anhängt (oder VERMITTLER_HINTER_PROXY=1)
  -h, --hilfe             Diese Hilfe
  -V, --version           Version

Der Vermittler selbst antwortet nur unter /vermittler/…: /vermittler/stand (wer Gastgeber ist, als
JSON) und /vermittler/knoten (WebSocket für die Gastgeber-Offices). Alles andere geht zum Gastgeber.
";

#[derive(Debug, Clone, PartialEq)]
pub struct Konfig {
    pub host: String,
    pub port: u16,
    pub passwort: String,
    /// Hinter einem Proxy: Die Adresse des Browsers steht ganz rechts in `X-Forwarded-For`.
    pub hinter_proxy: bool,
}

/// Was aus der Umgebung kommt: `VERMITTLER_PASSWORT` und `VERMITTLER_HINTER_PROXY`.
#[derive(Debug, Default)]
pub struct Umgebung {
    pub passwort: Option<String>,
    pub hinter_proxy: bool,
}

impl Umgebung {
    pub fn lesen() -> Self {
        Self {
            passwort: std::env::var("VERMITTLER_PASSWORT").ok(),
            hinter_proxy: std::env::var("VERMITTLER_HINTER_PROXY")
                .is_ok_and(|v| !matches!(v.trim(), "" | "0" | "nein" | "false")),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Aufruf {
    Starten(Konfig),
    Hilfe,
    Version,
}

/// Liest die Argumente (ohne den Programmnamen); was fehlt, kommt aus der Umgebung.
pub fn lesen(args: &[String], umgebung: Umgebung) -> Result<Aufruf, String> {
    let mut konfig = Konfig {
        host: "0.0.0.0".into(),
        port: 4600,
        passwort: umgebung.passwort.unwrap_or_default(),
        hinter_proxy: umgebung.hinter_proxy,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let (name, eingebettet) = match a.split_once('=') {
            Some((n, w)) if n.starts_with("--") => (n, Some(w.to_string())),
            _ => (a.as_str(), None),
        };
        let mut wert = |was: &str| {
            eingebettet
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{name} braucht {was}"))
        };
        match name {
            "-h" | "--hilfe" | "--help" => return Ok(Aufruf::Hilfe),
            "-V" | "--version" => return Ok(Aufruf::Version),
            "-H" | "--host" => konfig.host = wert("eine Adresse")?,
            "-p" | "--port" => {
                let w = wert("einen Port")?;
                konfig.port = w.parse().map_err(|_| format!("Kein gültiger Port: {w}"))?;
            }
            "--passwort" => konfig.passwort = wert("ein Passwort")?,
            "--hinter-proxy" => konfig.hinter_proxy = true,
            _ => return Err(format!("Unbekannte Option: {a} (siehe --hilfe)")),
        }
    }
    if konfig.passwort.trim().is_empty() {
        return Err(
            "Ohne Lobby-Passwort startet der Vermittler nicht: --passwort <pw> oder VERMITTLER_PASSWORT setzen."
                .into(),
        );
    }
    Ok(Aufruf::Starten(konfig))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn mit_passwort(pw: &str) -> Umgebung {
        Umgebung {
            passwort: Some(pw.into()),
            hinter_proxy: false,
        }
    }

    #[test]
    fn standard_und_optionen() {
        assert_eq!(
            lesen(&args(&["--passwort", "x"]), Umgebung::default()),
            Ok(Aufruf::Starten(Konfig {
                host: "0.0.0.0".into(),
                port: 4600,
                passwort: "x".into(),
                hinter_proxy: false,
            }))
        );
        assert_eq!(
            lesen(
                &args(&["-p", "4700", "--host=127.0.0.1", "--hinter-proxy"]),
                mit_passwort("aus-env")
            ),
            Ok(Aufruf::Starten(Konfig {
                host: "127.0.0.1".into(),
                port: 4700,
                passwort: "aus-env".into(),
                hinter_proxy: true,
            }))
        );
        assert_eq!(
            lesen(&args(&["--hilfe"]), Umgebung::default()),
            Ok(Aufruf::Hilfe)
        );
    }

    #[test]
    fn fehler_auf_deutsch() {
        assert!(
            lesen(&args(&[]), Umgebung::default())
                .unwrap_err()
                .contains("Lobby-Passwort")
        );
        assert!(
            lesen(&args(&["-p", "abc"]), mit_passwort("x"))
                .unwrap_err()
                .contains("Port")
        );
        assert!(
            lesen(&args(&["--quatsch"]), mit_passwort("x"))
                .unwrap_err()
                .contains("Unbekannte")
        );
    }
}
