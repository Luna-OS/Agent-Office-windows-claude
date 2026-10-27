//! Die Board-Agenten: je ein Agent hinter einem kleinen Pult neben dem Issues-Board, dem PR-Board
//! und dem Aufgabenboard. Wer hingeht, tippt ihm eine Bitte; ist keiner da, wird er dafür angelegt –
//! mit einer Einweisung, was er an diesem Board tut. Er arbeitet im Projekt selbst (kein Worktree).

use protokoll::arbeiter::{ArbeiterArt, ArbeiterStatus};

use crate::{Agenten, start, steuerung};

/// Wie ein Board-Agent das Aufgabenboard bedient.
const BOARD_CLI: &str = "Das Aufgabenboard bedienst du mit `office-agenten`: `office-agenten aufgaben` zeigt es; \
`office-agenten aufgabe-neu --auto \"<titel>\" \"<vollständiger Auftrag>\"` legt eine Aufgabe an, die das Office selbst einem \
neuen Arbeiter in seinem eigenen Git-Worktree gibt – der Auftrag muss für sich allein reichen (was wo zu ändern ist, wie man \
es prüft, und dass am Ende ein Pull Request geöffnet wird); `office-agenten aufgabe-weg <id>` nimmt eine wartende Aufgabe \
vom Board.";

pub struct Station {
    /// Kennung, auch in `ArbeiterInfo::station`.
    pub art: &'static str,
    /// Sitz-ID des Pults in der Welt.
    pub tisch: &'static str,
    pub name: &'static str,
    pub farbe: &'static str,
    /// Die Prompt-Vorlage mit der Einweisung.
    pub vorlage: &'static str,
}

pub const STATIONEN: &[Station] = &[
    Station { art: "issues", tisch: "station-issues", name: "Issue-Agent", farbe: "#ef476f", vorlage: "station_issues" },
    Station { art: "pulls", tisch: "station-pulls", name: "PR-Agent", farbe: "#118ab2", vorlage: "station_pulls" },
    Station { art: "aufgaben", tisch: "station-queue", name: "Aufgaben-Agent", farbe: "#06d6a0", vorlage: "station_aufgaben" },
];

pub fn zu_tisch(tisch: &str) -> Option<&'static Station> {
    STATIONEN.iter().find(|s| s.tisch == tisch)
}

/// Eine Bitte an den Agenten am Pult `tisch`: getippt, geweckt oder – ist keiner da – angelegt.
pub async fn auftrag(ag: &Agenten, etage: &str, von: &str, konto: Option<String>, tisch: &str, text: &str) -> Result<(), String> {
    let s = zu_tisch(tisch).ok_or("An diesem Platz steht kein Board-Agent")?;
    let text = text.trim();
    if text.is_empty() {
        return Err("Sag, worum es geht".into());
    }
    let da = {
        let z = ag.innen.zustand();
        let e = z.etagen.get(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
        e.arbeiter.values().find(|a| a.info.tisch == tisch).map(|a| (a.info.id.clone(), a.info.status))
    };
    if let Some((id, status)) = da {
        if status == ArbeiterStatus::Wartet {
            return Err("Der Board-Agent wartet gerade auf eine Antwort im Terminal".into());
        }
        return steuerung::prompt(ag, etage, &id, text, von);
    }
    let einweisung = format!("{}\n\n{BOARD_CLI}", crate::prompts::text(&ag.innen.zustand().prompts, s.vorlage, &[]));
    let wunsch = start::Wunsch {
        tisch: Some(tisch.to_string()),
        prompt: Some(format!("{einweisung}\n\nDie erste Bitte, von {von}:\n{text}")),
        worktree: Some(false),
        art: ArbeiterArt::Agent,
        konto,
        station: Some(s.art.to_string()),
        ..Default::default()
    };
    let id = start::anlegen(ag, etage, von, wunsch).await?;
    let mut z = ag.innen.zustand();
    if let Some(a) = z.arbeiter_mut(etage, &id) {
        a.info.name = s.name.to_string();
        a.info.farbe = s.farbe.to_string();
        ag.innen.arbeiter_melden(&z.etagen[etage], &id);
    }
    Ok(())
}
