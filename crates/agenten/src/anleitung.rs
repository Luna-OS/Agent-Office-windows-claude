//! Was jeder Agent beim Start über die Zusammenarbeit im Office erfährt.

use std::collections::HashMap;

use crate::arbeiter::status_wort;
use crate::etage::Etage;

/// Die Anleitung für den Arbeiter `id` auf der Etage `e`, mit allen, die gerade da sind.
pub fn fuer(e: &Etage, id: &str, eigene: &HashMap<String, String>) -> String {
    let Some(ich) = e.arbeiter.get(id) else { return String::new() };
    let mut text = crate::prompts::text(eigene, "anleitung", &[("name", &ich.info.name), ("etage", &e.id)]);
    if let Some(wt) = &ich.info.worktree {
        text.push_str(&format!(" Du arbeitest in deinem eigenen Git-Worktree auf dem Branch `{}`.", wt.branch));
    }
    let andere: Vec<String> = e
        .arbeiter
        .values()
        .filter(|a| a.info.id != id)
        .map(|a| {
            let was = a.notiz.as_deref().or(a.info.aktivitaet.as_deref()).unwrap_or("");
            let p = a.info.provider.as_deref().unwrap_or("shell");
            if was.is_empty() {
                format!("{} ({p}, {})", a.info.name, status_wort(a.info.status))
            } else {
                format!("{} ({p}, {}: {was})", a.info.name, status_wort(a.info.status))
            }
        })
        .collect();
    if andere.is_empty() {
        text.push_str(" Gerade bist du allein auf der Etage.");
    } else {
        text.push_str(&format!(" Gerade da: {}.", andere.join("; ")));
    }
    text
}
