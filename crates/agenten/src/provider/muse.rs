//! Muse Code (Hooks wie Claude Code, Einstellungen in eigenen XDG-Ordnern des Office, damit kein
//! Arbeiter `~/.config/muse` ändert) und DeepSeek Harness (kein Terminal: ACP, siehe `acp.rs`).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::weitere::{auftrag, mit_anleitung};
use super::{HookEreignis, Provider, StartEingabe, StartPlan, claude_artige_hooks, wert_gueltig};

pub struct Muse;

const MUSE_EREIGNISSE: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionRequest",
    "Stop",
    "StopFailure",
];

fn nutzer_muse() -> Option<PathBuf> {
    let basis = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|h| PathBuf::from(h).join(".config"))
        })?;
    Some(basis.join("muse")).filter(|p| p.is_dir())
}

/// Schreibt den Muse-Heimatordner des Office (Einstellungen mit den Hooks) und sagt, wo er liegt.
pub fn heim_schreiben(daten: &Path, cli: &str) -> std::io::Result<PathBuf> {
    let heim = daten.join("muse-heim");
    let konfig = heim.join("config").join("muse");
    std::fs::create_dir_all(&konfig)?;
    std::fs::create_dir_all(heim.join("share"))?;
    std::fs::create_dir_all(heim.join("state"))?;
    let mut hooks = serde_json::Map::new();
    for e in MUSE_EREIGNISSE {
        let befehl = format!("\"{cli}\" hook muse {e}");
        hooks.insert(
            e.to_string(),
            json!([{ "hooks": [{ "type": "command", "command": befehl, "timeout": 5 }] }]),
        );
    }
    // Provider und Modell aus den eigenen Einstellungen des Menschen übernehmen, die Anmeldung auch.
    let eigene: Value = nutzer_muse()
        .and_then(|d| std::fs::read_to_string(d.join("settings.json")).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null);
    let mut einstellungen = json!({ "schema_version": 1, "provider": eigene.get("provider").cloned().unwrap_or(json!("meta")), "hooks": hooks });
    if let Some(m) = eigene.get("model") {
        einstellungen["model"] = m.clone();
    }
    std::fs::write(
        konfig.join("settings.json"),
        serde_json::to_vec_pretty(&einstellungen)?,
    )?;
    if let Some(anmeldung) = nutzer_muse()
        .map(|d| d.join("auth.json"))
        .filter(|p| p.is_file())
    {
        let _ = std::fs::copy(anmeldung, konfig.join("auth.json"));
    }
    Ok(heim)
}

impl Provider for Muse {
    fn id(&self) -> &'static str {
        "muse"
    }
    fn name(&self) -> &'static str {
        "Muse Code"
    }
    fn programm(&self) -> &'static str {
        "muse"
    }
    fn nimmt_aufwand(&self) -> bool {
        true
    }
    fn meldet_start(&self) -> bool {
        true
    }
    fn hat_hooks(&self) -> bool {
        true
    }
    fn entfernen(&self) -> &'static [&'static str] {
        &[
            "MUSE_BIN",
            "MUSE_AGENTS_THREAD",
            "MUSE_AGENTS_ROLE",
            "MUSE_PROJECTS_HOME",
        ]
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        match heim_schreiben(e.daten, e.cli) {
            Ok(heim) => {
                for (k, sub) in [
                    ("XDG_CONFIG_HOME", "config"),
                    ("XDG_DATA_HOME", "share"),
                    ("XDG_STATE_HOME", "state"),
                ] {
                    plan.umgebung
                        .insert(k.into(), heim.join(sub).to_string_lossy().into_owned());
                }
            }
            Err(f) => tracing::warn!("Muse-Heimatordner nicht geschrieben: {f}"),
        }
        plan.argumente.push("--trust-workspace".into());
        if let Some(s) = e.fortsetzen.filter(|s| wert_gueltig(s)) {
            plan.argumente.extend(["resume".into(), s.into()]);
            // Fortsetzen nimmt keinen Auftrag auf der Befehlszeile: er wird nach dem Start getippt.
            plan.prompt_tippen = e.prompt.map(str::to_string);
            return plan;
        }
        if let Some(m) = e.info.modell.as_deref().filter(|m| wert_gueltig(m)) {
            plan.argumente.extend(["--model".into(), m.into()]);
        }
        if let Some(a) = e.info.aufwand.as_deref().filter(|a| wert_gueltig(a)) {
            plan.argumente
                .extend(["--reasoning-effort".into(), a.into()]);
        }
        auftrag(&mut plan, e, mit_anleitung(e), &["--"]);
        plan
    }
    fn hook(&self, ereignis: &str, nutzlast: &Value) -> Vec<HookEreignis> {
        if nutzlast.get("agent_id").is_some() {
            return Vec::new();
        }
        let ereignis = if ereignis == "StopFailure" {
            "Stop"
        } else {
            ereignis
        };
        claude_artige_hooks(ereignis, nutzlast)
    }
}

