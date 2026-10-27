//! Die Abläufe hinter den Anmeldungen: `claude auth login` (im PTY, der Link kommt in den Browser,
//! der Code von der Anmeldeseite zurück), `gh auth login --web` (Einmal-Code), Token einfügen,
//! Abmelden und Nachsehen, wer angemeldet ist.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use protokoll::konten::{AnmeldeArt, AnmeldeStatus, AnmeldeWeg, Anmeldung, AnmeldungOffen};

use super::{AnmeldungenRef, Wahl, anwenden, ist_api_schluessel, ist_claude_token};

/// So lange darf eine Anmeldung offen bleiben.
const ABLAUF_MAX: Duration = Duration::from_secs(15 * 60);

/// Findet ein Programm im PATH (unter Windows auch .exe/.cmd).
pub fn finden(name: &str) -> Option<PathBuf> {
    let endungen: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat", ""] } else { &[""] };
    std::env::split_paths(&std::env::var_os("PATH")?).flat_map(|d| endungen.iter().map(move |e| d.join(format!("{name}{e}")))).find(|p| p.is_file())
}

/// Entfernt Terminal-Steuerfolgen.
pub fn schlicht(s: &str) -> String {
    let mut aus = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\x1b' {
            aus.push(c);
            continue;
        }
        match it.next() {
            Some('[') => {
                for c in it.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = it.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && it.peek() == Some(&'\\') {
                        it.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    aus
}

/// Die letzte sinnvolle Zeile (für Fehlermeldungen).
fn letzte_worte(s: &str) -> Option<String> {
    schlicht(s).lines().map(str::trim).rfind(|l| !l.is_empty() && !l.contains("http") && !l.to_lowercase().contains("paste code")).map(|l| l.chars().take(200).collect())
}

fn laeuft(offen: AnmeldungOffen, fehler: Option<String>) -> Option<Anmeldung> {
    Some(Anmeldung { status: AnmeldeStatus::Laeuft, weg: AnmeldeWeg::Login, wer: None, offen: Some(offen), fehler })
}

fn keine(fehler: Option<String>) -> Option<Anmeldung> {
    Some(Anmeldung { status: AnmeldeStatus::Keine, weg: AnmeldeWeg::Login, wer: None, offen: None, fehler })
}

async fn ausfuehren(prog: &str, args: &[&str], env: &[(String, String)], eingabe: Option<String>) -> Option<(bool, String, String)> {
    let pfad = finden(prog)?;
    let mut cmd = tokio::process::Command::new(pfad);
    cmd.args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    anwenden(&mut cmd, env);
    let mut kind = cmd.spawn().ok()?;
    if let (Some(e), Some(mut stdin)) = (eingabe, kind.stdin.take()) {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(e.as_bytes()).await;
    }
    let aus = tokio::time::timeout(Duration::from_secs(30), kind.wait_with_output()).await.ok()?.ok()?;
    Some((aus.status.success(), String::from_utf8_lossy(&aus.stdout).into_owned(), String::from_utf8_lossy(&aus.stderr).into_owned()))
}

/// Sieht nach, wer angemeldet ist, und merkt es sich.
pub async fn nachsehen(a: &AnmeldungenRef, konto: &str, env: Vec<(String, String)>) {
    let laufend = a.live.lock().unwrap().get(konto).map(|l| (l.claude_eingabe.is_some(), l.abbruch_github.is_some())).unwrap_or_default();
    if !laufend.0 {
        let g = a.laden(konto);
        let token = matches!(g.claude, Some(Wahl::Token { .. }));
        let wer = match ausfuehren("claude", &["auth", "status", "--json"], &env, None).await {
            Some((_, out, _)) => {
                let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
                let plan = v.get("subscriptionType").and_then(|p| p.as_str()).map(|p| format!(" · {p}")).unwrap_or_default();
                let drin = v.get("loggedIn").and_then(|b| b.as_bool()).unwrap_or(false);
                (drin || token).then(|| format!("{}{plan}", v.get("email").and_then(|e| e.as_str()).unwrap_or("ein Token")))
            }
            None if token => Some("ein eingefügter Token".into()),
            None => None,
        };
        a.gesehen(konto, AnmeldeArt::Claude, wer.clone());
        a.setzen(konto, AnmeldeArt::Claude, wer.is_none().then(|| keine(None)).flatten());
    }
    if !laufend.1 {
        let wer = match ausfuehren("gh", &["api", "user", "--jq", "{login, id, name}"], &env, None).await {
            Some((true, out, _)) => {
                let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
                v.get("login").and_then(|l| l.as_str()).map(|login| {
                    let name = v.get("name").and_then(|n| n.as_str()).filter(|n| !n.trim().is_empty()).unwrap_or(login);
                    let id = v.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
                    let gitconfig = format!("[user]\n\tname = {name}\n\temail = {id}+{login}@users.noreply.github.com\n[credential \"https://github.com\"]\n\thelper = !gh auth git-credential\n");
                    let _ = std::fs::write(a.home(konto).join("gitconfig"), gitconfig);
                    format!("@{login}")
                })
            }
            _ => None,
        };
        a.gesehen(konto, AnmeldeArt::Github, wer.clone());
        a.setzen(konto, AnmeldeArt::Github, wer.is_none().then(|| keine(None)).flatten());
    }
}

/// Startet `claude auth login` in einem PTY. Fehlertext, wenn es nicht geht.
pub fn claude_starten(a: &AnmeldungenRef, konto: &str, env: Vec<(String, String)>) -> Result<(), String> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    let pfad = finden("claude").ok_or("Claude Code ist auf dem Rechner des Office nicht installiert – füge stattdessen einen Token aus `claude setup-token` ein")?;
    abbrechen(a, konto, AnmeldeArt::Claude);
    let pty = native_pty_system().openpty(PtySize { rows: 40, cols: 4000, pixel_width: 0, pixel_height: 0 }).map_err(|e| format!("Kein PTY: {e}"))?;
    let mut cmd = CommandBuilder::new(pfad);
    cmd.args(["auth", "login", "--claudeai"]);
    cmd.cwd(a.home(konto));
    for (k, v) in &env {
        if v.is_empty() { cmd.env_remove(k) } else { cmd.env(k, v) }
    }
    cmd.env("BROWSER", "true");
    cmd.env("TERM", "xterm-256color");
    let mut kind = pty.slave.spawn_command(cmd).map_err(|e| format!("Claudes Anmeldung ließ sich nicht starten: {e}"))?;
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
    a.setzen(konto, AnmeldeArt::Claude, laeuft(AnmeldungOffen::default(), None));

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
                let bis = aus.char_indices().find(|(i, _)| *i >= weg).map(|(i, _)| i).unwrap_or(0);
                aus.drain(..bis);
            }
            if url.is_none() {
                let text = schlicht(&aus);
                if let Some(i) = text.find("https://") {
                    let kandidat: String = text[i..].chars().take_while(|c| !c.is_whitespace()).collect();
                    if kandidat.contains("oauth/authorize") {
                        url = Some(kandidat.clone());
                        a2.setzen(&k2, AnmeldeArt::Claude, laeuft(AnmeldungOffen { url: Some(kandidat), code: None, geschickt: false }, None));
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
            let warum = letzte_worte(&ausgabe.lock().unwrap()).unwrap_or_else(|| "Die Anmeldung endete, bevor sie fertig war".into());
            a3.setzen(&k3, AnmeldeArt::Claude, keine(Some(warum)));
        }
        let _ = a3.geaendert.send(format!("nachsehen:{k3}"));
    });
    Ok(())
}

