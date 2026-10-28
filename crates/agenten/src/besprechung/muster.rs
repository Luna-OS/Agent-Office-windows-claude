//! Die Muster des Besprechungsraums: wer welche Rolle hat, wie viele Runden, und was jeder Platz in
//! jeder Runde tut – samt dem Auftrag, den er dafür bekommt.

use protokoll::agenten::{Besprechung, Muster};

pub struct MusterDef {
    pub name: &'static str,
    pub wie: &'static str,
    pub rollen: &'static [&'static str],
    /// Plätze: wenigstens, höchstens, Standard.
    pub plaetze: (usize, usize, usize),
    /// Runden: wenigstens, höchstens, Standard.
    pub runden: (u32, u32, u32),
    pub braucht_pr: bool,
    pub braucht_teile: bool,
}

pub fn def(m: Muster) -> MusterDef {
    match m {
        Muster::Debatte => MusterDef {
            name: "Debatte",
            wie: "Jeder schlägt vor, dann kritisiert ihr einander; in der letzten Runde schreibt der Vorsitz die Entscheidung.",
            rollen: &[
                "Vorsitz",
                "Pragmatiker",
                "Skeptiker",
                "Vereinfacher",
                "Anwalt der Nutzer",
            ],
            plaetze: (2, 5, 3),
            runden: (2, 4, 3),
            braucht_pr: false,
            braucht_teile: false,
        },
        Muster::Leitung => MusterDef {
            name: "Leitung & Team",
            wie: "Die Leitung teilt die Aufgabe auf, jeder im Team erledigt einen Teil, die Leitung führt alles zusammen und schreibt es auf.",
            rollen: &[
                "Leitung",
                "Entwickler 1",
                "Entwickler 2",
                "Entwickler 3",
                "Entwickler 4",
            ],
            plaetze: (2, 5, 3),
            runden: (3, 3, 3),
            braucht_pr: false,
            braucht_teile: false,
        },
        Muster::MapReduce => MusterDef {
            name: "Map-Reduce",
            wie: "Dieselbe Aufgabe über jeden Teil parallel; der Kopf des Tisches fasst die Ergebnisse zusammen.",
            rollen: &[
                "Zusammenführer",
                "Bearbeiter 1",
                "Bearbeiter 2",
                "Bearbeiter 3",
                "Bearbeiter 4",
            ],
            plaetze: (2, 5, 3),
            runden: (2, 2, 2),
            braucht_pr: false,
            braucht_teile: true,
        },
        Muster::RotBlau => MusterDef {
            name: "Rot / Blau",
            wie: "Rot greift die Änderung an (Fehler, Sicherheit), Blau behebt, was stimmt, Runde um Runde; Blau schreibt es auf.",
            rollen: &["Blaues Team", "Rotes Team"],
            plaetze: (2, 2, 2),
            runden: (1, 5, 3),
            braucht_pr: false,
            braucht_teile: false,
        },
        Muster::Review => MusterDef {
            name: "Review-Runde",
            wie: "Jeder liest einen Pull Request aus seinem Blickwinkel; der Kopf des Tisches führt alles zu einem Review zusammen, das als Kommentar an den PR geht.",
            rollen: &[
                "Korrektheit",
                "Sicherheit",
                "Leistung & Einfachheit",
                "Tests",
                "API-Gestaltung",
            ],
            plaetze: (2, 5, 3),
            runden: (2, 2, 2),
            braucht_pr: true,
            braucht_teile: false,
        },
    }
}

/// Wohin die Ausgabe ohne eigenen Wunsch geht.
pub fn standard_ausgabe(m: Muster, slug: &str, pr: Option<u64>) -> String {
    match m {
        Muster::Debatte => format!("docs/entscheidungen/{slug}.md"),
        Muster::RotBlau => format!("docs/reviews/{slug}.md"),
        Muster::Review => format!("reviews/pr-{}.md", pr.map_or("n".into(), |p| p.to_string())),
        _ => format!("docs/besprechungen/{slug}.md"),
    }
}

/// Warum ein Ausgabepfad nicht geht.
pub fn ausgabe_problem(p: &str) -> Option<&'static str> {
    if p.trim().is_empty() {
        return Some("Sag, welche Datei die Besprechung schreibt");
    }
    if p.len() > 200 {
        return Some("Der Ausgabepfad ist zu lang");
    }
    if p.starts_with(['/', '\\']) || p.chars().nth(1) == Some(':') {
        return Some("Die Ausgabe liegt im Projekt: gib einen relativen Pfad an");
    }
    let teile: Vec<&str> = p.split(['/', '\\']).collect();
    if teile
        .iter()
        .any(|t| t.is_empty() || *t == "." || *t == "..")
    {
        return Some("Der Ausgabepfad darf keine leeren, . oder .. Teile haben");
    }
    if [".git", ".office", super::NOTIZEN].contains(&teile[0]) {
        return Some("Die Ausgabe darf nicht in .git/, .office/ oder den Notizen liegen");
    }
    if p.chars().any(char::is_control) {
        return Some("Der Ausgabepfad enthält Steuerzeichen");
    }
    None
}

