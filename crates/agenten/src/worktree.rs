//! Git-Worktrees für die Arbeiter: jeder bekommt `<projekt>/.office/worktrees/<name>-<id>` auf dem
//! Branch `office/<name>-<id>`, abgezweigt von dem, worauf das Projekt gerade steht.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use protokoll::arbeiter::{Aufraeumen, Worktree, WorktreeStand};
use tokio::process::Command;

pub const WORKTREE_ORDNER: &str = ".office/worktrees";
pub const BRANCH_PRAEFIX: &str = "office/";

/// Führt `git` in `dir` aus und gibt die Ausgabe zurück (Fehler mit der Meldung von git).
pub async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    git_mit(dir, args, &[]).await
}

/// Setzt eine Konto-Umgebung auf einen Befehl; ein leerer Wert entfernt die Variable.
pub fn umgebung_anwenden(cmd: &mut Command, umgebung: &[(String, String)]) {
    for (k, v) in umgebung {
        if v.is_empty() {
            cmd.env_remove(k);
        } else {
            cmd.env(k, v);
        }
    }
}

/// Wie [`git`], mit der Umgebung eines Kontos (dessen Git-/GitHub-Anmeldung).
pub async fn git_mit(dir: &Path, args: &[&str], umgebung: &[(String, String)]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir).kill_on_drop(true);
    umgebung_anwenden(&mut cmd, umgebung);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let aus = cmd.output().await.context("git ist nicht installiert oder nicht im PATH")?;
    if !aus.status.success() {
        let fehler = String::from_utf8_lossy(&aus.stderr).trim().to_string();
        bail!("git {}: {}", args.first().unwrap_or(&""), if fehler.is_empty() { "fehlgeschlagen".into() } else { fehler });
    }
    Ok(String::from_utf8_lossy(&aus.stdout).trim_end().to_string())
}

pub async fn ist_repo(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--is-inside-work-tree"]).await.is_ok_and(|s| s == "true")
}

/// Ein Name, wie er in Pfaden und Branches stehen darf.
pub fn slug(name: &str, id: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ä' => 'a',
            'ö' => 'o',
            'ü' => 'u',
            'ß' => 's',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    s = s.split('-').filter(|t| !t.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() {
        s = "arbeiter".into();
    }
    format!("{s}-{id}")
}

/// Legt einen neuen Worktree für einen Arbeiter an.
pub async fn erstellen(projekt: &Path, slug: &str) -> Result<Worktree> {
    let basis = git(projekt, &["rev-parse", "HEAD"]).await.context("Das Projekt hat noch keinen Commit")?;
    let von = git(projekt, &["rev-parse", "--abbrev-ref", "HEAD"]).await.ok().filter(|b| b != "HEAD");
    let rel = format!("{WORKTREE_ORDNER}/{slug}");
    let branch = format!("{BRANCH_PRAEFIX}{slug}");
    ausschliessen(projekt).await;
    git(projekt, &["worktree", "add", "-b", &branch, &rel, &basis]).await?;
    Ok(Worktree { pfad: rel, branch, basis, von })
}

/// `.office/` gehört nie ins Repository: steht in `.git/info/exclude`.
async fn ausschliessen(projekt: &Path) {
    let Ok(git_dir) = git(projekt, &["rev-parse", "--git-common-dir"]).await else { return };
    let datei = absolut(projekt, &git_dir).join("info").join("exclude");
    let alt = tokio::fs::read_to_string(&datei).await.unwrap_or_default();
    if !alt.lines().any(|z| z.trim() == ".office/") {
        let _ = tokio::fs::create_dir_all(datei.parent().unwrap_or(projekt)).await;
        let neu = format!("{}{}.office/\n", alt, if alt.is_empty() || alt.ends_with('\n') { "" } else { "\n" });
        let _ = tokio::fs::write(&datei, neu).await;
    }
}

pub fn absolut(projekt: &Path, pfad: &str) -> PathBuf {
    let p = Path::new(pfad);
    if p.is_absolute() { p.to_path_buf() } else { projekt.join(p) }
}

/// Der Branch, auf dem der Worktree gerade steht.
pub async fn aktueller_branch(dir: &Path) -> Option<String> {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await.ok().filter(|b| b != "HEAD")
}

/// Die geänderten Dateien (höchstens 50).
pub async fn geaenderte_dateien(dir: &Path) -> Result<Vec<String>> {
    let aus = git(dir, &["status", "--porcelain", "--untracked-files=all"]).await?;
    Ok(aus.lines().filter(|z| z.len() > 3).map(|z| z[3..].trim_matches('"').to_string()).collect())
}

/// Was der Worktree enthält.
pub async fn stand(projekt: &Path, wt: &Worktree) -> WorktreeStand {
    let abs = absolut(projekt, &wt.pfad);
    if !abs.is_dir() {
        return WorktreeStand { fehler: Some("Der Worktree-Ordner fehlt".into()), ..Default::default() };
    }
    let mut stand = WorktreeStand { vorhanden: true, ..Default::default() };
    match geaenderte_dateien(&abs).await {
        Ok(d) => {
            stand.geaendert = d.len() as u32;
            stand.dateien = d.into_iter().take(50).collect();
        }
        Err(e) => stand.fehler = Some(e.to_string()),
    }
    let zahl = |s: Result<String>| s.ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0);
    stand.voraus = zahl(git(&abs, &["rev-list", "--count", &format!("{}..HEAD", wt.basis)]).await);
    let mut args = vec!["rev-list", "--count", "HEAD", "--not", "--remotes"];
    if let Some(von) = wt.von.as_deref() {
        args.push(von);
    }
    stand.ungepusht = zahl(git(&abs, &args).await);
    stand
}

