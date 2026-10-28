//! Eine Browser-Seite: hingehen, JavaScript auswerten, warten, klicken, tippen, Tasten, Bildschirmfotos.
//! Klicks und Tasten gehen über `Input.*`, zählen also als echte Nutzergesten (Pointer-Lock!).

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde_json::{Value, json};

use crate::cdp::Cdp;

pub struct Seite {
    cdp: Cdp,
    sitzung: String,
    /// Was die Seite in die Konsole schrieb und welche Fehler sie warf (für Fehlermeldungen).
    pub konsole: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Seite {
    pub async fn neu(cdp: Cdp, sitzung: String) -> Result<Self> {
        let konsole = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut rx = cdp.zuhoeren();
        let (k, s) = (konsole.clone(), sitzung.clone());
        tokio::spawn(async move {
            while let Ok(e) = rx.recv().await {
                if e.sitzung.as_deref() != Some(&s) {
                    continue;
                }
                let zeile = match e.methode.as_str() {
                    "Runtime.consoleAPICalled" => {
                        let teile: Vec<String> = e.params["args"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|a| {
                                a.get("value").map(|v| v.to_string()).unwrap_or_else(|| {
                                    a["description"].as_str().unwrap_or("").to_string()
                                })
                            })
                            .collect();
                        format!(
                            "[{}] {}",
                            e.params["type"].as_str().unwrap_or(""),
                            teile.join(" ")
                        )
                    }
                    "Runtime.exceptionThrown" => format!(
                        "[Ausnahme] {}",
                        e.params["exceptionDetails"]["exception"]["description"]
                            .as_str()
                            .unwrap_or_else(|| e.params["exceptionDetails"]["text"]
                                .as_str()
                                .unwrap_or(""))
                    ),
                    _ => continue,
                };
                let mut k = k.lock().unwrap();
                if k.len() < 2000 {
                    k.push(zeile);
                }
            }
        });
        let seite = Self {
            cdp,
            sitzung,
            konsole,
        };
        seite.langsam("Page.enable", json!({})).await?;
        seite.langsam("Runtime.enable", json!({})).await?;
        // Kopflos hat die Seite sonst keinen Fokus, und Pointer-Lock schlägt fehl.
        seite
            .rufen(
                "Emulation.setFocusEmulationEnabled",
                json!({ "enabled": true }),
            )
            .await?;
        seite.rufen("Page.bringToFront", json!({})).await?;
        // Merkt sich, wann die Seite die Maus fangen wollte (für den Fall, dass Pointer-Lock kopflos nicht geht).
        seite
            .rufen(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": "window.__mausFangen = 0; const __rpl = Element.prototype.requestPointerLock; Element.prototype.requestPointerLock = function(...a) { window.__mausFangen++; window.__mausZiel = this.id; return __rpl.apply(this, a); };" }),
            )
            .await?;
        Ok(seite)
    }

    pub async fn rufen(&self, methode: &str, params: Value) -> Result<Value> {
        self.cdp.rufen(Some(&self.sitzung), methode, params).await
    }

    /// Für Befehle, die unter Last (Software-GPU, mehrere Browser) lange brauchen können.
    async fn langsam(&self, methode: &str, params: Value) -> Result<Value> {
        self.cdp
            .rufen_mit_zeit(
                Some(&self.sitzung),
                methode,
                params,
                Duration::from_secs(120),
            )
            .await
    }

    pub async fn gehe(&self, url: &str) -> Result<()> {
        self.langsam("Page.navigate", json!({ "url": url })).await?;
        self.warte_auf(
            "document.readyState === 'complete'",
            Duration::from_secs(20),
        )
        .await
    }

    /// Wertet `ausdruck` aus (Promises werden abgewartet) und gibt den Wert als JSON zurück.
    pub async fn js(&self, ausdruck: &str) -> Result<Value> {
        let r = self.rufen("Runtime.evaluate", json!({ "expression": ausdruck, "awaitPromise": true, "returnByValue": true, "userGesture": true })).await?;
        if let Some(f) = r.get("exceptionDetails") {
            bail!(
                "JavaScript-Fehler in `{ausdruck}`: {}",
                f["exception"]["description"]
                    .as_str()
                    .unwrap_or(&f.to_string())
            );
        }
        Ok(r["result"].get("value").cloned().unwrap_or(Value::Null))
    }

    /// Wartet, bis `bedingung` (JavaScript) wahr ist.
    pub async fn warte_auf(&self, bedingung: &str, zeit: Duration) -> Result<()> {
        let start = Instant::now();
        loop {
            if let Ok(Value::Bool(true)) = self.js(&format!("!!({bedingung})")).await {
                return Ok(());
            }
            if start.elapsed() > zeit {
                let konsole = self
                    .konsole
                    .lock()
                    .unwrap()
                    .iter()
                    .rev()
                    .take(15)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n  ");
                bail!(
                    "Zeitüberschreitung beim Warten auf `{bedingung}`.\n  Letzte Konsole:\n  {konsole}"
                );
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    }

    /// Wartet auf ein Element mit diesem Text (sichtbar im DOM, z. B. ein Knopf).
    pub async fn warte_auf_text(&self, text: &str, zeit: Duration) -> Result<()> {
        self.warte_auf(
            &format!(
                "document.body && document.body.innerText.includes({})",
                json!(text)
            ),
            zeit,
        )
        .await
    }

    /// Mitte eines Elements (CSS-Selektor), eines Elements mit genau diesem Text (`text=…`)
    /// oder eines Knopfs, der den Text enthält (`knopf=…`).
    async fn mitte(&self, ziel: &str) -> Result<(f64, f64)> {
        let finden = if let Some(t) = ziel.strip_prefix("knopf=") {
            // Ein sichtbarer Knopf, dessen Text `t` enthält.
            format!(
                "[...document.querySelectorAll('button,[role=button]')].filter(e => e.offsetParent !== null && e.innerText.includes({t})).pop()",
                t = json!(t)
            )
        } else {
            match ziel.strip_prefix("text=") {
                Some(t) => format!(
                    "[...document.querySelectorAll('button,a,[role=button],summary,label,li,span,div')].filter(e => e.offsetParent !== null && e.innerText && e.innerText.trim() === {t}).pop()",
                    t = json!(t)
                ),
                None => format!("document.querySelector({})", json!(ziel)),
            }
        };
        let v = self.js(&format!("(() => {{ const e = {finden}; if (!e) return null; e.scrollIntoView({{block:'center'}}); const r = e.getBoundingClientRect(); return [r.x + r.width/2, r.y + r.height/2]; }})()")).await?;
        let a = v
            .as_array()
            .with_context(|| format!("Element `{ziel}` nicht gefunden"))?;
        Ok((a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)))
    }

    /// Klickt mit der Maus (echte Geste) auf ein Element.
    pub async fn klick(&self, ziel: &str) -> Result<()> {
        let (x, y) = self.mitte(ziel).await?;
        self.klick_bei(x, y).await
    }

    pub async fn klick_bei(&self, x: f64, y: f64) -> Result<()> {
        self.rufen(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseMoved", "x": x, "y": y }),
        )
        .await?;
        for typ in ["mousePressed", "mouseReleased"] {
            self.rufen(
                "Input.dispatchMouseEvent",
                json!({ "type": typ, "x": x, "y": y, "button": "left", "clickCount": 1 }),
            )
            .await?;
        }
        Ok(())
    }

    /// Tippt Text in das Element mit dem Fokus.
    pub async fn tippe(&self, text: &str) -> Result<()> {
        self.rufen("Input.insertText", json!({ "text": text }))
            .await
            .map(|_| ())
    }

    /// Eine Taste: "Escape", "Enter", "Tab" oder ein einzelnes Zeichen wie "t".
    pub async fn taste(&self, taste: &str) -> Result<()> {
        let (code, vk, text) = match taste {
            "Escape" => ("Escape".to_string(), 27, None),
            "Enter" => ("Enter".to_string(), 13, Some("\r".to_string())),
            "Tab" => ("Tab".to_string(), 9, None),
            z if z.chars().count() == 1 => {
                let c = z.chars().next().unwrap_or(' ');
                (
                    format!("Key{}", c.to_ascii_uppercase()),
                    c.to_ascii_uppercase() as u32,
                    Some(z.to_string()),
                )
            }
            andere => bail!("Unbekannte Taste {andere}"),
        };
        let mut runter =
            json!({ "type": "keyDown", "key": taste, "code": code, "windowsVirtualKeyCode": vk });
        if let Some(t) = &text {
            runter["text"] = json!(t);
        }
        self.rufen("Input.dispatchKeyEvent", runter).await?;
        self.rufen(
            "Input.dispatchKeyEvent",
            json!({ "type": "keyUp", "key": taste, "code": code, "windowsVirtualKeyCode": vk }),
        )
        .await?;
        Ok(())
    }

    /// Ein Bildschirmfoto als PNG.
    pub async fn foto(&self, datei: &Path) -> Result<Vec<u8>> {
        let r = self
            .cdp
            .rufen_mit_zeit(
                Some(&self.sitzung),
                "Page.captureScreenshot",
                json!({ "format": "png" }),
                Duration::from_secs(120),
            )
            .await?;
        let png =
            base64::engine::general_purpose::STANDARD.decode(r["data"].as_str().unwrap_or(""))?;
        if let Some(d) = datei.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        std::fs::write(datei, &png)?;
        Ok(png)
    }
}
