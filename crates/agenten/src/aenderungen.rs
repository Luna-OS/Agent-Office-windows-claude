//! Das Änderungen-Fenster: was ein Arbeiter gegenüber dem Abzweig seines Branches geändert hat,
//! der Diff einer Datei, Committen und Verwerfen.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use protokoll::arbeiter::{Aenderungen, GeaenderteDatei};

use crate::worktree::git;

/// Mehr Dateien zeigt das Fenster nicht.
const MAX_DATEIEN: usize = 300;
/// Größere Diffs werden abgeschnitten (Bytes).
const MAX_DIFF: usize = 256 * 1024;

/// Ein Pfad aus dem Fenster muss im Arbeitsordner bleiben.
fn pfad_ok(datei: &str) -> bool {
    !datei.is_empty() && !Path::new(datei).is_absolute() && !datei.split(['/', '\\']).any(|t| t == "..") && !datei.starts_with('-')
}

/// Liest `git diff --numstat -z`-artige Zeilen: Datei → (plus, minus, binär).
fn zahlen(numstat: &str) -> HashMap<String, (u32, u32, bool)> {
    numstat
        .lines()
        .filter_map(|z| {
            let mut t = z.splitn(3, '\t');
            let (plus, minus, pfad) = (t.next()?, t.next()?, t.next()?);
            // Umbenennungen stehen als `alt => neu` oder `pfad/{alt => neu}`.
            let pfad = match pfad.split_once(" => ") {
                Some((links, rechts)) if links.contains('{') => {
                    let (vor, _) = links.split_once('{').unwrap_or((links, ""));
                    let (neu, nach) = rechts.split_once('}').unwrap_or((rechts, ""));
                    format!("{vor}{neu}{nach}").replace("//", "/")
                }
                Some((_, rechts)) => rechts.to_string(),
                None => pfad.to_string(),
            };
            Some((pfad, (plus.parse().unwrap_or(0), minus.parse().unwrap_or(0), plus == "-")))
        })
        .collect()
}

pub async fn lesen(arbeiter: &str, dir: &Path, basis: Option<&str>, pr_ziel: Option<String>) -> Aenderungen {
    let mut a = Aenderungen { arbeiter: arbeiter.to_string(), basis: basis.unwrap_or("HEAD").to_string(), pr_ziel, ..Default::default() };
    if let Err(f) = lesen_in(&mut a, dir).await {
        a.fehler = Some(f.to_string());
    }
    a
}

async fn lesen_in(a: &mut Aenderungen, dir: &Path) -> anyhow::Result<()> {
    a.branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await.ok();
    if a.basis != "HEAD" {
        a.voraus = git(dir, &["rev-list", "--count", &format!("{}..HEAD", a.basis)]).await?.trim().parse().unwrap_or(0);
        if a.voraus > 0 {
            a.betreff = git(dir, &["log", "-1", "--format=%s"]).await.ok();
        }
    }
    let status = git(dir, &["status", "--porcelain", "--untracked-files=all"]).await?;
    let mut offen: Vec<String> = Vec::new();
    let mut neu: Vec<String> = Vec::new();
    for z in status.lines().filter(|z| z.len() > 3) {
        let pfad = z[3..].rsplit(" -> ").next().unwrap_or("").trim_matches('"').to_string();
        if z.starts_with("??") {
            neu.push(pfad);
        } else {
            offen.push(pfad);
        }
    }
    let namen = git(dir, &["diff", "--name-status", "-M", &a.basis]).await?;
    let zahlen = zahlen(&git(dir, &["diff", "--numstat", "-M", &a.basis]).await?);
    let mut dateien = Vec::new();
    for z in namen.lines() {
        let teile: Vec<&str> = z.split('\t').collect();
        let (Some(art), Some(erster)) = (teile.first(), teile.get(1)) else { continue };
        let (von, pfad) = match teile.get(2) {
            Some(neu) => (Some(erster.to_string()), neu.to_string()),
            None => (None, erster.to_string()),
        };
        let (plus, minus, binaer) = zahlen.get(&pfad).copied().unwrap_or_default();
        let status = art.chars().next().unwrap_or('M').to_string();
        dateien.push(GeaenderteDatei { offen: offen.contains(&pfad), pfad, von, status, plus, minus, binaer });
    }
    for pfad in neu {
        let inhalt = tokio::fs::read(dir.join(&pfad)).await.unwrap_or_default();
        let binaer = inhalt.contains(&0);
        let plus = if binaer { 0 } else { String::from_utf8_lossy(&inhalt).lines().count() as u32 };
        dateien.push(GeaenderteDatei { pfad, von: None, status: "?".into(), plus, minus: 0, binaer, offen: true });
    }
    dateien.sort_by(|x, y| x.pfad.cmp(&y.pfad));
    a.weitere = dateien.len().saturating_sub(MAX_DATEIEN) as u32;
    dateien.truncate(MAX_DATEIEN);
    a.dateien = dateien;
    Ok(())
}

