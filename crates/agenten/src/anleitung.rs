//! Was jeder Agent beim Start über die Zusammenarbeit im Office erfährt.

use crate::arbeiter::status_wort;
use crate::etage::Etage;

/// Die Anleitung für den Arbeiter `id` auf der Etage `e`, mit allen, die gerade da sind.
pub fn fuer(e: &Etage, id: &str) -> String {
    let Some(ich) = e.arbeiter.get(id) else { return String::new() };
    let mut text = format!(
        "Du bist „{name}“, ein Arbeiter im OpenAgenticOffice auf der Etage „{etage}“. Andere Agenten arbeiten \
gleichzeitig am selben Projekt. Ihr stimmt euch mit dem Befehl `office-agenten` ab (er ist im PATH):\n\
- `office-agenten wer` – wer ist da und woran arbeitet jeder (Status, Branch, geänderte Dateien)\n\
- `office-agenten senden <name|alle> \"<text>\"` – Nachricht an einen Agenten oder an alle\n\
- `office-agenten posteingang` – deine ungelesenen Nachrichten\n\
- `office-agenten status \"<text>\"` – melde in einem Satz, woran du gerade arbeitest\n\
- `office-agenten aufgaben` – das gemeinsame Aufgabenboard\n\
- `office-agenten aufgabe-neu \"<titel>\" [\"<notiz>\"]` – eine Aufgabe anlegen\n\
- `office-agenten aufgabe <id> <offen|in_arbeit|erledigt|blockiert> [\"<notiz>\"]` – eine Aufgabe übernehmen oder abschließen\n\
Regeln: Melde zu Beginn und bei jedem neuen Schritt kurz deinen Status. Bevor du Dateien änderst, an denen laut \
`office-agenten wer` ein anderer arbeitet, sprich dich mit ihm ab. Nachrichten an dich erscheinen als Eingabe, die mit \
„[Nachricht von …]“ beginnt: beantworte sie mit `office-agenten senden`. Wenn du fertig bist, sag es dem, der dich \
beauftragt hat.",
        name = ich.info.name,
        etage = e.id,
    );
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
