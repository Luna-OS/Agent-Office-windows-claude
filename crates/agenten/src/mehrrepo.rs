//! Arbeiter über mehrere Repositories: ein Arbeitsbereich `<projekt>/.office/worktrees/<slug>/` mit
//! einem Worktree je Repository (der eigenen Etage zuerst), alle auf dem Branch `office/<slug>`,
//! dazu eine Einweisung (CLAUDE.md und AGENTS.md), welcher Ordner welches Projekt ist. Alles oder
//! nichts: geht ein Repository nicht, wird nichts angelegt.

use std::path::{Path, PathBuf};

use protokoll::arbeiter::{ArbeiterInfo, Aufraeumen, PrVerweis, WeiteresRepo, Worktree};

use crate::worktree::{self, BRANCH_PRAEFIX, WORKTREE_ORDNER, git};
use crate::{Agenten, github};

/// Was das Office in den Arbeitsbereich schreibt (und beim Aufräumen wieder entfernt).
const EINWEISUNG: &[&str] = &["CLAUDE.md", "AGENTS.md"];

/// Ein Ordnername aus dem letzten Teil eines Pfads, mit -2, -3 … bei Doppelten.
pub fn ordnernamen(verzeichnisse: &[PathBuf]) -> Vec<String> {
    let mut namen: Vec<String> = Vec::new();
    for v in verzeichnisse {
        let basis = worktree::slug(
            &v.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            "",
        )
        .trim_end_matches('-')
        .to_string();
        let basis = if basis.is_empty() {
            "projekt".to_string()
        } else {
            basis
        };
        let mut name = basis.clone();
        let mut n = 2;
        while namen.contains(&name) {
            name = format!("{basis}-{n}");
            n += 1;
        }
        namen.push(name);
    }
    namen
}

/// Der Ordner, in dem ein Arbeiter über mehrere Repositories startet.
pub fn arbeitsbereich(projekt: &Path, info: &ArbeiterInfo) -> Option<PathBuf> {
    if info.repos.is_empty() {
        return None;
    }
    worktree::absolut(projekt, &info.worktree.as_ref()?.pfad)
        .parent()
        .map(Path::to_path_buf)
}

async fn worktree_an(
    repo: &Path,
    ziel: &Path,
    branch: &str,
) -> anyhow::Result<(String, Option<String>)> {
    let basis = git(repo, &["rev-parse", "HEAD"]).await?;
    let von = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .ok()
        .filter(|b| b != "HEAD");
    let ziel = ziel.to_string_lossy().into_owned();
    git(repo, &["worktree", "add", "-b", branch, &ziel, &basis]).await?;
    Ok((basis, von))
}

async fn git_verzeichnis(dir: &Path) -> Option<PathBuf> {
    let g = git(dir, &["rev-parse", "--git-common-dir"]).await.ok()?;
    std::fs::canonicalize(worktree::absolut(dir, &g)).ok()
}

