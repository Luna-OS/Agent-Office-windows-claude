//! Das Bücherregal: alle Markdown-Dateien im Projekt einer Etage, zum Lesen im Office, dazu die
//! Bilder, die sie zeigen. git sagt, welche Dateien zum Projekt gehören (erfasst oder neu und nicht
//! ignoriert); ohne git wird der Ordner durchsucht, ohne versteckte und Build-Ordner.

use std::path::{Component, Path, PathBuf};

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Route, Zugriff, route};
use crate::office::OfficeRef;

const MAX_BUECHER: usize = 5000;
const MAX_BUCH: u64 = 2 * 1024 * 1024;
const MAX_BILD: u64 = 10 * 1024 * 1024;
const MAX_TIEFE: usize = 12;
const UEBERSPRINGEN: [&str; 9] = ["node_modules", "dist", "build", "out", "target", "vendor", "coverage", "__pycache__", "venv"];

pub fn routen() -> Vec<Route> {
    vec![
        route("/api/buecher", Zugriff::Sitzung, get(liste)),
        route("/api/buch", Zugriff::Sitzung, get(buch)),
        route("/api/buch/bild", Zugriff::Sitzung, get(bild)),
    ]
}

fn fehler(code: StatusCode, text: &str) -> Response {
    (code, Json(json!({ "fehler": text }))).into_response()
}

fn ist_buch(p: &str) -> bool {
    let k = p.to_lowercase();
    k.ends_with(".md") || k.ends_with(".markdown")
}

/// Ein relativer Pfad, der nicht aus dem Projekt herausführt.
fn sicherer_pfad(wurzel: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let voll = wurzel.join(rel);
    let echt = voll.canonicalize().ok()?;
    echt.starts_with(wurzel.canonicalize().ok()?).then_some(echt)
}

async fn per_git(dir: &Path) -> Option<Vec<String>> {
    let aus = tokio::process::Command::new("git")
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", ":(icase)*.md", ":(icase)*.markdown"])
        .current_dir(dir)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    if !aus.status.success() {
        return None;
    }
    let mut v: Vec<String> = String::from_utf8_lossy(&aus.stdout).split('\0').filter(|s| !s.is_empty()).map(String::from).collect();
    v.sort();
    v.dedup();
    Some(v)
}

fn durchsuchen(dir: &Path) -> Vec<String> {
    let mut aus = Vec::new();
    let mut warteschlange = vec![(PathBuf::new(), 0usize)];
    while let Some((rel, tiefe)) = warteschlange.pop() {
        let Ok(eintraege) = std::fs::read_dir(dir.join(&rel)) else { continue };
        for e in eintraege.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let p = rel.join(&name);
            let Ok(typ) = e.file_type() else { continue };
            if typ.is_dir() {
                if tiefe < MAX_TIEFE && !name.starts_with('.') && !UEBERSPRINGEN.contains(&name.as_str()) {
                    warteschlange.push((p, tiefe + 1));
                }
            } else if typ.is_file() && ist_buch(&name) {
                aus.push(p.to_string_lossy().replace('\\', "/"));
                if aus.len() >= MAX_BUECHER {
                    return aus;
                }
            }
        }
    }
    aus.sort();
    aus
}

/// Der Titel: die erste Überschrift, sonst der Dateiname.
fn titel(dir: &Path, rel: &str) -> String {
    use std::io::Read;
    let mut kopf = String::new();
    if let Ok(f) = std::fs::File::open(dir.join(rel)) {
        let _ = f.take(4096).read_to_string(&mut kopf);
    }
    kopf.lines().find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string())).filter(|t| !t.is_empty()).unwrap_or_else(|| rel.rsplit('/').next().unwrap_or(rel).to_string())
}

#[derive(Deserialize)]
struct Frage {
    etage: String,
    #[serde(default)]
    pfad: String,
}

#[derive(Serialize)]
struct Eintrag {
    pfad: String,
    titel: String,
}

