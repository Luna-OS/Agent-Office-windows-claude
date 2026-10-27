//! Ein Pseudo-Terminal mit einem Prozess darin, über portable-pty (Windows ConPTY, Unix openpty).
//!
//! Die Ausgabe liest ein eigener Thread und reicht sie an `bei_daten`; ein zweiter wartet auf das
//! Ende des Prozesses und meldet den Exit-Code an `bei_ende`.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Was gestartet werden soll.
#[derive(Debug, Clone, Default)]
pub struct Befehl {
    pub programm: PathBuf,
    pub argumente: Vec<String>,
    pub verzeichnis: PathBuf,
    /// Zusätzliche Umgebungsvariablen.
    pub umgebung: HashMap<String, String>,
    /// Umgebungsvariablen, die das Kind nicht erben soll.
    pub entfernen: Vec<String>,
}

/// Schreibt ins Terminal; der Lese-Rückruf bekommt ihn, um Anfragen des Programms zu beantworten
/// (ConPTY fragt beim Start nach der Cursorposition und wartet auf die Antwort).
#[derive(Clone)]
pub struct Antworter(Arc<Mutex<Box<dyn Write + Send>>>);

impl Antworter {
    pub fn schreiben(&self, daten: &[u8]) -> std::io::Result<()> {
        let mut w = self.0.lock().unwrap_or_else(|e| e.into_inner());
        w.write_all(daten)?;
        w.flush()
    }
}

/// Fragt das Programm nach der Cursorposition (DSR 6)?
pub fn fragt_cursor(daten: &[u8]) -> bool {
    daten.windows(4).any(|w| w == b"\x1b[6n")
}

