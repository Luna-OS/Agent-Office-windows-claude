//! GitHub über die `gh`-CLI: Issues und Pull Requests einer Etage lesen, anlegen, kommentieren,
//! mergen, schließen, und den Pull Request eines Arbeiters öffnen.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use protokoll::ServerNachricht;
use protokoll::github::{Issue, Label, MergeArt, Pruefungen, PullRequest, Server as GhSrv};
use serde_json::Value;
use tokio::process::Command;

use crate::{Agenten, Ziel};

/// So oft werden Issues und PRs neu geholt.
const AUFFRISCHEN: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Default)]
pub struct GithubStand {
    pub issues: Vec<Issue>,
    pub prs: Vec<PullRequest>,
    pub fehler: Option<String>,
    pub laedt: bool,
}

impl GithubStand {
    pub fn nachricht(&self) -> ServerNachricht {
        ServerNachricht::Github(GhSrv::Stand { issues: self.issues.clone(), prs: self.prs.clone(), fehler: self.fehler.clone(), laedt: self.laedt })
    }
}

/// Führt `gh` in `dir` aus.
pub async fn gh(dir: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("gh");
    cmd.args(args).current_dir(dir).kill_on_drop(true).env("GH_PROMPT_DISABLED", "1").env("NO_COLOR", "1");
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let aus = tokio::time::timeout(Duration::from_secs(60), cmd.output())
        .await
        .context("gh antwortet nicht")?
        .context("Die GitHub-CLI `gh` ist nicht installiert")?;
    if !aus.status.success() {
        let fehler = String::from_utf8_lossy(&aus.stderr).trim().to_string();
        if fehler.contains("gh auth login") || fehler.contains("not logged") {
            bail!("gh ist nicht angemeldet – im Terminal `gh auth login` ausführen");
        }
        if fehler.contains("no git remotes") || fehler.contains("not a git repository") {
            bail!("Das Projekt hat kein GitHub-Repository");
        }
        bail!("gh: {}", if fehler.is_empty() { "fehlgeschlagen".to_string() } else { fehler });
    }
    Ok(String::from_utf8_lossy(&aus.stdout).trim().to_string())
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn labels(v: &Value) -> Vec<Label> {
    v.get("labels")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(|l| Label { name: text(l, "name"), farbe: format!("#{}", text(l, "color")) }).collect())
        .unwrap_or_default()
}

fn autor(v: &Value) -> String {
    v.pointer("/author/login").and_then(Value::as_str).unwrap_or("ghost").to_string()
}

pub fn issue_lesen(v: &Value) -> Issue {
    Issue {
        nummer: v.get("number").and_then(Value::as_u64).unwrap_or(0),
        titel: text(v, "title"),
        zustand: text(v, "state"),
        url: text(v, "url"),
        autor: autor(v),
        labels: labels(v),
        zugewiesen: v
            .get("assignees")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|x| text(x, "login")).collect())
            .unwrap_or_default(),
        erstellt: text(v, "createdAt"),
        geaendert: text(v, "updatedAt"),
        text: text(v, "body"),
        kommentare: v.get("comments").and_then(Value::as_array).map(|c| c.len() as u32).unwrap_or(0),
    }
}

pub fn pruefungen_lesen(v: &Value) -> Pruefungen {
    let Some(liste) = v.get("statusCheckRollup").and_then(Value::as_array).filter(|l| !l.is_empty()) else {
        return Pruefungen::Keine;
    };
    let zustaende: Vec<String> = liste
        .iter()
        .map(|c| {
            let s = c.get("conclusion").and_then(Value::as_str).filter(|s| !s.is_empty()).or_else(|| c.get("state").and_then(Value::as_str));
            s.or_else(|| c.get("status").and_then(Value::as_str)).unwrap_or("").to_ascii_uppercase()
        })
        .collect();
    if zustaende.iter().any(|s| matches!(s.as_str(), "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED")) {
        Pruefungen::Fehlgeschlagen
    } else if zustaende.iter().any(|s| matches!(s.as_str(), "PENDING" | "IN_PROGRESS" | "QUEUED" | "EXPECTED" | "WAITING" | "")) {
        Pruefungen::Laufen
    } else {
        Pruefungen::Bestanden
    }
}

pub fn pr_lesen(v: &Value) -> PullRequest {
    PullRequest {
        nummer: v.get("number").and_then(Value::as_u64).unwrap_or(0),
        titel: text(v, "title"),
        zustand: text(v, "state"),
        entwurf: v.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
        url: text(v, "url"),
        autor: autor(v),
        labels: labels(v),
        review: text(v, "reviewDecision"),
        branch: text(v, "headRefName"),
        ziel: text(v, "baseRefName"),
        erstellt: text(v, "createdAt"),
        geaendert: text(v, "updatedAt"),
        plus: v.get("additions").and_then(Value::as_u64).unwrap_or(0),
        minus: v.get("deletions").and_then(Value::as_u64).unwrap_or(0),
        pruefungen: pruefungen_lesen(v),
        text: text(v, "body"),
    }
}

async fn holen(dir: &Path) -> Result<(Vec<Issue>, Vec<PullRequest>)> {
    let issue_felder = "number,title,state,url,author,labels,assignees,createdAt,updatedAt,body,comments";
    let pr_felder = "number,title,state,isDraft,url,author,labels,reviewDecision,headRefName,baseRefName,createdAt,updatedAt,additions,deletions,statusCheckRollup,body";
    let issue_args = ["issue", "list", "--state", "open", "--limit", "200", "--json", issue_felder];
    let pr_args = ["pr", "list", "--state", "all", "--limit", "100", "--json", pr_felder];
    let (issues, prs) = tokio::join!(gh(dir, &issue_args), gh(dir, &pr_args));
    let issues: Vec<Value> = serde_json::from_str(&issues?).unwrap_or_default();
    let prs: Vec<Value> = serde_json::from_str(&prs?).unwrap_or_default();
    Ok((issues.iter().map(issue_lesen).collect(), prs.iter().map(pr_lesen).collect()))
}