fn ordner(o: &OfficeRef, etage: &str) -> Option<PathBuf> {
    o.etagen.get(etage).map(|e| e.ordner)
}

async fn liste(State(o): State<OfficeRef>, Query(q): Query<Frage>) -> Response {
    let Some(dir) = ordner(&o, &q.etage) else { return fehler(StatusCode::NOT_FOUND, "Diese Etage gibt es nicht") };
    let pfade = match per_git(&dir).await {
        Some(p) => p,
        None => {
            let d = dir.clone();
            tokio::task::spawn_blocking(move || durchsuchen(&d)).await.unwrap_or_default()
        }
    };
    let gekuerzt = pfade.len() > MAX_BUECHER;
    let d = dir.clone();
    let buecher = tokio::task::spawn_blocking(move || pfade.into_iter().take(MAX_BUECHER).map(|p| Eintrag { titel: titel(&d, &p), pfad: p }).collect::<Vec<_>>()).await.unwrap_or_default();
    Json(json!({ "buecher": buecher, "gekuerzt": gekuerzt })).into_response()
}

async fn buch(State(o): State<OfficeRef>, Query(q): Query<Frage>) -> Response {
    let Some(dir) = ordner(&o, &q.etage) else { return fehler(StatusCode::NOT_FOUND, "Diese Etage gibt es nicht") };
    let Some(p) = sicherer_pfad(&dir, &q.pfad).filter(|_| ist_buch(&q.pfad)) else { return fehler(StatusCode::NOT_FOUND, "Dieses Buch gibt es nicht") };
    match tokio::fs::metadata(&p).await {
        Ok(m) if m.len() > MAX_BUCH => return fehler(StatusCode::PAYLOAD_TOO_LARGE, "Dieses Buch ist zu dick zum Lesen im Office"),
        Err(_) => return fehler(StatusCode::NOT_FOUND, "Dieses Buch gibt es nicht"),
        _ => {}
    }
    match tokio::fs::read(&p).await {
        Ok(b) => ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], b).into_response(),
        Err(_) => fehler(StatusCode::NOT_FOUND, "Dieses Buch gibt es nicht"),
    }
}

async fn bild(State(o): State<OfficeRef>, Query(q): Query<Frage>) -> Response {
    let Some(dir) = ordner(&o, &q.etage) else { return fehler(StatusCode::NOT_FOUND, "Diese Etage gibt es nicht") };
    let endung = q.pfad.rsplit('.').next().unwrap_or("").to_lowercase();
    let typ = match endung.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return fehler(StatusCode::NOT_FOUND, "Das ist kein Bild"),
    };
    let Some(p) = sicherer_pfad(&dir, &q.pfad) else { return fehler(StatusCode::NOT_FOUND, "Dieses Bild gibt es nicht") };
    match tokio::fs::metadata(&p).await {
        Ok(m) if m.len() <= MAX_BILD => {}
        _ => return fehler(StatusCode::NOT_FOUND, "Dieses Bild gibt es nicht"),
    }
    match tokio::fs::read(&p).await {
        Ok(b) => ([(header::CONTENT_TYPE, typ), (header::CONTENT_SECURITY_POLICY, "default-src 'none'; style-src 'unsafe-inline'; sandbox"), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], b).into_response(),
        Err(_) => fehler(StatusCode::NOT_FOUND, "Dieses Bild gibt es nicht"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfade_bleiben_im_projekt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("LIES.md"), "# Lies mich\n").unwrap();
        assert!(sicherer_pfad(dir.path(), "LIES.md").is_some());
        assert!(sicherer_pfad(dir.path(), "../x.md").is_none());
        assert!(sicherer_pfad(dir.path(), "/etc/passwd").is_none());
        assert_eq!(durchsuchen(dir.path()), vec!["LIES.md"]);
        assert_eq!(titel(dir.path(), "LIES.md"), "Lies mich");
    }
}