/// Der Code von Claudes Anmeldeseite.
pub fn claude_code(a: &AnmeldungenRef, konto: &str, code: &str) -> Result<(), String> {
    let code = code.trim();
    if code.len() < 8 || code.len() > 2048 || !code.chars().all(|c| c.is_ascii_alphanumeric() || "#_.~-".contains(c)) {
        return Err("Das sieht nicht aus wie der Code von Claudes Anmeldeseite".into());
    }
    let tx = a.live.lock().unwrap().get(konto).and_then(|l| l.claude_eingabe.clone()).ok_or("Starte erst die Anmeldung bei Claude")?;
    tx.send(code.to_string()).map_err(|_| "Die Anmeldung läuft nicht mehr".to_string())?;
    let url = a.live.lock().unwrap().get(konto).and_then(|l| l.claude.as_ref()?.offen.as_ref()?.url.clone());
    a.setzen(konto, AnmeldeArt::Claude, laeuft(AnmeldungOffen { url, code: None, geschickt: true }, None));
    Ok(())
}

/// Startet `gh auth login --web`: der Einmal-Code und die Seite gehen an den Browser.
pub fn github_starten(a: &AnmeldungenRef, konto: &str, env: Vec<(String, String)>) -> Result<(), String> {
    let pfad = finden("gh").ok_or("Die GitHub-CLI (gh) ist auf dem Rechner des Office nicht installiert")?;
    abbrechen(a, konto, AnmeldeArt::Github);
    let mut cmd = tokio::process::Command::new(pfad);
    cmd.args(["auth", "login", "--hostname", "github.com", "--git-protocol", "https", "--web", "--insecure-storage", "--skip-ssh-key"]);
    cmd.current_dir(a.home(konto)).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    anwenden(&mut cmd, &env);
    cmd.env("BROWSER", "true").env("GH_BROWSER", "true");
    let mut kind = cmd.spawn().map_err(|e| format!("gh ließ sich nicht starten: {e}"))?;
    let (stopp_tx, stopp_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut live = a.live.lock().unwrap();
        let l = live.entry(konto.to_string()).or_default();
        l.abbruch_github = Some(Box::new(move || {
            let _ = stopp_tx.send(());
        }));
    }
    a.setzen(konto, AnmeldeArt::Github, laeuft(AnmeldungOffen::default(), None));
    let (a, konto) = (a.clone(), konto.to_string());
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let (mut o, mut e) = (kind.stdout.take().expect("gepipt"), kind.stderr.take().expect("gepipt"));
        let mut aus = String::new();
        let (mut po, mut pe) = ([0u8; 2048], [0u8; 2048]);
        let mut code_da = false;
        let mut stopp = stopp_rx;
        let frist = tokio::time::sleep(ABLAUF_MAX);
        tokio::pin!(frist);
        let ok = loop {
            tokio::select! {
                n = o.read(&mut po) => if let Ok(n @ 1..) = n { aus.push_str(&String::from_utf8_lossy(&po[..n])) },
                n = e.read(&mut pe) => if let Ok(n @ 1..) = n { aus.push_str(&String::from_utf8_lossy(&pe[..n])) },
                s = kind.wait() => break s.map(|s| s.success()).unwrap_or(false),
                _ = &mut stopp => { let _ = kind.kill().await; break false },
                _ = &mut frist => { let _ = kind.kill().await; break false },
            }
            if !code_da && let Some(i) = aus.find("one-time code: ") {
                let code: String = aus[i + 15..].chars().take(9).collect();
                if code.len() == 9 {
                    code_da = true;
                    a.setzen(&konto, AnmeldeArt::Github, laeuft(AnmeldungOffen { url: Some("https://github.com/login/device".into()), code: Some(code), geschickt: false }, None));
                }
            }
        };
        if let Some(l) = a.live.lock().unwrap().get_mut(&konto) {
            l.abbruch_github = None;
        }
        if ok {
            let mut g = a.laden(&konto);
            g.github = None;
            a.sichern(&konto, &g);
            a.setzen(&konto, AnmeldeArt::Github, None);
        } else {
            a.setzen(&konto, AnmeldeArt::Github, keine(Some(letzte_worte(&aus).unwrap_or_else(|| "Die Anmeldung endete, bevor sie fertig war".into()))));
        }
        let _ = a.geaendert.send(format!("nachsehen:{konto}"));
    });
    Ok(())
}

