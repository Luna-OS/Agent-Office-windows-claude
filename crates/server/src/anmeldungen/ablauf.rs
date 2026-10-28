//! Die Abläufe hinter den Anmeldungen (Claudes Login steht in `claude.rs`): `gh auth login --web` (Einmal-Code), Token einfügen,
//! Abmelden und Nachsehen, wer angemeldet ist.

use std::path::PathBuf;
use std::time::Duration;

use protokoll::konten::{AnmeldeArt, AnmeldeStatus, AnmeldeWeg, Anmeldung, AnmeldungOffen};

use super::{AnmeldungenRef, Wahl, anwenden, ist_api_schluessel, ist_claude_token};

/// So lange darf eine Anmeldung offen bleiben.
pub(super) const ABLAUF_MAX: Duration = Duration::from_secs(15 * 60);

/// Findet ein Programm im PATH (unter Windows auch .exe/.cmd).
pub fn finden(name: &str) -> Option<PathBuf> {
    let endungen: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|d| endungen.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|p| p.is_file())
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
pub(super) fn letzte_worte(s: &str) -> Option<String> {
    schlicht(s)
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.contains("http") && !l.to_lowercase().contains("paste code"))
        .map(|l| l.chars().take(200).collect())
}

pub(super) fn laeuft(offen: AnmeldungOffen, fehler: Option<String>) -> Option<Anmeldung> {
    Some(Anmeldung {
        status: AnmeldeStatus::Laeuft,
        weg: AnmeldeWeg::Login,
        wer: None,
        offen: Some(offen),
        fehler,
    })
}

pub(super) fn keine(fehler: Option<String>) -> Option<Anmeldung> {
    Some(Anmeldung {
        status: AnmeldeStatus::Keine,
        weg: AnmeldeWeg::Login,
        wer: None,
        offen: None,
        fehler,
    })
}

async fn ausfuehren(
    prog: &str,
    args: &[&str],
    env: &[(String, String)],
    eingabe: Option<String>,
) -> Option<(bool, String, String)> {
    let pfad = finden(prog)?;
    let mut cmd = tokio::process::Command::new(pfad);
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    anwenden(&mut cmd, env);
    let mut kind = cmd.spawn().ok()?;
    if let (Some(e), Some(mut stdin)) = (eingabe, kind.stdin.take()) {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(e.as_bytes()).await;
    }
    let aus = tokio::time::timeout(Duration::from_secs(30), kind.wait_with_output())
        .await
        .ok()?
        .ok()?;
    Some((
        aus.status.success(),
        String::from_utf8_lossy(&aus.stdout).into_owned(),
        String::from_utf8_lossy(&aus.stderr).into_owned(),
    ))
}

/// Sieht nach, wer angemeldet ist, und merkt es sich.
pub async fn nachsehen(a: &AnmeldungenRef, konto: &str, env: Vec<(String, String)>) {
    let laufend = a
        .live
        .lock()
        .unwrap()
        .get(konto)
        .map(|l| (l.claude_eingabe.is_some(), l.abbruch_github.is_some()))
        .unwrap_or_default();
    if !laufend.0 {
        let g = a.laden(konto);
        let token = matches!(g.claude, Some(Wahl::Token { .. }));
        let wer = match ausfuehren("claude", &["auth", "status", "--json"], &env, None).await {
            Some((_, out, _)) => {
                let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
                let plan = v
                    .get("subscriptionType")
                    .and_then(|p| p.as_str())
                    .map(|p| format!(" · {p}"))
                    .unwrap_or_default();
                let drin = v.get("loggedIn").and_then(|b| b.as_bool()).unwrap_or(false);
                (drin || token).then(|| {
                    format!(
                        "{}{plan}",
                        v.get("email")
                            .and_then(|e| e.as_str())
                            .unwrap_or("ein Token")
                    )
                })
            }
            None if token => Some("ein eingefügter Token".into()),
            None => None,
        };
        a.gesehen(konto, AnmeldeArt::Claude, wer.clone());
        a.setzen(
            konto,
            AnmeldeArt::Claude,
            wer.is_none().then(|| keine(None)).flatten(),
        );
    }
    if !laufend.1 {
        let wer = match ausfuehren(
            "gh",
            &["api", "user", "--jq", "{login, id, name}"],
            &env,
            None,
        )
        .await
        {
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
        a.setzen(
            konto,
            AnmeldeArt::Github,
            wer.is_none().then(|| keine(None)).flatten(),
        );
    }
}

/// Startet `gh auth login --web`: der Einmal-Code und die Seite gehen an den Browser.
pub fn github_starten(
    a: &AnmeldungenRef,
    konto: &str,
    env: Vec<(String, String)>,
) -> Result<(), String> {
    let pfad = finden("gh")
        .ok_or("Die GitHub-CLI (gh) ist auf dem Rechner des Office nicht installiert")?;
    abbrechen(a, konto, AnmeldeArt::Github);
    let mut cmd = tokio::process::Command::new(pfad);
    cmd.args([
        "auth",
        "login",
        "--hostname",
        "github.com",
        "--git-protocol",
        "https",
        "--web",
        "--insecure-storage",
        "--skip-ssh-key",
    ]);
    cmd.current_dir(a.home(konto))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    anwenden(&mut cmd, &env);
    cmd.env("BROWSER", "true").env("GH_BROWSER", "true");
    let mut kind = cmd
        .spawn()
        .map_err(|e| format!("gh ließ sich nicht starten: {e}"))?;
    let (stopp_tx, stopp_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut live = a.live.lock().unwrap();
        let l = live.entry(konto.to_string()).or_default();
        l.abbruch_github = Some(Box::new(move || {
            let _ = stopp_tx.send(());
        }));
    }
    a.setzen(
        konto,
        AnmeldeArt::Github,
        laeuft(AnmeldungOffen::default(), None),
    );
    let (a, konto) = (a.clone(), konto.to_string());
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let (mut o, mut e) = (
            kind.stdout.take().expect("gepipt"),
            kind.stderr.take().expect("gepipt"),
        );
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
                    a.setzen(
                        &konto,
                        AnmeldeArt::Github,
                        laeuft(
                            AnmeldungOffen {
                                url: Some("https://github.com/login/device".into()),
                                code: Some(code),
                                geschickt: false,
                            },
                            None,
                        ),
                    );
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
            a.setzen(
                &konto,
                AnmeldeArt::Github,
                keine(Some(letzte_worte(&aus).unwrap_or_else(|| {
                    "Die Anmeldung endete, bevor sie fertig war".into()
                }))),
            );
        }
        let _ = a.geaendert.send(format!("nachsehen:{konto}"));
    });
    Ok(())
}

