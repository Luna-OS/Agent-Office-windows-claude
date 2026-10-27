//! Claude Code: Hooks über eine Einstellungsdatei (`--settings`), die jedes Ereignis mit
//! `office-agenten hook claude <Ereignis>` an den Hook-Server meldet; die Anleitung zur
//! Zusammenarbeit kommt mit `--append-system-prompt`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Provider, StartEingabe, StartPlan, wert_gueltig};

pub struct Claude;

const EREIGNISSE: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "PermissionRequest",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
];

/// Schreibt die Hook-Einstellungen für alle Claude-Arbeiter und sagt, wo sie liegen.
pub fn hook_einstellungen(daten: &Path, cli: &str) -> std::io::Result<PathBuf> {
    let mut hooks = serde_json::Map::new();
    for ereignis in EREIGNISSE {
        let befehl = format!("\"{cli}\" hook claude {ereignis}");
        hooks.insert(
            ereignis.to_string(),
            json!([{ "hooks": [{ "type": "command", "command": befehl, "timeout": 10 }] }]),
        );
    }
    // Die eigene CLI darf ein Arbeiter ohne Nachfrage aufrufen: so redet er mit den anderen.
    let inhalt = json!({ "hooks": Value::Object(hooks), "permissions": { "allow": ["Bash(office-agenten:*)"] } });
    std::fs::create_dir_all(daten)?;
    let pfad = daten.join("claude-hooks.json");
    std::fs::write(&pfad, serde_json::to_vec_pretty(&inhalt)?)?;
    Ok(pfad)
}

impl Provider for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn name(&self) -> &'static str {
        "Claude Code"
    }
    fn programm(&self) -> &'static str {
        "claude"
    }
    fn modelle(&self) -> &'static [&'static str] {
        &["fable", "opus", "sonnet", "haiku"]
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
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_SSE_PORT",
            "CLAUDE_CODE_EXECPATH",
            "CLAUDE_PID",
            "CLAUDE_EFFORT",
        ]
    }

    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        let a = &mut plan.argumente;
        match hook_einstellungen(e.daten, e.cli) {
            Ok(pfad) => a.extend(["--settings".into(), pfad.to_string_lossy().into_owned()]),
            Err(fehler) => tracing::warn!("Claude-Hooks nicht geschrieben: {fehler}"),
        }
        if !e.skript {
            a.extend(["--append-system-prompt".into(), e.anleitung.to_string()]);
        }
        if let Some(m) = e.info.modell.as_deref().filter(|m| wert_gueltig(m)) {
            a.extend(["--model".into(), m.into()]);
        }
        if let Some(m) = e.info.aufwand.as_deref().filter(|m| wert_gueltig(m)) {
            a.extend(["--effort".into(), m.into()]);
        }
        // Der Aufgaben-Agent legt nur Aufgaben an: ohne diese Werkzeuge kann er keine Dateien ändern.
        if e.info.station.as_deref() == Some("aufgaben") {
            a.extend([
                "--disallowedTools".into(),
                "Edit".into(),
                "Write".into(),
                "NotebookEdit".into(),
            ]);
        }
        if let Some(s) = e.fortsetzen.filter(|s| wert_gueltig(s)) {
            a.extend(["--resume".into(), s.into()]);
        }
        let prompt = match (e.prompt, e.skript) {
            // Über ein .cmd liest cmd.exe die Befehlszeile: die Anleitung kommt dann mit dem Auftrag.
            (Some(p), true) => Some(format!("{}\n\n{p}", e.anleitung)),
            (None, true) if e.fortsetzen.is_none() => Some(e.anleitung.to_string()),
            (p, _) => p.map(str::to_string),
        };
        if e.skript {
            plan.prompt_tippen = prompt;
        } else if let Some(p) = prompt {
            // `--`, damit ein Auftrag wie "- fix login" nie als Option gilt.
            plan.argumente.extend(["--".into(), p]);
        }
        plan
    }

    fn blockiert(&self, text: &str, frueh: bool) -> Option<String> {
        let t = text.to_lowercase();
        if t.contains("not logged in")
            || t.contains("invalid api key")
            || t.contains("please run /login")
        {
            return Some("Claude ist auf diesem Rechner nicht angemeldet – Terminal öffnen und /login eingeben".into());
        }
        let einrichtung = [
            "trust this folder",
            "do you trust the files",
            "select login method",
            "choose the text style",
            "press enter to continue",
        ];
        if frueh && einrichtung.iter().any(|s| t.contains(s)) {
            return Some(
                "Wartet auf eine Einrichtungsfrage (Vertrauen/Login) – Terminal öffnen".into(),
            );
        }
        None
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use protokoll::arbeiter::{ArbeiterArt, ArbeiterInfo, ArbeiterStatus};

    pub fn info() -> ArbeiterInfo {
        ArbeiterInfo {
            id: "a1".into(),
            etage: "e".into(),
            art: ArbeiterArt::Agent,
            provider: Some("claude".into()),
            modell: Some("opus".into()),
            aufwand: None,
            tisch: "t1".into(),
            name: "Ada".into(),
            farbe: "#ffffff".into(),
            status: ArbeiterStatus::Startet,
            gesehen: true,
            wartet_seit: None,
            angelegt_von: "Luna".into(),
            angelegt_um: 0,
            prompt: None,
            worktree: None,
            pr: None,
            pr_oeffnet: false,
            titel: None,
            sitzung: None,
            exit_code: None,
            spalten: 120,
            zeilen: 32,
            zuschauer: vec![],
            aktivitaet: None,
            aktion: None,
            aufgabe: None,
            verbrauch: None,
            letzte_eingabe: None,
            repos: Vec::new(),
            station: None,
            besprechung: None,
            gearbeitet_ms: 0,
            arbeitet_seit: None,
            konto: None,
            hinweis: None,
        }
    }

    #[test]
    fn startplan() {
        let dir = tempfile::tempdir().unwrap();
        let i = info();
        let e = StartEingabe {
            info: &i,
            prompt: Some("- fix"),
            fortsetzen: None,
            anleitung: "ANLEITUNG",
            daten: dir.path(),
            cli: "C:/x/office-agenten.exe",
            skript: false,
        };
        let plan = Claude.starten(&e);
        let a = plan.argumente.join(" ");
        assert!(a.contains("--settings"));
        assert!(a.contains("--model opus"));
        assert!(a.ends_with("-- - fix"));
        let hooks = std::fs::read_to_string(dir.path().join("claude-hooks.json")).unwrap();
        assert!(hooks.contains("\\\"C:/x/office-agenten.exe\\\" hook claude Stop"));
        let e = StartEingabe { skript: true, ..e };
        let plan = Claude.starten(&e);
        assert!(plan.prompt_tippen.unwrap().starts_with("ANLEITUNG"));
    }
}
