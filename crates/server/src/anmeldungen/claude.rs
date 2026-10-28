//! Die Anmeldung bei Claude: `claude auth login` läuft in einem PTY; der Link zur Anmeldeseite geht
//! an den Browser, der Code von dort zurück ins PTY.

use std::io::{Read, Write};
use std::time::Duration;

use protokoll::konten::{AnmeldeArt, AnmeldungOffen};

use super::AnmeldungenRef;
use super::ablauf::{ABLAUF_MAX, abbrechen, finden, keine, laeuft, letzte_worte, schlicht};

/// Startet `claude auth login` in einem PTY. Fehlertext, wenn es nicht geht.
pub fn claude_starten(
    a: &AnmeldungenRef,
    konto: &str,
    env: Vec<(String, String)>,
) -> Result<(), String> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    let pfad = finden("claude").ok_or("Claude Code ist auf dem Rechner des Office nicht installiert – füge stattdessen einen Token aus `claude setup-token` ein")?;
    abbrechen(a, konto, AnmeldeArt::Claude);
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 40,
            cols: 4000,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Kein PTY: {e}"))?;
    let mut cmd = CommandBuilder::new(pfad);
    cmd.args(["auth", "login", "--claudeai"]);
    cmd.cwd(a.home(konto));
    for (k, v) in &env {
        if v.is_empty() {
            cmd.env_remove(k)
        } else {
            cmd.env(k, v)
        }
    }
    cmd.env("BROWSER", "true");
    cmd.env("TERM", "xterm-256color");
    let mut kind = pty
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Claudes Anmeldung ließ sich nicht starten: {e}"))?;
    drop(pty.slave);
    let mut leser = pty.master.try_clone_reader().map_err(|e| e.to_string())?;
    let mut schreiber = pty.master.take_writer().map_err(|e| e.to_string())?;
    let mut killer = kind.clone_killer();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    {
        let mut live = a.live.lock().unwrap();
        let l = live.entry(konto.to_string()).or_default();
        l.claude_eingabe = Some(tx);
        l.abbruch_claude = Some(Box::new(move || {
            let _ = killer.kill();
        }));
    }
    a.setzen(
        konto,
        AnmeldeArt::Claude,
        laeuft(AnmeldungOffen::default(), None),
    );

    // Schreiber: Codes von der Anmeldeseite ins PTY.
    std::thread::spawn(move || {
        while let Ok(code) = rx.recv() {
            let _ = schreiber.write_all(format!("{code}\r").as_bytes());
            let _ = schreiber.flush();
        }
    });
    // Leser: Link finden, Fehler nach dem Code erkennen.
    let (a2, k2) = (a.clone(), konto.to_string());
    let ausgabe = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let aus2 = ausgabe.clone();
    std::thread::spawn(move || {
        let mut puffer = [0u8; 4096];
        let mut url: Option<String> = None;
        while let Ok(n) = leser.read(&mut puffer) {
            if n == 0 {
                break;
            }
            let mut aus = aus2.lock().unwrap();
            aus.push_str(&String::from_utf8_lossy(&puffer[..n]));
            if aus.len() > 50_000 {
                let weg = aus.len() - 40_000;
                let bis = aus
                    .char_indices()
                    .find(|(i, _)| *i >= weg)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                aus.drain(..bis);
            }
            if url.is_none() {
                let text = schlicht(&aus);
                if let Some(i) = text.find("https://") {
                    let kandidat: String = text[i..]
                        .chars()
                        .take_while(|c| !c.is_whitespace())
                        .collect();
                    if kandidat.contains("oauth/authorize") {
                        url = Some(kandidat.clone());
                        a2.setzen(
                            &k2,
                            AnmeldeArt::Claude,
                            laeuft(
                                AnmeldungOffen {
                                    url: Some(kandidat),
                                    code: None,
                                    geschickt: false,
                                },
                                None,
                            ),
                        );
                    }
                }
            }
        }
    });
    // Warten aufs Ende (mit Höchstdauer).
    let (a3, k3) = (a.clone(), konto.to_string());
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let ok = loop {
            match kind.try_wait() {
                Ok(Some(s)) => break s.success(),
                Ok(None) if start.elapsed() > ABLAUF_MAX => {
                    let _ = kind.kill();
                    break false;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(300)),
                Err(_) => break false,
            }
        };
        if let Some(l) = a3.live.lock().unwrap().get_mut(&k3) {
            l.claude_eingabe = None;
            l.abbruch_claude = None;
        }
        if ok {
            let mut g = a3.laden(&k3);
            g.claude = None;
            a3.sichern(&k3, &g);
            a3.setzen(&k3, AnmeldeArt::Claude, None);
        } else {
            let warum = letzte_worte(&ausgabe.lock().unwrap())
                .unwrap_or_else(|| "Die Anmeldung endete, bevor sie fertig war".into());
            a3.setzen(&k3, AnmeldeArt::Claude, keine(Some(warum)));
        }
        let _ = a3.geaendert.send(format!("nachsehen:{k3}"));
    });
    Ok(())
}

/// Der Code von Claudes Anmeldeseite.
pub fn claude_code(a: &AnmeldungenRef, konto: &str, code: &str) -> Result<(), String> {
    let code = code.trim();
    if code.len() < 8
        || code.len() > 2048
        || !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "#_.~-".contains(c))
    {
        return Err("Das sieht nicht aus wie der Code von Claudes Anmeldeseite".into());
    }
    let tx = a
        .live
        .lock()
        .unwrap()
        .get(konto)
        .and_then(|l| l.claude_eingabe.clone())
        .ok_or("Starte erst die Anmeldung bei Claude")?;
    tx.send(code.to_string())
        .map_err(|_| "Die Anmeldung läuft nicht mehr".to_string())?;
    let url = a
        .live
        .lock()
        .unwrap()
        .get(konto)
        .and_then(|l| l.claude.as_ref()?.offen.as_ref()?.url.clone());
    a.setzen(
        konto,
        AnmeldeArt::Claude,
        laeuft(
            AnmeldungOffen {
                url,
                code: None,
                geschickt: true,
            },
            None,
        ),
    );
    Ok(())
}