pub fn abbrechen(a: &AnmeldungenRef, konto: &str, art: AnmeldeArt) {
    let f = a
        .live
        .lock()
        .unwrap()
        .get_mut(konto)
        .and_then(|l| match art {
            AnmeldeArt::Claude => l.abbruch_claude.take(),
            AnmeldeArt::Github => l.abbruch_github.take(),
        });
    if let Some(f) = f {
        f();
    }
}

/// Ein eingefügter Token. Claude: aus `claude setup-token` oder ein API-Schlüssel; GitHub: ein Token für gh.
pub async fn token(
    a: &AnmeldungenRef,
    konto: &str,
    art: AnmeldeArt,
    token: &str,
    env: Vec<(String, String)>,
) -> Result<(), String> {
    let token = token.trim();
    match art {
        AnmeldeArt::Claude => {
            if !ist_claude_token(token) {
                return Err("Das ist kein Token aus `claude setup-token` (sk-ant-oat01-…) und kein Anthropic-API-Schlüssel (sk-ant-api03-…)".into());
            }
            let mut g = a.laden(konto);
            g.claude = Some(Wahl::Token {
                token: token.into(),
            });
            a.sichern(konto, &g);
            let wer = if ist_api_schluessel(token) {
                "ein API-Schlüssel"
            } else {
                "ein Token aus claude setup-token"
            };
            a.gesehen(konto, art, Some(wer.into()));
            a.setzen(konto, art, None);
            Ok(())
        }
        AnmeldeArt::Github => {
            if token.len() < 20
                || token.len() > 400
                || !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err("Das sieht nicht aus wie ein GitHub-Token".into());
            }
            match ausfuehren(
                "gh",
                &[
                    "auth",
                    "login",
                    "--hostname",
                    "github.com",
                    "--with-token",
                    "--insecure-storage",
                ],
                &env,
                Some(format!("{token}\n")),
            )
            .await
            {
                Some((true, ..)) => {
                    let mut g = a.laden(konto);
                    g.github = None;
                    a.sichern(konto, &g);
                    Ok(())
                }
                Some((false, _, err)) => Err(letzte_worte(&err)
                    .unwrap_or_else(|| "GitHub hat den Token nicht angenommen".into())),
                None => Err(
                    "Die GitHub-CLI (gh) ist auf dem Rechner des Office nicht installiert".into(),
                ),
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
pub async fn abmelden(
    a: &AnmeldungenRef,
    konto: &str,
    art: AnmeldeArt,
    env: Vec<(String, String)>,
) {
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
                let _ = ausfuehren(
                    "gh",
                    &["auth", "logout", "--hostname", "github.com"],
                    &env,
                    None,
                )
                .await;
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
        assert_eq!(
            schlicht("\x1b[1mfett\x1b[0m \x1b]8;;http://x\x07link\x1b]8;;\x07"),
            "fett link"
        );
        assert_eq!(
            letzte_worte("a\nFehler: kaputt\n\n").as_deref(),
            Some("Fehler: kaputt")
        );
    }
}
