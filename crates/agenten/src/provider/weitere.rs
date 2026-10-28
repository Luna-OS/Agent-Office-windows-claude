//! Die übrigen Provider: Codex (mit eigenen Hooks), OpenCode, Gemini CLI, Grok, Pi und ein eigener
//! Befehl. Ohne Hooks gilt für ihren Status der OSC-9;4-Fortschritt im Terminal.

use serde_json::Value;

use super::{HookEreignis, Provider, StartEingabe, StartPlan, claude_artige_hooks, wert_gueltig};

/// Für Provider ohne System-Prompt-Option: die Anleitung kommt vor den ersten Auftrag.
pub(super) fn mit_anleitung(e: &StartEingabe) -> Option<String> {
    if e.fortsetzen.is_some() {
        return e.prompt.map(str::to_string);
    }
    Some(match e.prompt {
        Some(p) => format!("{}\n\n{p}", e.anleitung),
        None => format!("{}\n\nWarte auf deinen ersten Auftrag.", e.anleitung),
    })
}

/// Hängt den Auftrag an die Befehlszeile oder lässt ihn tippen (bei Windows-Skripten).
pub(super) fn auftrag(
    plan: &mut StartPlan,
    e: &StartEingabe,
    prompt: Option<String>,
    vorsatz: &[&str],
) {
    let Some(p) = prompt else { return };
    if e.skript {
        plan.prompt_tippen = Some(p);
    } else {
        plan.argumente.extend(vorsatz.iter().map(|s| s.to_string()));
        plan.argumente.push(p);
    }
}

pub(super) fn modell(plan: &mut StartPlan, e: &StartEingabe, option: &str) {
    if let Some(m) = e.info.modell.as_deref().filter(|m| wert_gueltig(m)) {
        plan.argumente.extend([option.to_string(), m.to_string()]);
    }
}

pub struct Codex;

const CODEX_EREIGNISSE: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Stop",
];

impl Provider for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn name(&self) -> &'static str {
        "Codex"
    }
    fn programm(&self) -> &'static str {
        "codex"
    }
    fn nimmt_aufwand(&self) -> bool {
        true
    }
    fn hat_hooks(&self) -> bool {
        true
    }
    fn meldet_start(&self) -> bool {
        true
    }
    fn entfernen(&self) -> &'static [&'static str] {
        &["CODEX_THREAD_ID", "CODEX_INTERNAL_ORIGINATOR_OVERRIDE"]
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        for ereignis in CODEX_EREIGNISSE {
            let befehl = format!("\"{}\" hook codex {ereignis}", e.cli);
            // Der Befehl als TOML-String (JSON-Quoting ist dafür gültig).
            let befehl = serde_json::to_string(&befehl).unwrap_or_default();
            plan.argumente.extend([
                "-c".into(),
                format!(
                    "hooks.{ereignis}=[{{hooks=[{{type=\"command\",command={befehl},timeout=5}}]}}]"
                ),
            ]);
        }
        plan.argumente.push("--no-alt-screen".into());
        modell(&mut plan, e, "--model");
        if let Some(a) = e.info.aufwand.as_deref().filter(|a| wert_gueltig(a)) {
            plan.argumente
                .extend(["-c".into(), format!("model_reasoning_effort=\"{a}\"")]);
        }
        if let Some(s) = e.fortsetzen.filter(|s| wert_gueltig(s)) {
            plan.argumente.extend(["resume".into(), s.into()]);
        }
        auftrag(&mut plan, e, mit_anleitung(e), &["--"]);
        plan
    }
    fn hook(&self, ereignis: &str, nutzlast: &Value) -> Vec<HookEreignis> {
        // Hooks seiner Unter-Agenten zählen nicht für ihn selbst.
        if nutzlast.get("agent_id").is_some() || nutzlast.get("agent_type").is_some() {
            return Vec::new();
        }
        claude_artige_hooks(ereignis, nutzlast)
    }
}

pub struct OpenCode;

impl Provider for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn name(&self) -> &'static str {
        "OpenCode"
    }
    fn programm(&self) -> &'static str {
        "opencode"
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        if let Some(s) = e.fortsetzen.filter(|s| wert_gueltig(s)) {
            plan.argumente.extend(["--session".into(), s.into()]);
        } else {
            modell(&mut plan, e, "--model");
        }
        auftrag(&mut plan, e, mit_anleitung(e), &["--prompt"]);
        plan
    }
}