/// DeepSeek Harness über sein ACP-Profil. Seine Sitzungen liegen im Datenordner des Office.
pub struct DeepSeek;

fn dsh_patch(daten: &Path) -> std::io::Result<PathBuf> {
    let wurzel = daten.join("dsh-sitzungen");
    std::fs::create_dir_all(&wurzel)?;
    let datei = daten.join("dsh-patch.yaml");
    let inhalt = format!(
        "# Vom Office geschrieben: DeepSeek-Harness-Sitzungen im Datenordner halten.\n- id: session-persistence-jsonl\n  config:\n    root: {}\n",
        serde_json::to_string(&wurzel.to_string_lossy()).unwrap_or_default()
    );
    std::fs::write(&datei, inhalt)?;
    Ok(datei)
}

impl Provider for DeepSeek {
    fn id(&self) -> &'static str {
        "dsh"
    }
    fn name(&self) -> &'static str {
        "DeepSeek Harness"
    }
    fn programm(&self) -> &'static str {
        "dsh"
    }
    fn acp(&self) -> bool {
        true
    }
    fn nimmt_aufwand(&self) -> bool {
        true
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let profil = std::env::var("OFFICE_DSH_PROFIL")
            .ok()
            .filter(|p| wert_gueltig(p))
            .unwrap_or_else(|| "acp".into());
        let mut plan = StartPlan::default();
        plan.argumente.extend(["--profile".into(), profil]);
        match dsh_patch(e.daten) {
            Ok(p) => plan
                .argumente
                .extend(["--patch".into(), p.to_string_lossy().into_owned()]),
            Err(f) => tracing::warn!("DeepSeek-Harness-Patch nicht geschrieben: {f}"),
        }
        // Über ACP: der erste Auftrag (mit der Anleitung davor) geht als session/prompt.
        plan.prompt_tippen = if e.fortsetzen.is_some() {
            e.prompt.map(str::to_string)
        } else {
            mit_anleitung(e)
        };
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::claude::tests::info;

    #[test]
    fn muse_und_deepseek_startplan() {
        let dir = tempfile::tempdir().unwrap();
        let i = info();
        let e = StartEingabe {
            info: &i,
            prompt: Some("Tu was"),
            fortsetzen: None,
            anleitung: "ANL",
            daten: dir.path(),
            cli: "/bin/oa",
            skript: false,
        };
        let m = Muse.starten(&e);
        assert!(m.argumente.contains(&"--trust-workspace".to_string()));
        assert_eq!(m.argumente.last().unwrap(), "ANL\n\nTu was");
        let s = std::fs::read_to_string(dir.path().join("muse-heim/config/muse/settings.json"))
            .unwrap();
        assert!(s.contains("hook muse StopFailure"));
        assert_eq!(
            Muse.hook("StopFailure", &json!({})),
            vec![HookEreignis::Stopp]
        );
        let d = DeepSeek.starten(&e);
        assert_eq!(
            d.argumente[..2],
            ["--profile".to_string(), "acp".to_string()]
        );
        assert_eq!(d.prompt_tippen.as_deref(), Some("ANL\n\nTu was"));
        assert!(dir.path().join("dsh-patch.yaml").is_file());
    }
}
