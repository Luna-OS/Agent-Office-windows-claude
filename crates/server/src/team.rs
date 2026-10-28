//! Das SSH-Team: wer per Tunnel (`ssh -L 4600:localhost:4600 office@rechner`) ins Office darf.
//! Nur auf Offices, die mit deploy/provision.sh aufgesetzt sind: dessen Helfer (Umgebung
//! `OFFICE_TEAM_HELFER`, Standard /usr/local/bin/open-agentic-office-team) ändert die
//! authorized_keys des Nutzers `office`; das Office übergibt nur geprüfte Namen und Schlüsseltext.
//!
//! Helfer-Vertrag: `fingerabdruck` → eine Zeile; `liste` → Zeilen "name anzahl"; `hinzufuegen NAME`
//! (Schlüssel auf stdin) → Anzahl, Exit 65 = kein brauchbarer Schlüssel; `entfernen NAME`, Exit 66 = nicht eingeladen.

use std::path::PathBuf;
use std::time::Duration;

use protokoll::konten::{TeamMitglied, TeamStand};

use crate::konfig::Konfig;

const NUTZER: &str = "office";

fn helfer() -> PathBuf {
    std::env::var_os("OFFICE_TEAM_HELFER")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin/open-agentic-office-team"))
}

fn ist_github_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 39
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn ist_mitglied(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 39
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

async fn rufen(args: &[&str], eingabe: Option<String>) -> (i32, String, String) {
    let mut cmd = tokio::process::Command::new(helfer());
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let Ok(mut kind) = cmd.spawn() else {
        return (
            1,
            String::new(),
            "Der Team-Helfer ließ sich nicht starten".into(),
        );
    };
    if let Some(mut stdin) = kind.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin
            .write_all(eingabe.unwrap_or_default().as_bytes())
            .await;
    }
    match tokio::time::timeout(Duration::from_secs(15), kind.wait_with_output()).await {
        Ok(Ok(a)) => (
            a.status.code().unwrap_or(1),
            String::from_utf8_lossy(&a.stdout).into_owned(),
            String::from_utf8_lossy(&a.stderr).trim().to_string(),
        ),
        _ => (1, String::new(), "Der Team-Helfer antwortete nicht".into()),
    }
}

pub fn verfuegbar(k: &Konfig) -> bool {
    k.oeffentlicher_host.is_some() && helfer().is_file()
}

/// Wohin getunnelt wird: office@host, oder ssh://office@host:port, wenn die Adresse einen eigenen Port hat.
pub fn ssh(k: &Konfig) -> Option<String> {
    let h = k
        .oeffentlicher_host
        .as_deref()?
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    match h.rsplit_once(':') {
        Some((host, port)) if port != "22" && port.bytes().all(|b| b.is_ascii_digit()) => {
            Some(format!("ssh://{NUTZER}@{host}:{port}"))
        }
        Some((host, _)) => Some(format!("{NUTZER}@{host}")),
        None => Some(format!("{NUTZER}@{h}")),
    }
}

pub async fn stand(k: &Konfig) -> TeamStand {
    let mut s = TeamStand {
        port: k.port,
        tailnet: k.tailscale_host.clone(),
        deploy: k.deploy_befehl.clone(),
        ..Default::default()
    };
    if !verfuegbar(k) {
        if s.tailnet.is_none() {
            s.nicht_verfuegbar = Some(
                "Einladungen gibt es auf Offices, die mit deploy/provision.sh aufgesetzt sind."
                    .into(),
            );
        }
        return s;
    }
    s.ssh = ssh(k);
    let (_, fp, _) = rufen(&["fingerabdruck"], None).await;
    s.fingerabdruck = Some(fp.trim().to_string()).filter(|f| !f.is_empty());
    let (code, aus, err) = rufen(&["liste"], None).await;
    if code != 0 {
        s.fehler = Some(format!("Das Team ließ sich nicht auflisten: {err}"));
        return s;
    }
    s.mitglieder = aus
        .lines()
        .filter_map(|l| {
            let mut t = l.split_whitespace();
            Some(TeamMitglied {
                name: t.next()?.to_string(),
                schluessel: t.next()?.parse().unwrap_or(0),
            })
        })
        .collect();
    s
}

/// Lädt die SSH-Schlüssel von github.com/<name>.keys ein. Gibt (Name, Anzahl) zurück.
pub async fn einladen(k: &Konfig, github: &str) -> Result<(String, u32), String> {
    if !verfuegbar(k) {
        return Err(
            "Einladungen gibt es auf Offices, die mit deploy/provision.sh aufgesetzt sind".into(),
        );
    }
    let name = github.trim().trim_start_matches('@').to_string();
    if !ist_github_name(&name) {
        return Err(format!("„{github}“ ist kein GitHub-Name"));
    }
    let url = format!("https://github.com/{name}.keys");
    let n = name.clone();
    let text = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut r = agent
            .get(&url)
            .call()
            .map_err(|e| format!("GitHub war nicht erreichbar: {e}"))?;
        match r.status().as_u16() {
            404 => Err(format!("Einen GitHub-Nutzer {n} gibt es nicht")),
            200 => r
                .body_mut()
                .with_config()
                .limit(64 * 1024)
                .read_to_string()
                .map_err(|e| e.to_string()),
            s => Err(format!(
                "GitHub antwortete {s} für die Schlüssel von {n} – bitte noch einmal"
            )),
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    if text.trim().is_empty() {
        return Err(format!(
            "{name} hat keine SSH-Schlüssel auf GitHub – unter github.com/settings/keys lässt sich einer hinzufügen"
        ));
    }
    match rufen(&["hinzufuegen", &name], Some(text)).await {
        (0, aus, _) => Ok((name, aus.trim().parse().unwrap_or(0))),
        (65, ..) => Err(format!(
            "Keiner der GitHub-Schlüssel von {name} passt hier zu SSH"
        )),
        (_, _, err) => Err(format!(
            "Die Schlüssel von {name} ließen sich nicht hinzufügen: {err}"
        )),
    }
}

pub async fn entfernen(k: &Konfig, name: &str) -> Result<(), String> {
    if !verfuegbar(k) {
        return Err(
            "Einladungen gibt es auf Offices, die mit deploy/provision.sh aufgesetzt sind".into(),
        );
    }
    if !ist_mitglied(name) {
        return Err(format!("„{name}“ ist kein Name aus dem Team"));
    }
    match rufen(&["entfernen", name], None).await {
        (0, ..) => Ok(()),
        (66, ..) => Err(format!("{name} ist nicht eingeladen")),
        (_, _, err) => Err(format!("{name} ließ sich nicht entfernen: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen_und_ssh() {
        assert!(ist_github_name("luna-os"));
        assert!(!ist_github_name("-x"));
        assert!(!ist_github_name("a/b"));
        let mut k = Konfig::fuer_test(PathBuf::from("x"));
        k.oeffentlicher_host = Some("203.0.113.7".into());
        assert_eq!(ssh(&k).as_deref(), Some("office@203.0.113.7"));
        k.oeffentlicher_host = Some("host.example:2222".into());
        assert_eq!(ssh(&k).as_deref(), Some("ssh://office@host.example:2222"));
    }
}
