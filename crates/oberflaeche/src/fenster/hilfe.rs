//! 🎮 Steuerung: alle Tasten und was sie tun. Eine neue Taste ist eine neue Zeile in [`ZEILEN`].

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};

const ZEILEN: &[(&str, &str)] = &[
    ("W A S D", "Gehen (Shift gedrückt halten zum Rennen)"),
    ("Leertaste", "Springen"),
    (
        "Maus",
        "In der Ich-Perspektive umsehen (Klick fängt die Maus, Esc gibt sie frei)",
    ),
    (
        "Klick / E",
        "Benutzen, worauf du schaust: Arbeiter anstellen, sein Terminal öffnen, ein Board lesen, ein Meeting einberufen, fernsehen, die Jukebox, sich hinsetzen",
    ),
    (
        "🛗",
        "Jedes Projekt ist eine Etage: im Aufzug an der Nordwand E drücken (oder oben links auf den Projektnamen klicken), um zu einer anderen zu fahren oder ein Projekt hinzuzufügen",
    ),
    (
        "📋",
        "Das Aufgabenboard der Etage: Menschen und Agenten legen Aufgaben an, übernehmen und erledigen sie. Aufgaben mit 🤖 vergibt das Office selbst an neue Arbeiter",
    ),
    (
        "📡",
        "Agenten: was sich die Arbeiter schreiben und wer gerade woran arbeitet. Du kannst selbst mitschreiben, an einen oder an alle",
    ),
    (
        "N",
        "Zum nächsten Arbeiter, der dich braucht (wartet am längsten)",
    ),
    (
        "P",
        "Auftrag an einen neuen oder vorhandenen Arbeiter am Schreibtisch vor dir",
    ),
    (
        "C",
        "Änderungen des Arbeiters vor dir: Dateien und Diff, Commit, Verwerfen, Pull Request",
    ),
    (
        "B",
        "Eine gemeinsame Shell an einem leeren Schreibtisch öffnen (Dev-Server, git, Tests)",
    ),
    ("R", "Einen schlafenden Arbeiter fortsetzen"),
    (
        "X",
        "Einen Arbeiter heimschicken (macht den Schreibtisch frei)",
    ),
    (
        "L",
        "Ein Schild über den Schreibtisch vor dir hängen, ändern oder abnehmen",
    ),
    (
        "O",
        "Pull Request für den Arbeiter öffnen oder den vorhandenen ansehen",
    ),
    (
        "🏰",
        "In der Burg (⚙️ Einstellungen → Karte): E am Thron öffnet das Terminal des Ersten in der Schlange, E an der Hand des Königs oder K stellt einen Arbeiter an einer freien Tafel ein, das Treppenloch im Süden führt in den Kerker",
    ),
    ("T", "Chat (Enter schickt, Esc bricht ab)"),
    (
        "G / 1–6",
        "Emote: winken, Daumen hoch, klatschen, tanzen, zeigen, Facepalm",
    ),
    ("/", "Suchen: Arbeiter, Aufgaben, Fenster, Leute"),
    (
        "Strg+K",
        "Befehlspalette: ein paar Buchstaben tippen, Enter öffnet",
    ),
    (
        "V",
        "Voice beitreten; im Voice V gedrückt halten zum Sprechen",
    ),
    ("M", "Mikrofon stumm schalten oder wieder an"),
    ("Tab", "Das ☰-Menü oben rechts: alle Fenster"),
    ("Esc", "Jedes Fenster schließen und sofort wieder umsehen"),
    (
        "Strg+[",
        "Stattdessen ein Esc an das Terminal schicken (z. B. um Claude zu unterbrechen). ⎋ im Terminal-Kopf tut dasselbe",
    ),
    ("F1 / ?", "Diese Hilfe"),
];

#[component]
pub fn Hilfe() -> impl IntoView {
    view! {
        <Rahmen fenster=Fenster::Hilfe titel="🎮 Steuerung" breite="max-w-2xl">
            <div class="grid grid-cols-[auto_1fr] items-baseline gap-x-4 gap-y-2 text-sm">
                {ZEILEN.iter().map(|(taste, was)| view! {
                    <kbd class="justify-self-start rounded-md border-2 border-foreground bg-secondary px-2 py-0.5 font-mono text-xs font-bold whitespace-nowrap">{*taste}</kbd>
                    <span>{*was}</span>
                }).collect_view()}
            </div>
        </Rahmen>
    }
}
