//! Selbst-Update über das Deploy-Skript (`OFFICE_SELBSTUPDATE=1`, `OFFICE_DEPLOY_SKRIPT=<pfad>`):
//! `<skript> pruefen` antwortet mit JSON, `<skript> ausfuehren` baut, installiert und startet neu.

use protokoll::einstellungen::{Update, UpdatePhase, Version};
use serde::Deserialize;

#[derive(Deserialize)]
struct Pruefung {
    aktuell: Option<Version>,
    #[serde(default)]
    neueste: Option<Version>,
    #[serde(default)]
    aenderungen: Vec<Version>,
}

/// Der Ausgangsstand: verfügbar nur mit Skript und eingeschaltetem Selbst-Update.
pub fn anfang(selbstupdate: bool, skript: Option<&str>) -> Update {
    Update {
        verfuegbar: selbstupdate && skript.is_some(),
        ..Default::default()
    }
}

async fn ausfuehren(skript: &str, befehl: &str) -> Result<String, String> {
    let aus = tokio::process::Command::new(skript)
        .arg(befehl)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Das Update-Skript ließ sich nicht starten: {e}"))?;
    if !aus.status.success() {
        let err = String::from_utf8_lossy(&aus.stderr);
        return Err(err
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("Das Update-Skript ist fehlgeschlagen")
            .trim()
            .to_string());
    }
    Ok(String::from_utf8_lossy(&aus.stdout).into_owned())
}

/// Fragt das Skript, ob es Neues gibt.
pub async fn pruefen(skript: &str, mut u: Update) -> Update {
    u.prueft = false;
    match ausfuehren(skript, "pruefen").await.and_then(|s| {
        serde_json::from_str::<Pruefung>(&s)
            .map_err(|e| format!("Das Update-Skript antwortete unerwartet: {e}"))
    }) {
        Ok(p) => {
            u.neueste = p
                .neueste
                .filter(|n| p.aktuell.as_ref().is_none_or(|a| a.sha != n.sha));
            u.aktuell = p.aktuell;
            u.aenderungen = p.aenderungen.into_iter().take(15).collect();
            u.fehler = None;
        }
        Err(f) => u.fehler = Some(f),
    }
    u
}

/// Startet das Update; gibt es Fehler, steht die Phase danach auf `Fehlgeschlagen`.
pub async fn starten(skript: &str, mut u: Update) -> Update {
    match ausfuehren(skript, "ausfuehren").await {
        Ok(_) => u.phase = UpdatePhase::Startet,
        Err(f) => {
            u.phase = UpdatePhase::Fehlgeschlagen;
            u.fehler = Some(f);
        }
    }
    u
}