/// Legt den Arbeitsbereich an. `andere`: (Etage, Checkout) der weiteren Repositories.
pub async fn anlegen(
    projekt: &Path,
    slug: &str,
    andere: &[(String, PathBuf)],
) -> Result<(Worktree, Vec<WeiteresRepo>), String> {
    let eigenes = git_verzeichnis(projekt)
        .await
        .ok_or("Das eigene Projekt ist kein Git-Repository")?;
    let mut gesehen = vec![eigenes];
    for (etage, dir) in andere {
        let g = git_verzeichnis(dir).await.ok_or(format!(
            "Das Projekt der Etage „{etage}“ ist kein Git-Repository"
        ))?;
        if gesehen.contains(&g) {
            return Err(format!(
                "Die Etage „{etage}“ hat dasselbe Repository wie eine andere"
            ));
        }
        gesehen.push(g);
    }
    let mut alle = vec![projekt.to_path_buf()];
    alle.extend(andere.iter().map(|(_, d)| d.clone()));
    let namen = ordnernamen(&alle);
    let bereich_rel = format!("{WORKTREE_ORDNER}/{slug}");
    let bereich = projekt.join(&bereich_rel);
    let branch = format!("{BRANCH_PRAEFIX}{slug}");
    let mut gemacht: Vec<(PathBuf, PathBuf)> = Vec::new();
    let ergebnis = async {
        let (basis, von) = worktree_an(projekt, &bereich.join(&namen[0]), &branch)
            .await
            .map_err(|e| e.to_string())?;
        gemacht.push((projekt.to_path_buf(), bereich.join(&namen[0])));
        let eigen = Worktree {
            pfad: format!("{bereich_rel}/{}", namen[0]),
            branch: branch.clone(),
            basis,
            von,
        };
        let mut weitere = Vec::new();
        for ((etage, dir), name) in andere.iter().zip(&namen[1..]) {
            let ziel = bereich.join(name);
            let (basis, von) = worktree_an(dir, &ziel, &branch)
                .await
                .map_err(|e| format!("{etage}: {e}"))?;
            gemacht.push((dir.clone(), ziel));
            let repo = github::gh(
                dir,
                &[
                    "repo",
                    "view",
                    "--json",
                    "nameWithOwner",
                    "-q",
                    ".nameWithOwner",
                ],
            )
            .await
            .ok();
            weitere.push(WeiteresRepo {
                etage: etage.clone(),
                name: name.clone(),
                repo,
                verzeichnis: dir.to_string_lossy().into_owned(),
                pfad: format!("{bereich_rel}/{name}"),
                branch: branch.clone(),
                basis,
                von,
                pr: None,
            });
        }
        einweisung_schreiben(&bereich, &namen[0], &weitere)
            .map_err(|e| format!("Die Einweisung ließ sich nicht schreiben: {e}"))?;
        Ok::<_, String>((eigen, weitere))
    }
    .await;
    if ergebnis.is_err() {
        for (repo, ziel) in gemacht {
            let ziel = ziel.to_string_lossy().into_owned();
            let _ = git(&repo, &["worktree", "remove", "--force", &ziel]).await;
            let _ = git(&repo, &["branch", "-D", &branch]).await;
        }
        let _ = std::fs::remove_dir_all(&bereich);
    }
    ergebnis
}

fn einweisung_schreiben(
    bereich: &Path,
    eigen: &str,
    weitere: &[WeiteresRepo],
) -> std::io::Result<()> {
    let mut text = format!(
        "# Arbeitsbereich über mehrere Repositories\n\nJeder Ordner hier ist ein eigener Git-Worktree auf demselben Branch. Committe und pushe in jedem Ordner für sich; jedes Repository bekommt seinen eigenen Pull Request.\n\n- `{eigen}/` – das Projekt deiner Etage\n"
    );
    for r in weitere {
        let projekt = r.repo.clone().unwrap_or_else(|| r.etage.clone());
        text.push_str(&format!(
            "- `{}/` – {projekt} (Etage „{}“)\n",
            r.name, r.etage
        ));
    }
    for datei in EINWEISUNG {
        std::fs::write(bereich.join(datei), &text)?;
    }
    Ok(())
}

/// Räumt beim Heimschicken jeden Worktree auf (je in seinem Repository) und dann den Arbeitsbereich.
pub async fn aufraeumen(
    projekt: &Path,
    info: &ArbeiterInfo,
    aufraeumen: Aufraeumen,
) -> Option<String> {
    let mut fehler = Vec::new();
    if let Some(wt) = &info.worktree
        && let Some(f) = worktree::entfernen(projekt, wt, aufraeumen).await
    {
        fehler.push(f);
    }
    for r in &info.repos {
        let wt = Worktree {
            pfad: worktree::absolut(projekt, &r.pfad)
                .to_string_lossy()
                .into_owned(),
            branch: r.branch.clone(),
            basis: r.basis.clone(),
            von: r.von.clone(),
        };
        if let Some(f) = worktree::entfernen(Path::new(&r.verzeichnis), &wt, aufraeumen).await {
            fehler.push(format!("{}: {f}", r.name));
        }
    }
    if aufraeumen != Aufraeumen::Behalten
        && let Some(bereich) = arbeitsbereich(projekt, info)
    {
        for datei in EINWEISUNG {
            let _ = std::fs::remove_file(bereich.join(datei));
        }
        // Nur wenn nichts anderes übrig ist.
        let _ = std::fs::remove_dir(&bereich);
    }
    (!fehler.is_empty()).then(|| fehler.join("; "))
}

