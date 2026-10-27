//! Chromium starten (kopflos, mit Software-WebGL/WebGPU und Fake-Kamera/-Mikro) und Seiten öffnen.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::cdp::Cdp;
use crate::seite::Seite;

/// Wo Chromium liegt: `OFFICE_CHROMIUM` oder das von Playwright installierte.
pub fn chromium_finden() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OFFICE_CHROMIUM").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let home = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let basis = home.join("ms-playwright");
    let mut kandidaten: Vec<PathBuf> = std::fs::read_dir(&basis).ok()?.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("chromium-"))).collect();
    kandidaten.sort();
    kandidaten.into_iter().rev().map(|p| p.join("chrome-win64").join("chrome.exe")).find(|p| p.is_file())
}

pub struct Browser {
    kind: Child,
    pub cdp: Cdp,
    _profil: tempfile::TempDir,
}

impl Browser {
    pub async fn starten() -> Result<Self> {
        let exe = chromium_finden().context("Kein Chromium gefunden (OFFICE_CHROMIUM setzen oder Playwright-Chromium installieren)")?;
        let profil = tempfile::Builder::new().prefix("atlas-e2e-chrome-").tempdir()?;
        let kind = Command::new(&exe)
            .args([
                "--headless=new",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--enable-unsafe-webgpu",
                "--use-webgpu-adapter=swiftshader",
                "--enable-unsafe-swiftshader",
                "--use-fake-ui-for-media-stream",
                "--use-fake-device-for-media-stream",
                "--auto-select-desktop-capture-source=Entire screen",
                "--autoplay-policy=no-user-gesture-required",
                "--window-size=960,600",
                "--lang=de",
            ])
            .args(std::env::var("OFFICE_CHROMIUM_FLAGS").unwrap_or_default().split_whitespace())
            .arg(format!("--user-data-dir={}", profil.path().display()))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("{} ließ sich nicht starten", exe.display()))?;
        // Chromium schreibt Port und Pfad in DevToolsActivePort.
        let datei = profil.path().join("DevToolsActivePort");
        let mut url = None;
        for _ in 0..100 {
            if let Ok(s) = std::fs::read_to_string(&datei) {
                let mut z = s.lines();
                if let (Some(port), Some(pfad)) = (z.next(), z.next()) {
                    url = Some(format!("ws://127.0.0.1:{port}{pfad}"));
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let Some(url) = url else { bail!("Chromium hat keinen DevTools-Port gemeldet") };
        let cdp = Cdp::verbinden(&url).await?;
        Ok(Self { kind, cdp, _profil: profil })
    }

    /// Eine neue Seite (Tab) mit `url`.
    pub async fn seite(&self, url: &str) -> Result<Seite> {
        // Erst den Tab nehmen, mit dem Chromium startete; weitere Seiten als neue Tabs.
        let ziele = self.cdp.rufen(None, "Target.getTargets", json!({})).await?;
        let frei = ziele["targetInfos"].as_array().into_iter().flatten().find(|t| t["type"] == "page" && t["attached"] == false && t["url"] == "about:blank").and_then(|t| t["targetId"].as_str()).map(String::from);
        let id = match frei {
            Some(id) => id,
            None => self.cdp.rufen(None, "Target.createTarget", json!({ "url": "about:blank" })).await?["targetId"].as_str().context("keine targetId")?.to_string(),
        };
        let s = self.cdp.rufen(None, "Target.attachToTarget", json!({ "targetId": id, "flatten": true })).await?;
        let sitzung = s["sessionId"].as_str().context("keine sessionId")?.to_string();
        let seite = Seite::neu(self.cdp.clone(), sitzung).await?;
        seite.gehe(url).await?;
        Ok(seite)
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}