/// Ein Teil eines Schritts, bevor er übergeben wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Teil {
    pub platz: usize,
    pub tut: String,
    pub datei: String,
    pub frage: String,
}

fn notiz(b: &Besprechung, runde: u32, platz: usize) -> String {
    format!(
        "{}/r{runde}-{}.md",
        b.notizen,
        crate::worktree::slug(&b.plaetze[platz].rolle, &platz.to_string())
    )
}

const AUSGABE: &str =
    "Schreib dabei nur in diese Datei, nicht in die Notizen; dann beende deinen Zug.";

/// Ob Rot in dieser Datei nichts mehr gefunden hat.
pub fn nichts_gefunden(inhalt: &str) -> bool {
    let t = inhalt
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .to_uppercase();
    t.starts_with("NO FINDINGS") || t.starts_with("KEINE FUNDE")
}

/// Die Teile von Runde `runde`, Schritt `schritt`. `lesen(datei)` liefert den Inhalt einer Notiz.
/// `None`: es gibt keinen solchen Schritt (weiter mit der nächsten Runde).
pub fn teile(
    b: &Besprechung,
    runde: u32,
    schritt: u32,
    lesen: &dyn Fn(&str) -> String,
) -> Option<Vec<Teil>> {
    if runde > b.runden {
        return None;
    }
    let n = b.plaetze.len();
    let alle = 0..n;
    let rolle = |i: usize| b.plaetze[i].rolle.clone();
    let teil = |platz: usize, tut: &str, datei: String, frage: String| Teil {
        platz,
        tut: tut.into(),
        datei,
        frage,
    };
    let letzte = runde == b.runden;
    let aus = &b.ausgabe;
    Some(match b.muster {
        Muster::Debatte if schritt > 0 => return None,
        Muster::Debatte if letzte => {
            let vorher: Vec<String> = alle.clone().map(|i| notiz(b, runde - 1, i)).collect();
            vec![teil(0, "schreibt die Entscheidung", aus.clone(), format!(
                "Lies alle Notizen in {}/ (die der letzten Runde: {}). Wäge Vorschläge und Kritik ab und schreib die Entscheidung nach {aus}: was entschieden wurde und warum, welche Wege verworfen wurden und warum, was offen bleibt. {AUSGABE}",
                b.notizen, vorher.join(", ")))]
        }
        Muster::Debatte if runde == 1 => alle
            .map(|i| { let d = notiz(b, 1, i); teil(i, "schlägt vor", d.clone(), format!(
                "Schlag deine Antwort vor, aus deiner Sicht als {}: was du tun würdest, warum, und was es kostet. Schreib sie nach {d}, dann beende deinen Zug.", rolle(i))) })
            .collect(),
        Muster::Debatte => alle.clone()
            .map(|i| {
                let andere: Vec<String> = alle.clone().filter(|j| *j != i).map(|j| notiz(b, runde - 1, j)).collect();
                let d = notiz(b, runde, i);
                teil(i, "kritisiert", d.clone(), format!(
                    "Lies die Notizen der anderen aus Runde {}: {}. Sag, wo sie irren oder etwas übersehen, dann gib deinen überarbeiteten Vorschlag. Schreib ihn nach {d}, dann beende deinen Zug.",
                    runde - 1, andere.join(", ")))
            })
            .collect(),
        Muster::Leitung | Muster::MapReduce | Muster::Review if schritt > 0 || runde > 3 => return None,
        Muster::Leitung if runde == 1 => {
            let team: Vec<String> = (1..n).map(rolle).collect();
            let d = format!("{}/plan.md", b.notizen);
            vec![teil(0, "plant", d.clone(), format!(
                "Lies die Aufgabe und den Code, den sie berührt, und teile die Arbeit in {} Teile, je einen für {}. Schreib den Plan nach {d}: je ein Abschnitt pro Person mit ihrer Rolle als Überschrift (z. B. „## {}“), was zu tun ist und welche Dateien ihr gehören, sodass nie zwei dieselbe Datei anfassen. Ändere selbst nichts. Dann beende deinen Zug.",
                n - 1, team.join(", "), rolle(1)))]
        }
        Muster::Leitung if runde == 2 => (1..n)
            .map(|i| { let d = notiz(b, 2, i); teil(i, "arbeitet", d.clone(), format!(
                "Lies {}/plan.md und erledige deinen Teil, den Abschnitt „## {}“. Ändere nur die Dateien, die er dir gibt, und committe nicht. Wenn du fertig bist, schreib nach {d}, was du getan hast und was die {} wissen muss (was nicht ging, wie du es geprüft hast), dann beende deinen Zug.",
                b.notizen, rolle(i), rolle(0))) })
            .collect(),
        Muster::Leitung => {
            let berichte: Vec<String> = (1..n).map(|i| notiz(b, 2, i)).collect();
            vec![teil(0, "führt zusammen", aus.clone(), format!(
                "Lies die Berichte des Teams ({}) und sieh dir ihre Änderungen an (git status, git diff). Bring zusammen, was nicht passt, und prüfe, dass es läuft (bauen, Tests). Dann schreib {aus}: was getan wurde, von wem, und wie es geprüft wurde. {AUSGABE} Committe nicht.",
                berichte.join(", ")))]
        }
        Muster::MapReduce | Muster::Review if runde > 2 => return None,
        Muster::MapReduce if runde == 1 => (1..n)
            .map(|i| {
                let meine: Vec<String> = b.teile.iter().enumerate().filter(|(k, _)| k % (n - 1) == i - 1).map(|(_, t)| format!("- {t}")).collect();
                let d = notiz(b, 1, i);
                teil(i, "bearbeitet", d.clone(), format!(
                    "Erledige die Aufgabe für deine Teile, und nur für diese:\n{}\nSchreib, was du gefunden oder getan hast, nach {d}, ein Abschnitt je Teil, dann beende deinen Zug.",
                    meine.join("\n")))
            })
            .collect(),
        Muster::MapReduce => {
            let ergebnisse: Vec<String> = (1..n).map(|i| notiz(b, 1, i)).collect();
            vec![teil(0, "fasst zusammen", aus.clone(), format!(
                "Lies die Ergebnisse ({}) und fasse sie in {aus} zusammen: ein Ergebnis, das sich als Ganzes liest, kein Stapel von Abschnitten. {AUSGABE}",
                ergebnisse.join(", ")))]
        }
        Muster::Review if runde == 1 => alle
            .map(|i| { let d = notiz(b, 1, i); let pr = b.pr.unwrap_or(0); teil(i, "prüft", d.clone(), format!(
                "Prüfe Pull Request #{pr} aus deinem Blickwinkel, {}, und nur aus diesem. Lies ihn mit gh pr view {pr} und gh pr diff {pr}; checke ihn nicht aus und ändere keine Dateien. Schreib deine Funde nach {d}, einer pro Stichpunkt: Datei:Zeile, was falsch ist und was zu tun ist, das Schwerste zuerst. Findest du nichts, schreib nur KEINE FUNDE. Dann beende deinen Zug.",
                rolle(i))) })
            .collect(),
        Muster::Review => {
            let funde: Vec<String> = alle.map(|i| notiz(b, 1, i)).collect();
            vec![teil(0, "führt das Review zusammen", aus.clone(), format!(
                "Lies die Funde aller Prüfer ({}). Streich Doppeltes und schreib ein gemeinsames Review nach {aus} in Markdown: zuerst eine kurze Zusammenfassung mit deinem Urteil, dann die Funde, das Schwerste zuerst, jeder mit seinem Blickwinkel in fetten Klammern wie **[{}]** und Datei:Zeile. Poste es nicht: das Office setzt es an den PR, sobald die Datei steht. {AUSGABE}",
                funde.join(", "), rolle(1)))]
        }
        Muster::RotBlau if schritt > 1 || runde > b.runden => return None,
        Muster::RotBlau if schritt == 0 => {
            let d = notiz(b, runde, 1);
            let vorher = if runde > 1 { format!(" Lies zuerst, was Blau zuletzt behoben hat ({}).", notiz(b, runde - 1, 0)) } else { String::new() };
            vec![teil(1, "greift an", d.clone(), format!(
                "Greif die Änderung, um die es geht, an wie ein Gegner: Fehler, Sicherheitslücken, unbehandelte Randfälle, kaputte Fehlerbehandlung. Lies den Code, ändere ihn nicht.{vorher} Schreib jeden Fund nach {d} mit Datei:Zeile, was schiefgeht und wie man es auslöst, das Schwerste zuerst. Findest du nichts mehr, das sich zu beheben lohnt, schreib nur KEINE FUNDE. Dann beende deinen Zug."))]
        }
        Muster::RotBlau => {
            let funde = notiz(b, runde, 1);
            if nichts_gefunden(&lesen(&funde)) || letzte {
                let schluss = if letzte && !nichts_gefunden(&lesen(&funde)) { format!("Behebe zuerst, was an den Funden in {funde} stimmt (committe nicht). Dann ") } else { format!("Rot hat in {funde} nichts mehr gefunden. ") };
                vec![teil(0, "schreibt es auf", aus.clone(), format!(
                    "{schluss}schreib {aus}: jeden Fund aus jeder Runde ({}/), was behoben wurde und wie, und was offen bleibt. {AUSGABE} Committe nicht.", b.notizen))]
            } else {
                let d = notiz(b, runde, 0);
                vec![teil(0, "behebt", d.clone(), format!(
                    "Lies die Funde des roten Teams in {funde} und behebe jeden, der stimmt, im Arbeitsordner (committe nicht). Schreib zu jedem nach {d}, was du getan hast oder warum es kein Problem ist. Dann beende deinen Zug."))]
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn besprechung(m: Muster, rollen: &[&str], runden: u32) -> Besprechung {
        Besprechung {
            id: "b".into(),
            muster: m,
            titel: "T".into(),
            auftrag: "A".into(),
            ausgabe: "out.md".into(),
            plaetze: rollen
                .iter()
                .map(|r| protokoll::agenten::Platz {
                    rolle: r.to_string(),
                    tisch: "t".into(),
                    arbeiter: None,
                    name: None,
                    tokens: 0,
                })
                .collect(),
            teile: vec!["a".into(), "b".into(), "c".into()],
            pr: Some(7),
            issue: None,
            provider: None,
            modell: None,
            aufwand: None,
            runden,
            runde: 1,
            schritt: 0,
            zuege: vec![],
            budget: 1,
            tokens: 0,
            status: protokoll::agenten::BesprechungStatus::Laeuft,
            grund: None,
            einberufen_von: "x".into(),
            begonnen_um: 0,
            beendet_um: None,
            worktree: None,
            notizen: ".besprechung".into(),
            commit: None,
            review_url: None,
            review_fehler: None,
            vorschau: None,
            abgeraeumt: false,
        }
    }

    /// (Runde, Schritt) → (Plätze, schreibt die Ausgabe)
    fn ablauf(
        b: &Besprechung,
        lesen: &dyn Fn(&str) -> String,
    ) -> Vec<(u32, u32, Vec<usize>, bool)> {
        let mut aus = vec![];
        for r in 1..=6 {
            for s in 0..3 {
                if let Some(t) = teile(b, r, s, lesen) {
                    aus.push((
                        r,
                        s,
                        t.iter().map(|t| t.platz).collect(),
                        t.iter().any(|t| t.datei == b.ausgabe),
                    ));
                }
            }
        }
        aus
    }

    #[test]
    fn muster_ablaeufe() {
        let leer = |_: &str| String::new();
        let d = besprechung(Muster::Debatte, &["V", "P", "S"], 3);
        assert_eq!(
            ablauf(&d, &leer),
            vec![
                (1, 0, vec![0, 1, 2], false),
                (2, 0, vec![0, 1, 2], false),
                (3, 0, vec![0], true)
            ]
        );
        let l = besprechung(Muster::Leitung, &["L", "E1", "E2"], 3);
        assert_eq!(
            ablauf(&l, &leer),
            vec![
                (1, 0, vec![0], false),
                (2, 0, vec![1, 2], false),
                (3, 0, vec![0], true)
            ]
        );
        let m = besprechung(Muster::MapReduce, &["Z", "B1", "B2"], 2);
        assert_eq!(
            ablauf(&m, &leer),
            vec![(1, 0, vec![1, 2], false), (2, 0, vec![0], true)]
        );
        assert!(teile(&m, 1, 0, &leer).unwrap()[0].frage.contains(
            "- a
- c"
        ));
        let r = besprechung(Muster::Review, &["K", "S"], 2);
        assert_eq!(
            ablauf(&r, &leer),
            vec![(1, 0, vec![0, 1], false), (2, 0, vec![0], true)]
        );
        let rb = besprechung(Muster::RotBlau, &["Blau", "Rot"], 2);
        let gefunden = |_: &str| "- a.rs:1 kaputt".to_string();
        assert_eq!(
            ablauf(&rb, &gefunden),
            vec![
                (1, 0, vec![1], false),
                (1, 1, vec![0], false),
                (2, 0, vec![1], false),
                (2, 1, vec![0], true)
            ]
        );
        let nichts = |_: &str| "KEINE FUNDE".to_string();
        assert!(teile(&rb, 1, 1, &nichts).unwrap()[0].datei == "out.md");
    }

    #[test]
    fn ausgabepfade() {
        assert!(ausgabe_problem("docs/x.md").is_none());
        assert!(ausgabe_problem("../x.md").is_some());
        assert!(ausgabe_problem("C:/x.md").is_some());
        assert!(ausgabe_problem(".git/x").is_some());
        assert!(nichts_gefunden("**KEINE FUNDE**"));
        assert!(nichts_gefunden("No findings."));
        assert!(!nichts_gefunden("- a.rs:3 kaputt"));
    }
}
