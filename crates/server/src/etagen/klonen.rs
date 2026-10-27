//! Ein `gh repo clone` mit Fortschritt: liest gits Ausgabe (Zeilen enden auf \r oder \n), übersetzt
//! sie in Worte für den Aufzug, und gibt auf, wenn git zu lange nichts sagt.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use protokoll::etagen::KlonFortschritt;
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;

/// So lange darf git schweigen, bevor der Klon als hängend aufgegeben wird.
pub const STILL_MAX: Duration = Duration::from_secs(180);

fn schritt(git: &str) -> Option<&'static str> {
    Some(match git {
        "Enumerating objects" | "Counting objects" | "Compressing objects" => "GitHub packt zusammen",
        "Receiving objects" => "Herunterladen",
        "Resolving deltas" => "Entpacken",
        "Checking connectivity" => "Prüfen",
        "Updating files" => "Dateien auschecken",
        "Filtering content" => "LFS-Dateien laden",
        _ => return None,
    })
}

/// Fortschritt aus einer Zeile wie "Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s".
pub fn zeile_lesen(zeile: &str) -> Option<KlonFortschritt> {
    let z = zeile.trim();
    if z.starts_with("Cloning into ") {
        return Some(KlonFortschritt { schritt: "Verbinde mit GitHub".into(), prozent: None, detail: None });
    }
    let z = z.strip_prefix("remote: ").unwrap_or(z);
    let (was, rest) = z.split_once(':')?;
    let schritt = schritt(was)?;
    let rest = rest.trim();
    let prozent = rest.split_once('%').and_then(|(p, _)| p.trim().parse::<u8>().ok()).map(|p| p.min(100));
    let detail = rest.split_once(", ").map(|(_, d)| d.replace(" | ", " · ").trim_end_matches(", done.").trim_end_matches('.').to_string()).filter(|d| !d.is_empty());
    Some(KlonFortschritt { schritt: schritt.into(), prozent, detail })
}

/// Warum ein Klon scheiterte, als Hinweis, was auf dem Rechner des Office zu tun ist.
pub fn warum_gescheitert(ausgabe: &str) -> String {
    let gesagt: Vec<&str> = ausgabe.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty() && zeile_lesen(l).is_none()).collect();
    let alles = gesagt.join("\n");
    let hat = |s: &str| alles.to_lowercase().contains(&s.to_lowercase());
    if hat("Host key verification failed") {
        return "ssh auf dem Rechner des Office kennt den Host-Schlüssel von github.com noch nicht. Einmal `ssh -T git@github.com` ausführen oder mit `gh config set git_protocol https` über https klonen".into();
    }
    if hat("passphrase") {
        return "Der ssh-Schlüssel des Office braucht seine Passphrase. Mit `ssh-add` in den Agenten legen oder über https klonen (`gh config set git_protocol https`)".into();
    }
    if hat("Permission denied (publickey") {
        return "GitHub nahm den ssh-Schlüssel des Office nicht an. Mit `gh ssh-key add` hinzufügen oder über https klonen".into();
    }
    if hat("terminal prompts disabled") || hat("could not read Username") || hat("Authentication failed") {
        return "git wollte ein GitHub-Passwort. `gh auth setup-git` auf dem Rechner des Office ausführen".into();
    }
    if hat("Could not resolve to a Repository") || hat("not found") {
        return "Dieses Repository gibt es nicht, oder diese Anmeldung darf es nicht sehen".into();
    }
    let ende = gesagt[gesagt.len().saturating_sub(2)..].join(" ");
    if ende.is_empty() { "gh ist fehlgeschlagen".into() } else { ende }
}

/// Wie ein Klon endete.
#[derive(Debug)]
pub enum Ende {
    Fertig,
    Abgebrochen,
    Fehler(String),
}

/// Klont `repo` nach `ziel` und meldet Fortschritt (höchstens einmal pro Sekunde) an `fortschritt`.
pub async fn klonen(repo: &str, ziel: &Path, umgebung: &[(String, String)], mut fortschritt: impl FnMut(KlonFortschritt), abbruch: oneshot::Receiver<()>) -> Ende {
    if let Some(dir) = ziel.parent()
        && let Err(e) = tokio::fs::create_dir_all(dir).await
    {
        return Ende::Fehler(format!("Konnte {} nicht anlegen: {e}", dir.display()));
    }
    let mut cmd = tokio::process::Command::new("gh");
    cmd.arg("repo").arg("clone").arg(repo).arg(ziel).args(["--", "--progress"]);
    crate::anmeldungen::anwenden(&mut cmd, umgebung);
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("GH_PROMPT_DISABLED", "1");
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut kind = match cmd.spawn() {
        Ok(k) => k,
        Err(e) => return Ende::Fehler(format!("gh ließ sich nicht starten ({e}). Ist die GitHub-CLI installiert?")),
    };
    let mut stderr = kind.stderr.take().expect("stderr ist gepipt");
    let mut ausgabe = String::new();
    let mut puffer = [0u8; 4096];
    let mut zuletzt_gemeldet = tokio::time::Instant::now() - Duration::from_secs(2);
    let mut abbruch = abbruch;
    let ende = loop {
        tokio::select! {
            n = tokio::time::timeout(STILL_MAX, stderr.read(&mut puffer)) => match n {
                Err(_) => break Ende::Fehler("git hat drei Minuten nichts gesagt – der Klon hängt".into()),
                Ok(Ok(0)) | Ok(Err(_)) => break match kind.wait().await {
                    Ok(s) if s.success() => Ende::Fertig,
                    _ => Ende::Fehler(warum_gescheitert(&ausgabe)),
                },
                Ok(Ok(n)) => {
                    ausgabe.push_str(&String::from_utf8_lossy(&puffer[..n]));
                    if ausgabe.len() > 32 * 1024 {
                        ausgabe.drain(..ausgabe.len() - 16 * 1024);
                    }
                    if zuletzt_gemeldet.elapsed() >= Duration::from_secs(1)
                        && let Some(f) = ausgabe.rsplit(['\r', '\n']).find_map(zeile_lesen)
                    {
                        zuletzt_gemeldet = tokio::time::Instant::now();
                        fortschritt(f);
                    }
                }
            },
            _ = &mut abbruch => break Ende::Abgebrochen,
        }
    };
    if !matches!(ende, Ende::Fertig) {
        let _ = kind.kill().await;
        let _ = tokio::fs::remove_dir_all(ziel).await;
    }
    ende
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fortschritt_lesen() {
        let f = zeile_lesen("Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s").unwrap();
        assert_eq!(f.schritt, "Herunterladen");
        assert_eq!(f.prozent, Some(45));
        assert_eq!(f.detail.as_deref(), Some("1.20 MiB · 2.00 MiB/s"));
        assert_eq!(zeile_lesen("remote: Counting objects: 100% (5/5), done.").unwrap().prozent, Some(100));
        assert!(zeile_lesen("irgendwas").is_none());
    }

    #[test]
    fn fehler_erklaeren() {
        assert!(warum_gescheitert("Cloning into 'x'...\nHost key verification failed.\n").contains("ssh"));
        assert_eq!(warum_gescheitert(""), "gh ist fehlgeschlagen");
    }
}
