//! Kleine git-/gh-Helfer: Branch und Remote eines Checkouts lesen, Repositories auflisten.

use std::path::{Path, PathBuf};

use protokoll::etagen::RepoAuswahl;
use serde::Deserialize;

/// Der .git-Ordner eines Checkouts (auch bei Worktrees, wo .git eine Datei "gitdir: …" ist).
fn git_ordner(ordner: &Path) -> Option<PathBuf> {
    let g = ordner.join(".git");
    if g.is_dir() {
        return Some(g);
    }
    let s = std::fs::read_to_string(&g).ok()?;
    let p = PathBuf::from(s.trim().strip_prefix("gitdir:")?.trim());
    Some(if p.is_absolute() { p } else { ordner.join(p) })
}

/// Branch des Checkouts ("HEAD" wenn losgelöst); keiner, wenn es kein git-Checkout ist.
pub fn branch(ordner: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_ordner(ordner)?.join("HEAD")).ok()?;
    Some(head.trim().strip_prefix("ref: refs/heads/").map(String::from).unwrap_or_else(|| "HEAD".into()))
}

/// Die URL von `origin`, aus .git/config (bei Worktrees aus dem gemeinsamen Ordner).
pub fn remote(ordner: &Path) -> Option<String> {
    let g = git_ordner(ordner)?;
    let gemeinsam = std::fs::read_to_string(g.join("commondir")).ok().map(|c| g.join(c.trim())).unwrap_or(g);
    let cfg = std::fs::read_to_string(gemeinsam.join("config")).ok()?;
    let mut in_origin = false;
    for zeile in cfg.lines().map(str::trim) {
        if zeile.starts_with('[') {
            in_origin = zeile == r#"[remote "origin"]"#;
        } else if in_origin && let Some(url) = zeile.strip_prefix("url").map(str::trim_start).and_then(|r| r.strip_prefix('=')) {
            return Some(url.trim().to_string());
        }
    }
    None
}

/// owner/name aus einer GitHub-URL (https oder ssh) oder schon in der Form owner/name.
pub fn repo_aus(eingabe: &str) -> Option<String> {
    let s = eingabe.trim().trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    let rest = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))
        .or_else(|| s.strip_prefix("git@github.com:"))
        .or_else(|| s.strip_prefix("ssh://git@github.com/"))
        .or_else(|| s.strip_prefix("github.com/"))
        .unwrap_or(s);
    let mut teile = rest.split('/');
    let (owner, name) = (teile.next()?, teile.next()?);
    let gut = |t: &str| !t.is_empty() && t.len() <= 100 && t.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && t != "." && t != "..";
    (teile.next().is_none() && gut(owner) && gut(name)).then(|| format!("{owner}/{name}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhRepo {
    name_with_owner: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    is_private: bool,
    #[serde(default)]
    pushed_at: Option<String>,
}

/// Die Repositories, die `gh` sehen kann (eigene zuerst nach letztem Push).
pub async fn repos_auflisten(umgebung: &[(String, String)]) -> Result<Vec<RepoAuswahl>, String> {
    let mut cmd = tokio::process::Command::new("gh");
    cmd.args(["repo", "list", "--limit", "200", "--json", "nameWithOwner,description,isPrivate,pushedAt"]);
    cmd.envs(umgebung.iter().map(|(k, v)| (k, v)));
    cmd.kill_on_drop(true);
    let aus = cmd.output().await.map_err(|e| format!("gh ließ sich nicht starten ({e}). Ist die GitHub-CLI installiert?"))?;
    if !aus.status.success() {
        let err = String::from_utf8_lossy(&aus.stderr);
        return Err(if err.contains("auth login") { "gh ist nicht angemeldet: `gh auth login` auf dem Rechner des Office".into() } else { format!("gh: {}", err.trim()) });
    }
    let mut repos: Vec<GhRepo> = serde_json::from_slice(&aus.stdout).map_err(|e| format!("gh antwortete unerwartet: {e}"))?;
    repos.sort_by(|a, b| b.pushed_at.cmp(&a.pushed_at));
    Ok(repos
        .into_iter()
        .map(|r| RepoAuswahl { name: r.name_with_owner, beschreibung: r.description.filter(|d| !d.is_empty()), privat: r.is_private, gepusht_um: r.pushed_at })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_formen() {
        assert_eq!(repo_aus("Luna-OS/office").as_deref(), Some("Luna-OS/office"));
        assert_eq!(repo_aus("https://github.com/a/b.git").as_deref(), Some("a/b"));
        assert_eq!(repo_aus("git@github.com:a/b.git").as_deref(), Some("a/b"));
        assert_eq!(repo_aus("a/b/c"), None);
        assert_eq!(repo_aus("a/.."), None);
        assert_eq!(repo_aus("a/b;rm"), None);
    }

    #[test]
    fn branch_und_remote() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path().join(".git");
        std::fs::create_dir(&g).unwrap();
        std::fs::write(g.join("HEAD"), "ref: refs/heads/rust-neubau\n").unwrap();
        std::fs::write(g.join("config"), "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = https://github.com/a/b.git\n").unwrap();
        assert_eq!(branch(dir.path()).as_deref(), Some("rust-neubau"));
        assert_eq!(remote(dir.path()).as_deref(), Some("https://github.com/a/b.git"));
    }
}
