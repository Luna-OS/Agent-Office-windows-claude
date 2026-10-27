//! Das Register der Agent-Provider. Jeder Provider ist ein eigenes Modul mit einem `Provider`; die
//! Verwaltung fragt nie, welcher Provider ein Arbeiter ist. Ein neuer Provider: eigene Datei, eine
//! Zeile in `register()`.

pub(crate) mod claude;
mod weitere;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use protokoll::arbeiter::{Aktion, ArbeiterInfo, ProviderInfo};
use serde_json::Value;

/// Was ein Provider für einen Start bekommt.
pub struct StartEingabe<'a> {
    pub info: &'a ArbeiterInfo,
    /// Der erste Auftrag.
    pub prompt: Option<&'a str>,
    /// Die Sitzung, die fortgesetzt wird.
    pub fortsetzen: Option<&'a str>,
    /// Die Anleitung zur Zusammenarbeit (`office-agenten`), für den System-Prompt.
    pub anleitung: &'a str,
    /// Der Datenordner des Office, für Hook-Einstellungen und Heimatordner.
    pub daten: &'a Path,
    /// Pfad zur `office-agenten`-CLI, mit `/` als Trenner (Hooks rufen sie auf).
    pub cli: &'a str,
    /// Das Programm ist ein Windows-Skript (`.cmd`): keinen Freitext auf die Befehlszeile.
    pub skript: bool,
}

/// Wie ein Arbeiter gestartet wird.
#[derive(Debug, Default, Clone)]
pub struct StartPlan {
    pub argumente: Vec<String>,
    pub umgebung: HashMap<String, String>,
    /// Ein Auftrag, der erst ins laufende Terminal getippt wird (nach Sitzungsstart oder kurz danach).
    pub prompt_tippen: Option<String>,
}

/// Ein Hook-Ereignis, auf das Gemeinsame zurückgeführt.
#[derive(Debug, Clone, PartialEq)]
pub enum HookEreignis {
    /// Die Sitzung ist bereit; `geleert` nach `/clear`.
    SitzungStart { geleert: bool },
    /// Die Sitzungs-ID (zum Fortsetzen).
    Sitzung(String),
    /// Wo das Transkript der Sitzung liegt (für den Verbrauch).
    Transkript(String),
    Prompt(String),
    WerkzeugVor { beschreibung: String, aktion: Option<Aktion> },
    WerkzeugNach { fehlgeschlagen: bool, test: bool },
    /// Will eine Erlaubnis oder stellt eine Frage.
    BrauchtMenschen(String),
    /// Ein Hinweis, der nur auf einen Menschen wartet (Leerlauf-Hinweis).
    Leerlauf,
    Stopp,
}

/// Ein Agent-Provider.
pub trait Provider: Send + Sync {
    /// Kennung im Protokoll und in workers.json (`claude`, `codex`, ...).
    fn id(&self) -> &'static str;
    /// Anzeigename.
    fn name(&self) -> &'static str;
    /// Das Programm, als das er läuft.
    fn programm(&self) -> &'static str;
    fn modelle(&self) -> &'static [&'static str] {
        &[]
    }
    fn nimmt_aufwand(&self) -> bool {
        false
    }
    /// Er meldet seinen Sitzungsstart selbst über Hooks; bis dahin ist er `Startet`.
    fn meldet_start(&self) -> bool {
        false
    }
    /// Er meldet seine Arbeit über Hooks (sonst gilt der OSC-9;4-Fortschritt im Terminal).
    fn hat_hooks(&self) -> bool {
        false
    }
    /// Umgebungsvariablen eigener Sitzungen, die ein Arbeiter nicht erben darf.
    fn entfernen(&self) -> &'static [&'static str] {
        &[]
    }
    /// Befehlszeile und Umgebung für einen Start.
    fn starten(&self, e: &StartEingabe) -> StartPlan;
    /// Ein Hook-Ereignis seines Agenten, aufs Gemeinsame zurückgeführt.
    fn hook(&self, ereignis: &str, nutzlast: &Value) -> Vec<HookEreignis> {
        claude_artige_hooks(ereignis, nutzlast)
    }
    /// Was sein Bildschirm verrät, wenn er (noch) nicht benutzbar ist, auf Deutsch.
    fn blockiert(&self, _text: &str, _frueh: bool) -> Option<String> {
        None
    }
}