pub fn abbrechen(a: &AnmeldungenRef, konto: &str, art: AnmeldeArt) {
    let f = a.live.lock().unwrap().get_mut(konto).and_then(|l| match art {
        AnmeldeArt::Claude => l.abbruch_claude.take(),
        AnmeldeArt::Github => l.abbruch_github.take(),
    });
    if let Some(f) = f {
        f();
    }
}

/// Ein eingefügter Token. Claude: aus `claude setup-token` oder ein API-Schlüssel; GitHub: ein Token für gh.
pub async fn token(a: &AnmeldungenRef, konto: &str, art: AnmeldeArt, token: &str, env: Vec<(String, String)>) -> Result<(), String> {
    let token = token.trim();
    match art {
        AnmeldeArt::Claude => {
            if !ist_claude_token(token) {
                return Err("Das ist kein Token aus `claude setup-token` (sk-ant-oat01-…) und kein Anthropic-API-Schlüssel (sk-ant-api03-…)".into());
            }
            let mut g = a.laden(konto);
            g.claude = Some(Wahl::Token { token: token.into() });
            a.sichern(konto, &g);
            let wer = if ist_api_schluessel(token) { "ein API-Schlüssel" } else { "ein Token aus claude setup-token" };
            a.gesehen(konto, art, Some(wer.into()));
            a.setzen(konto, art, None);
            Ok(())
        }
        AnmeldeArt::Github => {
            if token.len() < 20 || token.len() > 400 || !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err("Das sieht nicht aus wie ein GitHub-Token".into());
            }
            match ausfuehren("gh", &["auth", "login", "--hostname", "github.com", "--with-token", "--insecure-storage"], &env, Some(format!("{token}\n"))).await {
                Some((true, ..)) => {
                    let mut g = a.laden(konto);
                    g.github = None;
                    a.sichern(konto, &g);
                    Ok(())
                }
                Some((false, _, err)) => Err(letzte_worte(&err).unwrap_or_else(|| "GitHub hat den Token nicht angenommen".into())),
                None => Err("Die GitHub-CLI (gh) ist auf dem Rechner des Office nicht installiert".into()),
            }
        }
    }
}