/// Holt Issues und PRs einer Etage neu und schickt sie an die Etage.
pub async fn aktualisieren(ag: &Agenten, etage: &str) {
    let dir = {
        let mut z = ag.innen.zustand();
        let Some(e) = z.etagen.get_mut(etage) else { return };
        e.github.laedt = true;
        e.verzeichnis.clone()
    };
    let ergebnis = holen(&dir).await;
    let mut z = ag.innen.zustand();
    let Some(e) = z.etagen.get_mut(etage) else { return };
    e.github.laedt = false;
    match ergebnis {
        Ok((issues, prs)) => {
            e.github.issues = issues;
            e.github.prs = prs;
            e.github.fehler = None;
        }
        Err(f) => e.github.fehler = Some(f.to_string()),
    }
    ag.innen.senden(Ziel::Etage(etage.to_string()), e.github.nachricht());
    drop(z);
    crate::nach_merge::pruefen(ag, etage);
}

/// Holt den GitHub-Stand jetzt und dann regelmäßig, solange die Etage da ist.
pub fn aktualisieren_spaeter(ag: Agenten, etage: String) {
    ag.innen.laufzeit.clone().spawn(async move {
        loop {
            if !ag.innen.zustand().etagen.contains_key(&etage) {
                break;
            }
            aktualisieren(&ag, &etage).await;
            tokio::time::sleep(AUFFRISCHEN).await;
        }
    });
}

pub async fn issue_anlegen(dir: &Path, titel: &str, text: &str) -> Result<String> {
    gh(dir, &["issue", "create", "--title", titel, "--body", text]).await
}

pub async fn kommentieren(dir: &Path, nummer: u64, text: &str) -> Result<String> {
    gh(dir, &["issue", "comment", &nummer.to_string(), "--body", text]).await
}

pub async fn mergen(dir: &Path, nummer: u64, art: MergeArt, branch_loeschen: bool) -> Result<String> {
    let art = match art {
        MergeArt::Squash => "--squash",
        MergeArt::Merge => "--merge",
        MergeArt::Rebase => "--rebase",
    };
    let n = nummer.to_string();
    let mut args = vec!["pr", "merge", n.as_str(), art];
    if branch_loeschen {
        args.push("--delete-branch");
    }
    gh(dir, &args).await
}

pub async fn schliessen(dir: &Path, nummer: u64) -> Result<String> {
    let n = nummer.to_string();
    match gh(dir, &["issue", "close", &n]).await {
        Ok(s) => Ok(s),
        Err(_) => gh(dir, &["pr", "close", &n]).await,
    }
}

pub async fn issue_uebernehmen(dir: &Path, nummer: u64) -> Result<String> {
    gh(dir, &["issue", "edit", &nummer.to_string(), "--add-assignee", "@me"]).await
}

/// Pusht den Branch eines Worktrees und öffnet (oder findet) seinen Pull Request.
pub async fn pr_oeffnen(worktree: &Path, branch: &str, ziel: Option<&str>, titel: Option<&str>) -> Result<(u64, String)> {
    crate::worktree::git(worktree, &["push", "-u", "origin", branch]).await?;
    if let Ok(vorhanden) = gh(worktree, &["pr", "view", branch, "--json", "number,url"]).await
        && let Ok(v) = serde_json::from_str::<Value>(&vorhanden)
        && let Some(n) = v.get("number").and_then(Value::as_u64)
    {
        return Ok((n, text(&v, "url")));
    }
    let mut args = vec!["pr", "create", "--head", branch, "--fill"];
    if let Some(z) = ziel {
        args.extend(["--base", z]);
    }
    if let Some(t) = titel {
        args.extend(["--title", t]);
    }
    let url = gh(worktree, &args).await?;
    let url = url.lines().rev().find(|z| z.starts_with("http")).unwrap_or(&url).trim().to_string();
    let nummer = url.rsplit('/').next().and_then(|n| n.parse().ok()).unwrap_or(0);
    Ok((nummer, url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pr_und_pruefungen() {
        let v = json!({
            "number": 7, "title": "Fix", "state": "OPEN", "isDraft": false, "url": "https://x/pull/7",
            "author": {"login": "ada"}, "labels": [{"name": "bug", "color": "d73a4a"}],
            "headRefName": "office/ada-1", "baseRefName": "main", "additions": 3, "deletions": 1,
            "statusCheckRollup": [{"conclusion": "SUCCESS"}, {"status": "IN_PROGRESS", "conclusion": ""}]
        });
        let pr = pr_lesen(&v);
        assert_eq!(pr.nummer, 7);
        assert_eq!(pr.autor, "ada");
        assert_eq!(pr.labels[0].farbe, "#d73a4a");
        assert_eq!(pr.pruefungen, Pruefungen::Laufen);
        assert_eq!(pruefungen_lesen(&json!({"statusCheckRollup": [{"conclusion": "FAILURE"}]})), Pruefungen::Fehlgeschlagen);
        assert_eq!(pruefungen_lesen(&json!({"statusCheckRollup": [{"state": "SUCCESS"}]})), Pruefungen::Bestanden);
        assert_eq!(pruefungen_lesen(&json!({})), Pruefungen::Keine);
    }

    #[test]
    fn issue() {
        let i = issue_lesen(&json!({"number": 3, "title": "T", "assignees": [{"login": "bob"}], "comments": [{}, {}]}));
        assert_eq!(i.zugewiesen, ["bob"]);
        assert_eq!(i.kommentare, 2);
    }
}