/// Alle Provider, in der Reihenfolge des Anlege-Fensters.
pub fn register() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(claude::Claude),
        Box::new(weitere::Codex),
        Box::new(weitere::OpenCode),
        Box::new(weitere::Gemini),
        Box::new(weitere::Grok),
        Box::new(weitere::Pi),
        Box::new(weitere::Eigen),
    ]
}

/// Der Provider mit dieser Kennung.
pub fn finden(id: &str) -> Option<Box<dyn Provider>> {
    register().into_iter().find(|p| p.id() == id)
}

pub const STANDARD: &str = "claude";

/// Die Provider für das Anlege-Fenster, mit der Angabe, ob ihr Programm hier installiert ist.
pub fn uebersicht() -> Vec<ProviderInfo> {
    register()
        .iter()
        .map(|p| ProviderInfo {
            id: p.id().into(),
            name: p.name().into(),
            verfuegbar: programm_von(p.as_ref()).is_some(),
            modelle: p.modelle().iter().map(|m| m.to_string()).collect(),
            aufwand: p.nimmt_aufwand(),
        })
        .collect()
}

/// Wo das Programm eines Providers liegt. `OFFICE_AGENT_<ID>` (z. B. `OFFICE_AGENT_CLAUDE`) überschreibt es.
pub fn programm_von(p: &dyn Provider) -> Option<PathBuf> {
    let var = format!("OFFICE_AGENT_{}", p.id().to_ascii_uppercase());
    match std::env::var(&var) {
        Ok(wert) if !wert.trim().is_empty() => crate::pty::programm_finden(wert.trim()),
        _ => crate::pty::programm_finden(p.programm()),
    }
}

/// Modelle und Aufwand landen auf der Befehlszeile: nur harmlose Zeichen, kein führendes `-`.
pub fn wert_gueltig(wert: &str) -> bool {
    !wert.is_empty()
        && wert.len() <= 256
        && !wert.starts_with('-')
        && wert.chars().all(|c| c.is_ascii_alphanumeric() || "._:/@+-".contains(c))
}

/// Was ein Werkzeugaufruf von außen aussieht.
pub fn aktion_von(werkzeug: &str, eingabe: &Value) -> Option<Aktion> {
    let w = werkzeug.to_ascii_lowercase();
    if w.contains("bash") || w.contains("shell") || w.contains("exec") {
        let befehl = eingabe.get("command").and_then(Value::as_str).unwrap_or("");
        return Some(if ist_test(befehl) { Aktion::Testen } else { Aktion::Schreiben });
    }
    if w.contains("web") || w.contains("fetch") || w.contains("search") && w.contains("web") {
        return Some(Aktion::Web);
    }
    if ["edit", "write", "multiedit", "notebookedit", "apply_patch", "patch"].iter().any(|n| w.contains(n)) {
        return Some(Aktion::Schreiben);
    }
    if ["read", "grep", "glob", "ls", "search", "view"].iter().any(|n| w.contains(n)) {
        return Some(Aktion::Lesen);
    }
    None
}

/// Ein Befehl, der Tests oder einen Build laufen lässt.
pub fn ist_test(befehl: &str) -> bool {
    let b = befehl.to_ascii_lowercase();
    ["cargo test", "cargo build", "cargo check", "cargo clippy", "npm test", "npm run test", "npm run build", "pytest", "go test", "jest", "vitest", "make", "tsc"]
        .iter()
        .any(|t| b.contains(t))
}

fn kuerzen(text: &str, max: usize) -> String {
    let einzeilig = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if einzeilig.chars().count() <= max {
        einzeilig
    } else {
        einzeilig.chars().take(max - 1).collect::<String>() + "…"
    }
}