/// Die Anmeldung des Office-Rechners verwenden.
pub fn office(a: &AnmeldungenRef, konto: &str, art: AnmeldeArt) {
    let mut g = a.laden(konto);
    match art {
        AnmeldeArt::Claude => g.claude = Some(Wahl::Office),
        AnmeldeArt::Github => g.github = Some(Wahl::Office),
    }
    a.sichern(konto, &g);
    a.melden(konto);
}

/// Abmelden: den eigenen Login löschen und die Wahl vergessen.
pub async fn abmelden(a: &AnmeldungenRef, konto: &str, art: AnmeldeArt, env: Vec<(String, String)>) {
    abbrechen(a, konto, art);
    let mut g = a.laden(konto);
    match art {
        AnmeldeArt::Claude => {
            if !matches!(g.claude, Some(Wahl::Office)) {
                let _ = ausfuehren("claude", &["auth", "logout"], &env, None).await;
            }
            g.claude = None;
        }
        AnmeldeArt::Github => {
            if !matches!(g.github, Some(Wahl::Office)) {
                let _ = ausfuehren("gh", &["auth", "logout", "--hostname", "github.com"], &env, None).await;
                let _ = std::fs::remove_file(a.home(konto).join("gh").join("hosts.yml"));
            }
            g.github = None;
        }
    }
    a.sichern(konto, &g);
    a.gesehen(konto, art, None);
    a.setzen(konto, art, keine(None));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steuerfolgen_weg() {
        assert_eq!(schlicht("\x1b[1mfett\x1b[0m \x1b]8;;http://x\x07link\x1b]8;;\x07"), "fett link");
        assert_eq!(letzte_worte("a\nFehler: kaputt\n\n").as_deref(), Some("Fehler: kaputt"));
    }
}
