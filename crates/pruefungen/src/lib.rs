//! Projektweite Prüfungen (Dateigrößen-Wächter, nur Rust, deutsche Texte).
//!
//! Die Prüfungen laufen als normale Tests: `cargo test -p pruefungen`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Größte erlaubte Zeilenzahl einer `.rs`- oder `.css`-Datei unter `crates/`.
pub const MAX_ZEILEN: usize = 600;

/// Wurzel des Repositorys (zwei Ebenen über diesem Crate).
pub fn repo_wurzel() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("Repo-Wurzel nicht gefunden")
}

/// Alle versionierten Dateien (relativ zur Wurzel, mit `/`), die es auf der Platte gibt.
/// Nimmt `git ls-files`; ohne git wird der Verzeichnisbaum durchlaufen.
pub fn dateien() -> Vec<String> {
    let wurzel = repo_wurzel();
    let ausgabe = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(&wurzel)
        .output();
    let mut liste: Vec<String> = match ausgabe {
        Ok(a) if a.status.success() => String::from_utf8_lossy(&a.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => {
            let mut l = Vec::new();
            durchlaufe(&wurzel, &wurzel, &mut l);
            l
        }
    };
    liste.retain(|d| wurzel.join(d).is_file());
    liste.sort();
    liste.dedup();
    liste
}

fn durchlaufe(wurzel: &Path, ordner: &Path, liste: &mut Vec<String>) {
    let Ok(eintraege) = fs::read_dir(ordner) else {
        return;
    };
    for e in eintraege.flatten() {
        let pfad = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if pfad.is_dir() {
            if matches!(name.as_str(), "target" | "node_modules" | "dist" | ".git") {
                continue;
            }
            durchlaufe(wurzel, &pfad, liste);
        } else if let Ok(rel) = pfad.strip_prefix(wurzel) {
            liste.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn zeilen(pfad: &str) -> usize {
    fs::read_to_string(repo_wurzel().join(pfad))
        .map(|t| t.lines().count())
        .unwrap_or(0)
}

/// Dateien unter `crates/` mit `.rs`/`.css`, die länger als `max` Zeilen sind.
pub fn zu_lange_dateien(dateien: &[String], max: usize) -> Vec<(String, usize)> {
    dateien
        .iter()
        .filter(|d| d.starts_with("crates/") && (d.ends_with(".rs") || d.ends_with(".css")))
        .map(|d| (d.clone(), zeilen(d)))
        .filter(|(_, n)| *n > max)
        .collect()
}

/// Verbotene Skript-Dateien (`.ts`, `.js`, `.mjs`, `.py`, …) – außer unter `dist/` und `node_modules/`.
/// Solange das alte TypeScript-Projekt noch im Repo liegt (`package.json` ist versioniert),
/// wird nur unter `crates/` geprüft; danach im ganzen Repo.
pub fn fremde_sprachen(dateien: &[String]) -> Vec<String> {
    const ENDUNGEN: [&str; 8] = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".mts"];
    let altbestand = dateien.iter().any(|d| d == "package.json");
    dateien
        .iter()
        .filter(|d| !altbestand || d.starts_with("crates/"))
        .filter(|d| !d.starts_with("dist/") && !d.contains("node_modules/"))
        .filter(|d| ENDUNGEN.iter().any(|e| d.ends_with(e)))
        .cloned()
        .collect()
}

/// Strukturwächter: `main.rs` bleibt eine Installationsliste (kurz, kaum eigene Funktionen).
pub fn main_rs_probleme(pfad: &str, text: &str) -> Vec<String> {
    const MAX_ZEILEN_MAIN: usize = 150;
    const MAX_FUNKTIONEN: usize = 3;
    let mut probleme = Vec::new();
    let n = text.lines().count();
    if n > MAX_ZEILEN_MAIN {
        probleme.push(format!(
            "{pfad}: {n} Zeilen (erlaubt: {MAX_ZEILEN_MAIN}) – Code gehört in ein eigenes Modul"
        ));
    }
    let fns = text
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("fn ")
                || t.starts_with("async fn ")
                || t.starts_with("pub fn ")
                || t.starts_with("pub async fn ")
        })
        .count();
    if fns > MAX_FUNKTIONEN {
        probleme.push(format!("{pfad}: {fns} Funktionen (erlaubt: {MAX_FUNKTIONEN}) – main.rs soll nur Module einhängen"));
    }
    probleme
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keine_datei_ueber_600_zeilen() {
        let zu_lang = zu_lange_dateien(&dateien(), MAX_ZEILEN);
        assert!(
            zu_lang.is_empty(),
            "Diese Dateien sind länger als {MAX_ZEILEN} Zeilen (bitte aufteilen): {zu_lang:?}"
        );
    }

    #[test]
    fn nur_rust_keine_skriptdateien() {
        let fremd = fremde_sprachen(&dateien());
        assert!(fremd.is_empty(), "Nur Rust erlaubt, gefunden: {fremd:?}");
    }

    #[test]
    fn main_rs_bleibt_installationsliste() {
        let mut probleme = Vec::new();
        for pfad in ["crates/server/src/main.rs", "crates/client/src/main.rs"] {
            let Ok(text) = fs::read_to_string(repo_wurzel().join(pfad)) else {
                probleme.push(format!("{pfad}: fehlt"));
                continue;
            };
            probleme.extend(main_rs_probleme(pfad, &text));
        }
        assert!(probleme.is_empty(), "{}", probleme.join("\n"));
    }

    #[test]
    fn groessenwaechter_erkennt_lange_dateien() {
        // Selbsttest: die eigene Datei ist kurz genug, mit Grenze 1 aber zu lang.
        let eigene = vec!["crates/pruefungen/src/lib.rs".to_string()];
        assert!(zu_lange_dateien(&eigene, MAX_ZEILEN).is_empty());
        assert_eq!(zu_lange_dateien(&eigene, 1).len(), 1);
    }

    #[test]
    fn skriptwaechter_laesst_dist_und_altbestand_zu() {
        let liste = vec![
            "package.json".to_string(),
            "src/alt.ts".to_string(),
            "crates/x/foo.js".to_string(),
        ];
        assert_eq!(fremde_sprachen(&liste), vec!["crates/x/foo.js".to_string()]);
        let neu = vec!["dist/app.js".to_string(), "tools/a.py".to_string()];
        assert_eq!(fremde_sprachen(&neu), vec!["tools/a.py".to_string()]);
    }

    #[test]
    fn main_rs_waechter_meldet_zu_viel_code() {
        let viel = "fn a(){}\nfn b(){}\nfn c(){}\nfn d(){}\n";
        assert_eq!(main_rs_probleme("m.rs", viel).len(), 1);
        assert!(main_rs_probleme("m.rs", "fn main() {}\n").is_empty());
    }
}
