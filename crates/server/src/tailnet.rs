//! Im Tailscale-Netz (deploy/provision.sh --tailscale) bekommt jeder Server eines Arbeiters seinen
//! eigenen Port auf https://<office>.ts.net:<port>, den Tailscale Serve auf den Office-Port zeigt;
//! das Office prüft die Anmeldung und reicht weiter (siehe `http::relay`). Tailscale umstellen braucht
//! root, darum geht das über einen Helfer (Env `OFFICE_SERVE_HELFER`, Aufruf `sync PORT…`).

use std::path::PathBuf;
use std::time::Duration;

use crate::office::OfficeRef;

fn helfer() -> PathBuf {
    std::env::var_os("OFFICE_SERVE_HELFER").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/local/bin/open-agentic-office-serve"))
}

async fn sync(ports: &[u16]) -> Result<(), String> {
    let aus = tokio::process::Command::new(helfer()).arg("sync").args(ports.iter().map(u16::to_string)).kill_on_drop(true).output().await.map_err(|e| e.to_string())?;
    if aus.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&aus.stderr).trim().to_string())
    }
}

/// Hält die Tailscale-Ports mit den Diensten der Arbeiter gleich (alle 15 s nachsehen).
pub fn starten(o: &OfficeRef) {
    if o.konfig.tailscale_host.is_none() || !helfer().is_file() {
        return;
    }
    let o = o.clone();
    tokio::spawn(async move {
        // Erst die Dienste finden lassen, damit ein Neustart sie nicht kurz abschaltet.
        tokio::time::sleep(Duration::from_secs(10)).await;
        let mut gesetzt: Option<Vec<u16>> = None;
        let mut letzter_fehler = String::new();
        loop {
            let mut ports = o.dienst_ports();
            ports.sort_unstable();
            ports.dedup();
            if gesetzt.as_ref() != Some(&ports) {
                match sync(&ports).await {
                    Ok(()) => {
                        gesetzt = Some(ports);
                        letzter_fehler.clear();
                    }
                    Err(f) => {
                        if f != letzter_fehler {
                            tracing::warn!("Die Dienste der Arbeiter ließen sich nicht im Tailnet freigeben: {f}");
                        }
                        letzter_fehler = f;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}