/// Ein laufendes Terminal.
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    schreiber: Antworter,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl Pty {
    /// Startet `befehl` in einem neuen Terminal der Größe `spalten` × `zeilen`.
    pub fn starten(
        befehl: &Befehl,
        spalten: u16,
        zeilen: u16,
        mut bei_daten: impl FnMut(&[u8], &Antworter) + Send + 'static,
        bei_ende: impl FnOnce(Option<i32>) + Send + 'static,
    ) -> Result<Pty> {
        let system = native_pty_system();
        let paar = system
            .openpty(PtySize { rows: zeilen, cols: spalten, pixel_width: 0, pixel_height: 0 })
            .context("Terminal konnte nicht geöffnet werden")?;
        let mut cmd = CommandBuilder::new(befehl.programm.as_os_str());
        cmd.args(befehl.argumente.iter());
        cmd.cwd(befehl.verzeichnis.as_os_str());
        for name in &befehl.entfernen {
            cmd.env_remove(name);
        }
        for (k, v) in &befehl.umgebung {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        let mut kind = paar
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("{} konnte nicht gestartet werden", befehl.programm.display()))?;
        drop(paar.slave);
        let killer = kind.clone_killer();
        let mut leser = paar.master.try_clone_reader().context("Terminal-Ausgabe nicht lesbar")?;
        let schreiber = Antworter(Arc::new(Mutex::new(paar.master.take_writer().context("Terminal-Eingabe nicht beschreibbar")?)));
        let antworter = schreiber.clone();
        std::thread::Builder::new().name("pty-lesen".into()).spawn(move || {
            let mut puffer = [0u8; 16 * 1024];
            loop {
                match leser.read(&mut puffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => bei_daten(&puffer[..n], &antworter),
                }
            }
        })?;
        std::thread::Builder::new().name("pty-warten".into()).spawn(move || {
            let code = kind.wait().ok().map(|s| s.exit_code() as i32);
            bei_ende(code);
        })?;
        Ok(Pty { master: paar.master, schreiber, killer })
    }

    pub fn schreiben(&mut self, daten: &[u8]) -> Result<()> {
        self.schreiber.schreiben(daten)?;
        Ok(())
    }

    pub fn groesse(&self, spalten: u16, zeilen: u16) -> Result<()> {
        self.master.resize(PtySize { rows: zeilen, cols: spalten, pixel_width: 0, pixel_height: 0 })?;
        Ok(())
    }

    /// Beendet den Prozess (das Ende kommt dann über `bei_ende`).
    pub fn beenden(&mut self) {
        let _ = self.killer.kill();
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}

/// Sucht ein Programm im PATH (unter Windows auch mit den Endungen aus PATHEXT).
pub fn programm_finden(name: &str) -> Option<PathBuf> {
    let pfad = Path::new(name);
    if pfad.components().count() > 1 {
        return pfad.is_file().then(|| pfad.to_path_buf());
    }
    let suchpfad = std::env::var_os("PATH")?;
    let endungen: Vec<OsString> = if cfg!(windows) {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        pathext.split(';').filter(|e| !e.is_empty()).map(|e| e.to_ascii_lowercase().into()).collect()
    } else {
        Vec::new()
    };
    for ordner in std::env::split_paths(&suchpfad) {
        if cfg!(windows) {
            // Mit Endung angegeben (`cmd.exe`): genau so suchen.
            let genau = ordner.join(name);
            if pfad.extension().is_some() && genau.is_file() {
                return Some(genau);
            }
            for endung in &endungen {
                let mut datei = ordner.join(name).into_os_string();
                datei.push(endung);
                let datei = PathBuf::from(datei);
                if datei.is_file() {
                    return Some(datei);
                }
            }
        } else {
            let datei = ordner.join(name);
            if datei.is_file() {
                return Some(datei);
            }
        }
    }
    None
}

/// Unter Windows lassen sich `.cmd`/`.bat` nur über cmd.exe starten, das Argumente selbst
/// auslegt: dorthin gibt man besser keinen frei formulierten Prompt mit.
pub fn ist_skript(programm: &Path) -> bool {
    programm
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// Die Login-Shell dieses Rechners, für Arbeiter der Art Shell.
pub fn standard_shell() -> PathBuf {
    if cfg!(windows) {
        programm_finden("pwsh").or_else(|| programm_finden("powershell")).unwrap_or_else(|| PathBuf::from("cmd.exe"))
    } else {
        std::env::var_os("SHELL").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/bin/sh"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Ein echter PTY mit einer Shell: Eingabe hinein, Ausgabe und Ende heraus.
    #[test]
    fn echte_shell() {
        let (programm, argumente) = if cfg!(windows) {
            (PathBuf::from("cmd.exe"), vec!["/Q".to_string(), "/K".to_string(), "echo bereit".to_string()])
        } else {
            (PathBuf::from("/bin/sh"), vec!["-c".to_string(), "echo bereit; read x; echo antwort-$x".to_string()])
        };
        let befehl = Befehl { programm, argumente, verzeichnis: std::env::temp_dir(), ..Default::default() };
        let ausgabe = Arc::new(Mutex::new(Vec::<u8>::new()));
        let (ende_tx, ende_rx) = mpsc::channel();
        let a = ausgabe.clone();
        let lesen = move |d: &[u8], antwort: &Antworter| {
            if fragt_cursor(d) {
                let _ = antwort.schreiben(b"\x1b[1;1R");
            }
            a.lock().unwrap().extend_from_slice(d);
        };
        let mut pty = Pty::starten(&befehl, 80, 24, lesen, move |c| {
            let _ = ende_tx.send(c);
        })
        .expect("PTY startet");
        let warte_auf = |text: &str| {
            for _ in 0..100 {
                if String::from_utf8_lossy(&ausgabe.lock().unwrap()).contains(text) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            false
        };
        assert!(warte_auf("bereit"), "Shell meldet sich nicht");
        if cfg!(windows) {
            pty.schreiben(b"echo antwort-42\r\n").unwrap();
        } else {
            pty.schreiben(b"42\n").unwrap();
        }
        assert!(warte_auf("antwort-42"), "Eingabe kommt nicht an");
        if cfg!(windows) {
            pty.schreiben(b"exit\r\n").unwrap();
        }
        let code = ende_rx.recv_timeout(Duration::from_secs(10)).expect("Prozess endet");
        assert_eq!(code, Some(0));
    }

    #[test]
    fn findet_programme() {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(programm_finden(shell).is_some());
        if cfg!(windows) {
            assert!(programm_finden("cmd.exe").is_some());
        }
        assert!(programm_finden("gibt-es-sicher-nicht-12345").is_none());
    }
}