/// Die Hooks von Claude Code; Codex spricht dieselben Ereignisse und Felder.
pub fn claude_artige_hooks(ereignis: &str, n: &Value) -> Vec<HookEreignis> {
    let mut aus = Vec::new();
    if let Some(s) = n.get("session_id").and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 160) {
        aus.push(HookEreignis::Sitzung(s.to_string()));
    }
    if let Some(t) = n.get("transcript_path").and_then(Value::as_str).filter(|t| !t.is_empty() && t.len() <= 4096) {
        aus.push(HookEreignis::Transkript(t.to_string()));
    }
    let werkzeug = n.get("tool_name").and_then(Value::as_str).unwrap_or("Werkzeug");
    let eingabe = n.get("tool_input").cloned().unwrap_or(Value::Null);
    let beschreibung = || {
        let detail = ["command", "file_path", "pattern", "url", "description"]
            .iter()
            .find_map(|k| eingabe.get(*k).and_then(Value::as_str))
            .unwrap_or("");
        kuerzen(&if detail.is_empty() { werkzeug.to_string() } else { format!("{werkzeug}: {detail}") }, 80)
    };
    match ereignis {
        "SessionStart" => aus.push(HookEreignis::SitzungStart { geleert: n.get("source").and_then(Value::as_str) == Some("clear") }),
        "UserPromptSubmit" => aus.push(HookEreignis::Prompt(n.get("prompt").and_then(Value::as_str).unwrap_or("").to_string())),
        "PreToolUse" if werkzeug == "AskUserQuestion" || werkzeug.ends_with("request_user_input") => {
            aus.push(HookEreignis::BrauchtMenschen("Stellt eine Frage".into()))
        }
        "PreToolUse" => aus.push(HookEreignis::WerkzeugVor { beschreibung: beschreibung(), aktion: aktion_von(werkzeug, &eingabe) }),
        "PostToolUse" | "PostToolUseFailure" => {
            let befehl = eingabe.get("command").and_then(Value::as_str).unwrap_or("");
            let fehler = ereignis == "PostToolUseFailure"
                || n.pointer("/tool_response/exit_code").and_then(Value::as_i64).is_some_and(|c| c != 0);
            aus.push(HookEreignis::WerkzeugNach { fehlgeschlagen: fehler, test: ist_test(befehl) })
        }
        "PermissionRequest" => aus.push(HookEreignis::BrauchtMenschen(format!("Will: {}", beschreibung()))),
        "Notification" => match n.get("notification_type").and_then(Value::as_str) {
            Some("permission_prompt") | Some("elicitation_dialog") => {
                aus.push(HookEreignis::BrauchtMenschen("Wartet auf eine Antwort".into()))
            }
            Some("idle_prompt") => aus.push(HookEreignis::Leerlauf),
            _ => {}
        },
        "Stop" => aus.push(HookEreignis::Stopp),
        _ => {}
    }
    aus
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn register_ist_vollstaendig_und_eindeutig() {
        let ids: Vec<_> = register().iter().map(|p| p.id()).collect();
        for id in ["claude", "codex", "opencode", "gemini", "grok", "pi", "eigen"] {
            assert!(ids.contains(&id), "{id} fehlt");
        }
        let mut sortiert = ids.clone();
        sortiert.sort();
        sortiert.dedup();
        assert_eq!(sortiert.len(), ids.len());
        assert_eq!(finden(STANDARD).unwrap().id(), "claude");
        assert!(finden("gibtsnicht").is_none());
        assert_eq!(uebersicht().len(), ids.len());
    }

    #[test]
    fn claude_hooks_werden_zurueckgefuehrt() {
        let e = claude_artige_hooks("PreToolUse", &json!({"session_id": "abc", "tool_name": "Bash", "tool_input": {"command": "cargo test -p x"}}));
        assert_eq!(e[0], HookEreignis::Sitzung("abc".into()));
        assert_eq!(e[1], HookEreignis::WerkzeugVor { beschreibung: "Bash: cargo test -p x".into(), aktion: Some(Aktion::Testen) });
        assert_eq!(claude_artige_hooks("Stop", &json!({})), vec![HookEreignis::Stopp]);
        assert!(matches!(claude_artige_hooks("PermissionRequest", &json!({"tool_name": "Edit"}))[0], HookEreignis::BrauchtMenschen(_)));
    }

    #[test]
    fn werte_fuer_die_befehlszeile() {
        assert!(wert_gueltig("opus"));
        assert!(wert_gueltig("openai/gpt-4.1"));
        assert!(!wert_gueltig("--dangerously"));
        assert!(!wert_gueltig("a b"));
        assert!(!wert_gueltig("x;rm"));
    }
}
