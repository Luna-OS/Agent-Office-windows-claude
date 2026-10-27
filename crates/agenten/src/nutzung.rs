//! Nutzung und Limits: der Verbrauch des ganzen Office (heute und insgesamt, in
//! `<daten>/nutzung.json`) und die Limits des Claude-Plans, die Claude Code auf eine
//! `get_usage`-Steueranfrage nennt (kostet nichts, startet kein Gespräch).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use protokoll::ServerNachricht;
use protokoll::agenten::{LimitFenster, Limits, Nutzung, Server as AgSrv};
use protokoll::arbeiter::Verbrauch;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{Agenten, Ziel, jetzt_ms, provider};

/// So oft werden die Limits gelesen.
const TAKT: Duration = Duration::from_secs(120);
/// Häufiger nicht, auch wenn jemand drängelt.
const MIN_ABSTAND_MS: u64 = 20_000;

/// Der Tag (UTC) zu einem Zeitpunkt in ms, als `JJJJ-MM-TT`.
pub fn tag(ms: u64) -> String {
    // Nach Howard Hinnants „civil_from_days“.
    let z = (ms / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn laden(daten: &Path) -> Nutzung {
    std::fs::read_to_string(daten.join("nutzung.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn addieren(v: &mut Verbrauch, ein: u64, aus: u64) {
    v.tokens_ein += ein;
    v.tokens_aus += aus;
}

/// Ein Arbeiter hat jetzt `neu` verbraucht, vorher `alt`: der Unterschied kommt aufs Office-Konto.
pub fn buchen(ag: &Agenten, alt: Option<&Verbrauch>, neu: &Verbrauch) {
    let ein = neu.tokens_ein.saturating_sub(alt.map_or(0, |a| a.tokens_ein));
    let aus = neu.tokens_aus.saturating_sub(alt.map_or(0, |a| a.tokens_aus));
    if ein == 0 && aus == 0 {
        return;
    }
    let heute = tag(jetzt_ms());
    let stand = {
        let mut z = ag.innen.zustand();
        let n = &mut z.nutzung;
        if n.tag != heute {
            n.tag = heute;
            n.heute = Verbrauch::default();
        }
        addieren(&mut n.gesamt, ein, aus);
        addieren(&mut n.heute, ein, aus);
        n.clone()
    };
    if let Ok(text) = serde_json::to_string_pretty(&stand) {
        let _ = std::fs::write(ag.innen.daten.join("nutzung.json"), text);
    }
    ag.innen.senden(Ziel::Alle, ServerNachricht::Agenten(AgSrv::Nutzung { stand }));
}

fn fenster(name: &str, w: Option<&Value>) -> Option<LimitFenster> {
    let w = w?;
    let prozent = w.get("utilization").and_then(Value::as_f64).filter(|p| p.is_finite())?;
    let neu_um = w.get("resets_at").and_then(Value::as_str).and_then(zeit_lesen);
    Some(LimitFenster { name: name.to_string(), prozent: prozent.clamp(0.0, 100.0), neu_um })
}

/// Liest einen RFC-3339-Zeitpunkt (`2026-10-27T18:00:00Z` oder mit `+hh:mm`) in ms.
pub fn zeit_lesen(t: &str) -> Option<u64> {
    let (datum, rest) = t.split_once('T')?;
    let mut d = datum.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, tag) = (d.next()??, d.next()??, d.next()??);
    let (uhr, versatz) = match rest.find(['Z', '+', '-']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "Z"),
    };
    let mut u = uhr.split(':');
    let h: i64 = u.next()?.parse().ok()?;
    let min: i64 = u.next()?.parse().ok()?;
    let s: f64 = u.next().unwrap_or("0").parse().ok()?;
    let versatz_min = match versatz.as_bytes().first() {
        Some(b'+') | Some(b'-') => {
            let (vh, vm) = versatz[1..].split_once(':').unwrap_or((&versatz[1..], "0"));
            let v = vh.parse::<i64>().ok()? * 60 + vm.parse::<i64>().ok()?;
            if versatz.starts_with('-') { -v } else { v }
        }
        _ => 0,
    };
    // Tage seit 1970 nach Hinnants „days_from_civil“.
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + tag - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let tage = era * 146_097 + doe - 719_468;
    let sek = tage * 86_400 + h * 3600 + min * 60 - versatz_min * 60;
    u64::try_from(sek).ok().map(|ganz| ((ganz as f64 + s) * 1000.0) as u64)
}

/// Wertet Claude Codes Antwort auf `get_usage` aus.
pub fn limits_lesen(antwort: &Value) -> Limits {
    let plan = antwort.get("subscription_type").and_then(Value::as_str).map(|p| p.chars().take(24).collect());
    let mut l = Limits { plan, fenster: Vec::new(), um: jetzt_ms() };
    let Some(rl) = antwort.get("rate_limits").filter(|r| r.is_object()) else { return l };
    if antwort.get("rate_limits_available") == Some(&Value::Bool(false)) {
        return l;
    }
    l.fenster.extend(fenster("5-Stunden-Sitzung", rl.get("five_hour")));
    l.fenster.extend(fenster("Woche", rl.get("seven_day")));
    if let Some(modelle) = rl.get("model_scoped").and_then(Value::as_array) {
        for m in modelle {
            if let Some(name) = m.get("display_name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()) {
                l.fenster.extend(fenster(&format!("{}-Woche", name.chars().take(24).collect::<String>()), Some(m)));
            }
        }
    }
    l
}

/// Fragt Claude Code nach den Plan-Limits.
async fn fragen() -> Option<Value> {
    let claude = provider::programm_von(provider::finden("claude")?.as_ref())?;
    let mut cmd = tokio::process::Command::new(claude);
    cmd.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--tools", ""])
        .args(["--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands", "--no-session-persistence"])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut kind = cmd.spawn().ok()?;
    let anfrage = json!({ "type": "control_request", "request_id": "usage", "request": { "subtype": "get_usage", "skip_behaviors": true } });
    let mut ein = kind.stdin.take()?;
    ein.write_all(format!("{anfrage}\n").as_bytes()).await.ok()?;
    let mut zeilen = BufReader::new(kind.stdout.take()?).lines();
    let antwort = tokio::time::timeout(Duration::from_secs(30), async {
        while let Ok(Some(z)) = zeilen.next_line().await {
            let Ok(v) = serde_json::from_str::<Value>(&z) else { continue };
            let r = v.get("response");
            if v.get("type").and_then(Value::as_str) == Some("control_response")
                && r.and_then(|r| r.get("request_id")).and_then(Value::as_str) == Some("usage")
            {
                return r.filter(|r| r.get("subtype").and_then(Value::as_str) == Some("success")).and_then(|r| r.get("response")).cloned();
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    drop(ein);
    let _ = kind.kill().await;
    antwort
}

/// Liest die Limits jetzt (höchstens alle 20 s) und schickt sie an alle.
pub async fn limits_auffrischen(ag: &Agenten) {
    {
        let mut z = ag.innen.zustand();
        if jetzt_ms().saturating_sub(z.limits_gefragt) < MIN_ABSTAND_MS {
            return;
        }
        z.limits_gefragt = jetzt_ms();
    }
    let Some(antwort) = fragen().await else { return };
    let stand = limits_lesen(&antwort);
    ag.innen.zustand().limits = stand.clone();
    ag.innen.senden(Ziel::Alle, ServerNachricht::Agenten(AgSrv::Limits { stand }));
}

pub fn starten(ag: &Agenten) {
    let schwach = ag.innen.selbst();
    ag.innen.laufzeit.spawn(async move {
        loop {
            let Some(innen) = schwach.upgrade() else { return };
            limits_auffrischen(&Agenten { innen }).await;
            tokio::time::sleep(TAKT).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tage_und_zeiten() {
        assert_eq!(tag(0), "1970-01-01");
        assert_eq!(tag(1_793_000_000_000), "2026-10-26");
        assert_eq!(zeit_lesen("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(zeit_lesen("2026-10-27T18:30:00+02:00"), zeit_lesen("2026-10-27T16:30:00Z"));
        assert_eq!(tag(zeit_lesen("2024-02-29T12:00:00Z").unwrap()), "2024-02-29");
        assert_eq!(zeit_lesen("2026-10-27T19:50:00.021830+00:00"), zeit_lesen("2026-10-27T19:50:00Z").map(|m| m + 21));
    }

    #[test]
    fn limits_aus_antwort() {
        let a = json!({
            "subscription_type": "max",
            "rate_limits": {
                "five_hour": {"utilization": 42.5, "resets_at": "2026-10-27T20:00:00Z"},
                "seven_day": {"utilization": 130},
                "model_scoped": [{"display_name": "Opus", "utilization": 10}]
            }
        });
        let l = limits_lesen(&a);
        assert_eq!(l.plan.as_deref(), Some("max"));
        let namen: Vec<_> = l.fenster.iter().map(|f| (f.name.as_str(), f.prozent)).collect();
        assert_eq!(namen, [("5-Stunden-Sitzung", 42.5), ("Woche", 100.0), ("Opus-Woche", 10.0)]);
        assert!(l.fenster[0].neu_um.is_some());
        assert!(limits_lesen(&json!({"rate_limits_available": false})).fenster.is_empty());
    }
}