/// Räumt beim Heimschicken auf: Worktree-Ordner und/oder Branch. Sagt, was nicht ging.
pub async fn entfernen(projekt: &Path, wt: &Worktree, aufraeumen: Aufraeumen) -> Option<String> {
    if aufraeumen == Aufraeumen::Behalten {
        return None;
    }
    let mut fehler = Vec::new();
    if absolut(projekt, &wt.pfad).exists()
        && let Err(e) = git(projekt, &["worktree", "remove", "--force", &wt.pfad]).await
    {
        fehler.push(e.to_string());
    }
    let _ = git(projekt, &["worktree", "prune"]).await;
    if aufraeumen == Aufraeumen::Alles
        && wt.branch.starts_with(BRANCH_PRAEFIX)
        && let Err(e) = git(projekt, &["branch", "-D", &wt.branch]).await
    {
        fehler.push(e.to_string());
    }
    (!fehler.is_empty()).then(|| fehler.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q", "-b", "main"]).await.unwrap();
        git(p, &["config", "user.email", "test@example.com"]).await.unwrap();
        git(p, &["config", "user.name", "Test"]).await.unwrap();
        std::fs::write(p.join("a.txt"), "a").unwrap();
        git(p, &["add", "."]).await.unwrap();
        git(p, &["commit", "-qm", "start"]).await.unwrap();
        dir
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Jürgen Groß!", "a1"), "jurgen-gros-a1");
        assert_eq!(slug("???", "x"), "arbeiter-x");
    }

    #[tokio::test]
    async fn anlegen_stand_entfernen() {
        let dir = repo().await;
        let p = dir.path();
        assert!(ist_repo(p).await);
        let wt = erstellen(p, "ada-1").await.unwrap();
        assert_eq!(wt.branch, "office/ada-1");
        assert_eq!(wt.von.as_deref(), Some("main"));
        let abs = absolut(p, &wt.pfad);
        assert!(abs.join("a.txt").is_file());
        std::fs::write(abs.join("b.txt"), "b").unwrap();
        let s = stand(p, &wt).await;
        assert!(s.vorhanden);
        assert_eq!(s.geaendert, 1);
        assert_eq!(s.dateien, ["b.txt"]);
        assert_eq!(aktueller_branch(&abs).await.as_deref(), Some("office/ada-1"));
        assert!(std::fs::read_to_string(p.join(".git/info/exclude")).unwrap().contains(".office/"));
        assert_eq!(entfernen(p, &wt, Aufraeumen::Alles).await, None);
        assert!(!abs.exists());
        assert!(git(p, &["rev-parse", "--verify", "office/ada-1"]).await.is_err());
    }
}
