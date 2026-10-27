//! Die Texte, die das Office selbst an Agenten schickt: die Anleitung zur Zusammenarbeit, der
//! Auftrag für ein GitHub-Issue, der Auftrag einer Aufgabe aus der Warteschlange. Jeder lässt sich
//! im UI umschreiben (gilt fürs ganze Gebäude, `<daten>/prompts.json`); `{{name}}` füllt das Office.

use std::collections::HashMap;
use std::path::Path;

use protokoll::agenten::PromptInfo;

pub struct Vorlage {
    pub id: &'static str,
    pub titel: &'static str,
    pub verwendet: &'static str,
    pub platzhalter: &'static [&'static str],
    pub standard: &'static str,
}

pub const MAX_TEXT: usize = 20_000;

pub const VORLAGEN: &[Vorlage] = &[
    Vorlage {
        id: "anleitung",
        titel: "Anleitung zur Zusammenarbeit",
        verwendet: "Bekommt jeder Agent beim Start (System-Prompt oder vor dem ersten Auftrag).",
        platzhalter: &["name", "etage"],
        standard: "Du bist „{{name}}“, ein Arbeiter im OpenAgenticOffice auf der Etage „{{etage}}“. Andere Agenten arbeiten \
gleichzeitig am selben Projekt. Ihr stimmt euch mit dem Befehl `office-agenten` ab (er ist im PATH):
- `office-agenten wer` – wer ist da und woran arbeitet jeder (Status, Branch, geänderte Dateien)
- `office-agenten senden <name|alle> \"<text>\"` – Nachricht an einen Agenten oder an alle
- `office-agenten posteingang` – deine ungelesenen Nachrichten
- `office-agenten status \"<text>\"` – melde in einem Satz, woran du gerade arbeitest
- `office-agenten aufgaben` – das gemeinsame Aufgabenboard
- `office-agenten aufgabe-neu \"<titel>\" [\"<notiz>\"]` – eine Aufgabe anlegen
- `office-agenten aufgabe <id> <offen|in_arbeit|erledigt|blockiert> [\"<notiz>\"]` – eine Aufgabe übernehmen oder abschließen
Regeln: Melde zu Beginn und bei jedem neuen Schritt kurz deinen Status. Bevor du Dateien änderst, an denen laut \
`office-agenten wer` ein anderer arbeitet, sprich dich mit ihm ab. Nachrichten an dich erscheinen als Eingabe, die mit \
„[Nachricht von …]“ beginnt: beantworte sie mit `office-agenten senden`. Wenn du fertig bist, sag es dem, der dich \
beauftragt hat.",
    },
    Vorlage {
        id: "issue",
        titel: "Auftrag für ein GitHub-Issue",
        verwendet: "Wenn ein Arbeiter für ein Issue angelegt wird.",
        platzhalter: &["nummer", "titel", "text"],
        standard: "Bearbeite GitHub-Issue #{{nummer}}: {{titel}}\n\n{{text}}\n\nWenn du fertig bist: committe, pushe deinen Branch \
und öffne einen Pull Request, der „Closes #{{nummer}}“ enthält.",
    },
    Vorlage {
        id: "warteschlange",
        titel: "Auftrag aus der Warteschlange",
        verwendet: "Wenn das Office eine automatische Aufgabe vom Board an einen neuen Arbeiter gibt.",
        platzhalter: &["titel", "notiz"],
        standard: "{{titel}}\n\n{{notiz}}\n\nDu arbeitest in deinem eigenen Git-Worktree. Wenn du fertig bist: committe, pushe \
und öffne einen Pull Request.",
    },
];

pub fn laden(daten: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(daten.join("prompts.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn speichern(daten: &Path, eigene: &HashMap<String, String>) {
    if let Ok(text) = serde_json::to_string_pretty(eigene) {
        let _ = std::fs::write(daten.join("prompts.json"), text);
    }
}

/// Der Text einer Vorlage, wie das Office ihn gerade hat, mit ausgefüllten Platzhaltern.
pub fn text(eigene: &HashMap<String, String>, id: &str, werte: &[(&str, &str)]) -> String {
    let roh = eigene.get(id).map(String::as_str).or_else(|| VORLAGEN.iter().find(|v| v.id == id).map(|v| v.standard)).unwrap_or("");
    let mut aus = roh.to_string();
    for (k, v) in werte {
        aus = aus.replace(&format!("{{{{{k}}}}}"), v);
    }
    // Leere Platzhalter hinterlassen keine Lücken aus Leerzeilen.
    while aus.contains("\n\n\n") {
        aus = aus.replace("\n\n\n", "\n\n");
    }
    aus.trim().to_string()
}

pub fn liste(eigene: &HashMap<String, String>) -> Vec<PromptInfo> {
    VORLAGEN
        .iter()
        .map(|v| PromptInfo {
            id: v.id.into(),
            titel: v.titel.into(),
            verwendet: v.verwendet.into(),
            platzhalter: v.platzhalter.iter().map(|p| p.to_string()).collect(),
            standard: v.standard.into(),
            eigener: eigene.get(v.id).cloned(),
        })
        .collect()
}

/// Setzt (oder mit `None`/leer: verwirft) den eigenen Text einer Vorlage.
pub fn setzen(eigene: &mut HashMap<String, String>, id: &str, text: Option<String>) -> Result<(), String> {
    if !VORLAGEN.iter().any(|v| v.id == id) {
        return Err(format!("Unbekannter Prompt „{id}“"));
    }
    match text.map(|t| t.replace("\r\n", "\n").trim().to_string()).filter(|t| !t.is_empty()) {
        Some(t) if t.chars().count() > MAX_TEXT => return Err("Der Prompt ist zu lang".into()),
        Some(t) => {
            eigene.insert(id.to_string(), t);
        }
        None => {
            eigene.remove(id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausfuellen_und_ueberschreiben() {
        let mut eigene = HashMap::new();
        let t = text(&eigene, "warteschlange", &[("titel", "Login"), ("notiz", "")]);
        assert!(t.starts_with("Login\n\nDu arbeitest"));
        setzen(&mut eigene, "warteschlange", Some("Mach {{titel}}!".into())).unwrap();
        assert_eq!(text(&eigene, "warteschlange", &[("titel", "X")]), "Mach X!");
        setzen(&mut eigene, "warteschlange", None).unwrap();
        assert!(eigene.is_empty());
        assert!(setzen(&mut eigene, "gibtsnicht", Some("x".into())).is_err());
        assert_eq!(liste(&eigene).len(), VORLAGEN.len());
    }
}