/// Öffnet die Pull Requests der weiteren Repositories eines Arbeiters und trägt sie ein.
pub async fn prs_oeffnen(
    ag: &Agenten,
    etage: &str,
    id: &str,
    titel: Option<&str>,
    umgebung: &[(String, String)],
) -> Vec<String> {
    let (projekt, repos) = {
        let z = ag.innen.zustand();
        let Some(e) = z.etagen.get(etage) else {
            return Vec::new();
        };
        let Some(a) = e.arbeiter.get(id) else {
            return Vec::new();
        };
        (e.verzeichnis.clone(), a.info.repos.clone())
    };
    let mut fehler = Vec::new();
    for (i, r) in repos.iter().enumerate() {
        let pfad = worktree::absolut(&projekt, &r.pfad);
        match github::pr_oeffnen(&pfad, &r.branch, r.von.as_deref(), titel, umgebung).await {
            Ok((nummer, url)) => {
                let mut z = ag.innen.zustand();
                if let Some(a) = z.arbeiter_mut(etage, id)
                    && let Some(rr) = a.info.repos.get_mut(i)
                {
                    rr.pr = Some(PrVerweis { nummer, url });
                }
            }
            Err(e) => fehler.push(format!("{}: {e}", r.name)),
        }
    }
    fehler
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen_sind_eindeutig() {
        let n = ordnernamen(&[
            PathBuf::from("/a/web"),
            PathBuf::from("/b/api"),
            PathBuf::from("/c/web"),
        ]);
        assert_eq!(n, ["web", "api", "web-2"]);
    }

    #[tokio::test]
    async fn arbeitsbereich_anlegen_und_aufraeumen() {
        let mut repos = Vec::new();
        for name in ["web", "api"] {
            let d = tempfile::tempdir().unwrap();
            let p = d.path().join(name);
            std::fs::create_dir_all(&p).unwrap();
            for args in [
                &["init", "-q", "-b", "main"][..],
                &["config", "user.email", "t@e.de"],
                &["config", "user.name", "T"],
            ] {
                git(&p, args).await.unwrap();
            }
            std::fs::write(p.join("a.txt"), name).unwrap();
            git(&p, &["add", "."]).await.unwrap();
            git(&p, &["commit", "-qm", "x"]).await.unwrap();
            repos.push((d, p));
        }
        let (web, api) = (repos[0].1.clone(), repos[1].1.clone());
        let (eigen, weitere) = anlegen(&web, "ada-1", &[("api-etage".into(), api.clone())])
            .await
            .unwrap();
        assert_eq!(eigen.pfad, ".office/worktrees/ada-1/web");
        assert_eq!(weitere[0].name, "api");
        let bereich = web.join(".office/worktrees/ada-1");
        assert!(bereich.join("web/a.txt").is_file());
        assert_eq!(
            std::fs::read_to_string(bereich.join("api/a.txt")).unwrap(),
            "api"
        );
        assert!(
            std::fs::read_to_string(bereich.join("CLAUDE.md"))
                .unwrap()
                .contains("`api/`")
        );
        assert_eq!(
            git(&bereich.join("api"), &["rev-parse", "--abbrev-ref", "HEAD"])
                .await
                .unwrap(),
            "office/ada-1"
        );
        // Dasselbe Repository zweimal geht nicht.
        assert!(
            anlegen(&web, "bob-2", &[("x".into(), web.clone())])
                .await
                .is_err()
        );
        let mut info = crate::provider::claude::tests::info();
        info.worktree = Some(eigen);
        info.repos = weitere;
        assert_eq!(arbeitsbereich(&web, &info), Some(bereich.clone()));
        assert_eq!(aufraeumen(&web, &info, Aufraeumen::Alles).await, None);
        assert!(!bereich.exists());
        assert!(
            git(&api, &["rev-parse", "--verify", "office/ada-1"])
                .await
                .is_err()
        );
    }
}