pub struct Gemini;

impl Provider for Gemini {
    fn id(&self) -> &'static str {
        "gemini"
    }
    fn name(&self) -> &'static str {
        "Gemini CLI"
    }
    fn programm(&self) -> &'static str {
        "gemini"
    }
    fn modelle(&self) -> &'static [&'static str] {
        &["gemini-2.5-pro", "gemini-2.5-flash"]
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        modell(&mut plan, e, "--model");
        if e.fortsetzen.is_some() {
            plan.argumente.extend(["--resume".into(), "latest".into()]);
        }
        auftrag(&mut plan, e, mit_anleitung(e), &["--prompt-interactive"]);
        plan
    }
}

pub struct Grok;

impl Provider for Grok {
    fn id(&self) -> &'static str {
        "grok"
    }
    fn name(&self) -> &'static str {
        "Grok"
    }
    fn programm(&self) -> &'static str {
        "grok"
    }
    fn nimmt_aufwand(&self) -> bool {
        true
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        plan.argumente
            .extend(["--no-alt-screen".into(), "--trust".into()]);
        if let Some(s) = e.fortsetzen.filter(|s| wert_gueltig(s)) {
            plan.argumente.extend(["--resume".into(), s.into()]);
        } else {
            modell(&mut plan, e, "--model");
            if let Some(a) = e.info.aufwand.as_deref().filter(|a| wert_gueltig(a)) {
                plan.argumente.extend(["--effort".into(), a.into()]);
            }
        }
        auftrag(&mut plan, e, mit_anleitung(e), &["--"]);
        plan
    }
}

pub struct Pi;

impl Provider for Pi {
    fn id(&self) -> &'static str {
        "pi"
    }
    fn name(&self) -> &'static str {
        "Pi"
    }
    fn programm(&self) -> &'static str {
        "pi"
    }
    fn nimmt_aufwand(&self) -> bool {
        true
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        let mut plan = StartPlan::default();
        if e.fortsetzen.is_some() {
            plan.argumente.push("--continue".into());
        }
        modell(&mut plan, e, "--model");
        if let Some(a) = e.info.aufwand.as_deref().filter(|a| wert_gueltig(a)) {
            plan.argumente.extend(["--thinking".into(), a.into()]);
        }
        auftrag(&mut plan, e, mit_anleitung(e), &[]);
        plan
    }
}

/// Ein eigener Agent: Programm aus `OFFICE_AGENT_EIGEN`, der Auftrag wird ins Terminal getippt.
pub struct Eigen;

impl Provider for Eigen {
    fn id(&self) -> &'static str {
        "eigen"
    }
    fn name(&self) -> &'static str {
        "Eigener Befehl"
    }
    fn programm(&self) -> &'static str {
        ""
    }
    fn starten(&self, e: &StartEingabe) -> StartPlan {
        StartPlan {
            prompt_tippen: mit_anleitung(e),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::claude::tests::info;

    fn eingabe<'a>(
        i: &'a protokoll::arbeiter::ArbeiterInfo,
        dir: &'a std::path::Path,
    ) -> StartEingabe<'a> {
        StartEingabe {
            info: i,
            prompt: Some("Tu was"),
            fortsetzen: None,
            anleitung: "ANL",
            daten: dir,
            cli: "/bin/office-agenten",
            skript: false,
        }
    }

    #[test]
    fn codex_hooks_und_auftrag() {
        let dir = tempfile::tempdir().unwrap();
        let i = info();
        let plan = Codex.starten(&eingabe(&i, dir.path()));
        let a = plan.argumente.join(" ");
        assert!(a.contains("hooks.Stop=[{hooks=[{type=\"command\",command=\"\\\"/bin/office-agenten\\\" hook codex Stop\""));
        assert_eq!(plan.argumente.last().unwrap(), "ANL\n\nTu was");
        assert!(
            Codex
                .hook("Stop", &serde_json::json!({"agent_id": "x"}))
                .is_empty()
        );
    }

    #[test]
    fn skript_tippt_den_auftrag() {
        let dir = tempfile::tempdir().unwrap();
        let i = info();
        let e = StartEingabe {
            skript: true,
            ..eingabe(&i, dir.path())
        };
        let plan = OpenCode.starten(&e);
        assert!(!plan.argumente.iter().any(|a| a.contains("Tu was")));
        assert_eq!(plan.prompt_tippen.as_deref(), Some("ANL\n\nTu was"));
    }
}
