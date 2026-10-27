//! Startet den Server (`open-agentic-office`) als Kindprozess auf diesem Computer.

use crate::einstellungen::Einstellungen;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct LaufenderServer {
    kind: Child,
    pub port: u16,
    pub fuer_andere: bool,
    pub passwort: Option<String>,
}

impl LaufenderServer {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn laeuft(&mut self) -> bool {
        matches!(self.kind.try_wait(), Ok(None))
    }
}

impl Drop for LaufenderServer {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

fn programm_ordner() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn dateiname(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Sucht den Server: neben der App, im Unterordner `resources`, sonst im `target`-Ordner des Checkouts.
pub fn binary_finden() -> Option<PathBuf> {
    let name = dateiname("open-agentic-office");
    let ordner = programm_ordner();
    let mut kandidaten = vec![ordner.join(&name), ordner.join("resources").join(&name)];
    for stufe in ["../../..", "../../../.."] {
        for profil in ["release", "debug"] {
            kandidaten.push(ordner.join(stufe).join("target").join(profil).join(&name));
        }
    }
    kandidaten.into_iter().find(|p| p.is_file())
}

/// Ordner des gebauten Browser-Clients (`dist/`), falls er mitgeliefert wird.
fn client_finden() -> Option<PathBuf> {
    let ordner = programm_ordner();
    [
        ordner.join("dist"),
        ordner.join("resources/dist"),
        ordner.join("../../../dist"),
        ordner.join("../../../../dist"),
    ]
    .into_iter()
    .find(|p| p.join("index.html").is_file())
}

pub fn freier_port(ab: u16) -> u16 {
    (ab..ab.saturating_add(100))
        .find(|p| TcpListener::bind(("0.0.0.0", *p)).is_ok())
        .unwrap_or(ab)
}

pub fn zufalls_passwort() -> String {
    let s = RandomState::new();
    let (mut a, mut b) = (s.build_hasher(), s.build_hasher());
    a.write_u8(1);
    b.write_u8(2);
    format!("{:016x}{:016x}", a.finish(), b.finish())
}

/// Adresse dieses Computers im Netzwerk (die Schnittstelle, die ins Netz zeigt).
pub fn netz_adresse() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

pub fn starten(e: &Einstellungen, fuer_andere: bool) -> Result<LaufenderServer, String> {
    let binary = binary_finden()
        .ok_or("Der Server (open-agentic-office) wurde neben der App nicht gefunden.")?;
    let port = freier_port(crate::lobby::STANDARD_PORT);
    let daten = e.daten_ordner();
    let _ = std::fs::create_dir_all(&daten);

    let mut befehl = Command::new(binary);
    befehl
        .args(["--no-open", "--port", &port.to_string(), "--host"])
        .arg(if fuer_andere { "0.0.0.0" } else { "127.0.0.1" })
        .arg("--daten")
        .arg(&daten)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(p) = &e.projekte {
        befehl.arg("--projekte").arg(p);
    }
    if let Some(client) = client_finden() {
        befehl.arg("--client").arg(client);
    }
    // Werkzeuge der App (gh, office-agenten) zuerst im PATH.
    let sep = if cfg!(windows) { ";" } else { ":" };
    let ordner = programm_ordner();
    let alt = std::env::var("PATH").unwrap_or_default();
    befehl.env(
        "PATH",
        format!(
            "{}{sep}{}{sep}{alt}",
            ordner.display(),
            ordner.join("werkzeuge").display()
        ),
    );

    // Lobby für andere: mit eigenem Passwort, über die Umgebung statt der Kommandozeile.
    let passwort = fuer_andere.then(|| e.lobby_passwort.clone().unwrap_or_else(zufalls_passwort));
    if let Some(p) = &passwort {
        befehl.env("OFFICE_PASSWORT", p);
    }
    if e.nur_abo {
        for var in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
        ] {
            befehl.env_remove(var);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        befehl.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let kind = befehl
        .spawn()
        .map_err(|f| format!("Der Server ließ sich nicht starten: {f}"))?;
    Ok(LaufenderServer {
        kind,
        port,
        fuer_andere,
        passwort,
    })
}

/// Wartet, bis der Server auf seinem Port antwortet.
pub fn warten(port: u16, bis: Duration) -> bool {
    let ende = Instant::now() + bis;
    while Instant::now() < ende {
        if TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300))
            .is_ok()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}