/// Der Diff einer Datei gegen die Basis; (Text, abgeschnitten).
pub async fn diff(dir: &Path, basis: Option<&str>, datei: &str) -> anyhow::Result<(String, bool)> {
    if !pfad_ok(datei) {
        anyhow::bail!("Ungültiger Dateipfad");
    }
    let basis = basis.unwrap_or("HEAD");
    let mut text = git(dir, &["diff", "-M", basis, "--", datei]).await?;
    if text.is_empty() {
        // Neu und nie committet: die ganze Datei als hinzugefügt.
        let inhalt = tokio::fs::read(dir.join(datei)).await?;
        if inhalt.contains(&0) {
            text = format!("Binärdatei {datei} ist neu.");
        } else {
            let zeilen: Vec<String> = String::from_utf8_lossy(&inhalt).lines().map(|z| format!("+{z}")).collect();
            text = format!("--- /dev/null\n+++ b/{datei}\n@@ -0,0 +1,{} @@\n{}", zeilen.len(), zeilen.join("\n"));
        }
    }
    let gekuerzt = text.len() > MAX_DIFF;
    if gekuerzt {
        let mut ende = MAX_DIFF;
        while !text.is_char_boundary(ende) {
            ende -= 1;
        }
        text.truncate(ende);
    }
    Ok((text, gekuerzt))
}

pub async fn committen(dir: &Path, nachricht: &str) -> anyhow::Result<()> {
    let nachricht = nachricht.trim();
    if nachricht.is_empty() {
        anyhow::bail!("Die Commit-Nachricht ist leer");
    }
    git(dir, &["add", "-A"]).await?;
    git(dir, &["commit", "-m", nachricht]).await?;
    Ok(())
}

/// Verwirft nicht committete Änderungen: einer Datei oder alle.
pub async fn verwerfen(dir: &Path, datei: Option<&str>) -> anyhow::Result<()> {
    match datei {
        Some(d) => {
            if !pfad_ok(d) {
                anyhow::bail!("Ungültiger Dateipfad");
            }
            let bekannt = git(dir, &["ls-files", "--error-unmatch", "--", d]).await.is_ok();
            if bekannt {
                git(dir, &["restore", "--staged", "--worktree", "--source=HEAD", "--", d]).await?;
            } else {
                git(dir, &["clean", "-f", "--", d]).await?;
            }
        }
        None => {
            git(dir, &["reset", "--hard", "HEAD"]).await?;
            git(dir, &["clean", "-fd"]).await?;
        }
    }
    Ok(())
}

/// Schickt den Stand des Änderungen-Fensters an eine Verbindung.
pub async fn stand_senden(ag: &crate::Agenten, verbindung: &str, etage: &str, id: &str) -> Result<(), String> {
    let (dir, basis, ziel, _) = ort(ag, etage, id)?;
    let stand = lesen(id, &dir, basis.as_deref(), ziel).await;
    let n = protokoll::ServerNachricht::Arbeiter(protokoll::arbeiter::Server::Aenderungen { stand });
    ag.innen.senden(crate::Ziel::Verbindung(verbindung.to_string()), n);
    Ok(())
}

/// Die Änderungen-Nachrichten aus dem Browser.
pub async fn verarbeiten(ag: &crate::Agenten, verbindung: &str, etage: &str, id: &str, m: protokoll::arbeiter::Client) -> Result<(), String> {
    use protokoll::arbeiter::Client as C;
    let (dir, basis, _, eigener) = ort(ag, etage, id)?;
    let f = |e: anyhow::Error| e.to_string();
    match m {
        C::DiffAbfragen { datei, .. } => {
            let (diff, gekuerzt, fehler) = match diff(&dir, basis.as_deref(), &datei).await {
                Ok((d, g)) => (d, g, None),
                Err(e) => (String::new(), false, Some(e.to_string())),
            };
            let n = protokoll::arbeiter::Server::Diff { arbeiter: id.to_string(), datei, diff, gekuerzt, fehler };
            ag.innen.senden(crate::Ziel::Verbindung(verbindung.to_string()), protokoll::ServerNachricht::Arbeiter(n));
            return Ok(());
        }
        C::Committen { nachricht, .. } => committen(&dir, &nachricht).await.map_err(f)?,
        C::Verwerfen { datei, .. } => {
            // Im gemeinsamen Projektordner verwirft niemand die Arbeit aller anderen.
            if !eigener {
                return Err("Verwerfen geht nur im eigenen Worktree eines Arbeiters".into());
            }
            verwerfen(&dir, datei.as_deref()).await.map_err(f)?
        }
        _ => {}
    }
    stand_senden(ag, verbindung, etage, id).await
}

/// Arbeitsordner, Basis und PR-Ziel eines Arbeiters.
pub fn ort(ag: &crate::Agenten, etage: &str, id: &str) -> Result<(PathBuf, Option<String>, Option<String>, bool), String> {
    let z = ag.innen.zustand();
    let e = z.etagen.get(etage).ok_or("Diese Etage gibt es nicht (mehr)")?;
    let a = e.arbeiter.get(id).ok_or("Diesen Arbeiter gibt es nicht")?;
    Ok(match &a.info.worktree {
        Some(wt) => (crate::worktree::absolut(&e.verzeichnis, &wt.pfad), Some(wt.basis.clone()), wt.von.clone(), true),
        None => (e.verzeichnis.clone(), None, None, false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfade_und_numstat() {
        assert!(pfad_ok("src/a.rs"));
        assert!(!pfad_ok("../x"));
        assert!(!pfad_ok("--force"));
        let z = zahlen("3\t1\tsrc/a.rs\n-\t-\tbild.png\n2\t0\tsrc/{alt => neu}/b.rs\n");
        assert_eq!(z["src/a.rs"], (3, 1, false));
        assert_eq!(z["bild.png"], (0, 0, true));
        assert_eq!(z["src/neu/b.rs"], (2, 0, false));
    }

    #[tokio::test]
    async fn lesen_diff_commit_verwerfen() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        for args in [&["init", "-q", "-b", "main"][..], &["config", "user.email", "t@e.de"], &["config", "user.name", "T"]] {
            git(p, args).await.unwrap();
        }
        std::fs::write(p.join("a.txt"), "eins\n").unwrap();
        git(p, &["add", "."]).await.unwrap();
        git(p, &["commit", "-qm", "start"]).await.unwrap();
        let basis = git(p, &["rev-parse", "HEAD"]).await.unwrap();
        std::fs::write(p.join("a.txt"), "eins\nzwei\n").unwrap();
        std::fs::write(p.join("neu.txt"), "x\ny\n").unwrap();
        let a = lesen("w", p, Some(&basis), None).await;
        assert_eq!(a.fehler, None);
        let namen: Vec<_> = a.dateien.iter().map(|d| (d.pfad.as_str(), d.status.as_str(), d.plus)).collect();
        assert_eq!(namen, [("a.txt", "M", 1), ("neu.txt", "?", 2)]);
        assert!(diff(p, Some(&basis), "a.txt").await.unwrap().0.contains("+zwei"));
        assert!(diff(p, Some(&basis), "neu.txt").await.unwrap().0.contains("+y"));
        verwerfen(p, Some("neu.txt")).await.unwrap();
        assert!(!p.join("neu.txt").exists());
        committen(p, "Zwei dazu").await.unwrap();
        let a = lesen("w", p, Some(&basis), None).await;
        assert_eq!(a.voraus, 1);
        assert_eq!(a.betreff.as_deref(), Some("Zwei dazu"));
        assert!(!a.dateien[0].offen);
        std::fs::write(p.join("a.txt"), "kaputt").unwrap();
        verwerfen(p, None).await.unwrap();
        assert_eq!(std::fs::read_to_string(p.join("a.txt")).unwrap().replace('\r', ""), "eins\nzwei\n");
    }
}
