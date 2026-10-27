//! Ein Office für einen Test: Wegwerf-Datenordner, ein Projektordner als erste Etage, freier Port.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use server::konfig::Konfig;
use server::office::OfficeRef;

pub struct TestOffice {
    pub adresse: SocketAddr,
    pub office: OfficeRef,
    pub ordner: tempfile::TempDir,
}

/// Der Wurzelordner des Workspace.
pub fn wurzel() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Der gebaute Client, wenn es ihn gibt (`OFFICE_CLIENT` oder `<workspace>/dist`).
pub fn client() -> Option<PathBuf> {
    let p = std::env::var_os("OFFICE_CLIENT").map(PathBuf::from).unwrap_or_else(|| wurzel().join("dist"));
    p.join("index.html").is_file().then_some(p)
}

/// Wohin Bildschirmfotos gehen.
pub fn fotos() -> PathBuf {
    wurzel().join("target").join("ende-zu-ende")
}

impl TestOffice {
    /// Startet ein Office; `passwort` = gemeinsames Passwort (ohne: offen).
    pub async fn starten(passwort: Option<&str>) -> Result<Self> {
        let ordner = tempfile::Builder::new().prefix("atlas-e2e-").tempdir()?;
        let projekt = ordner.path().join("mein-projekt");
        std::fs::create_dir_all(&projekt)?;
        std::fs::write(projekt.join("LIES.md"), "# Lies mich\n\nEin Testprojekt.\n")?;
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let adresse = l.local_addr()?;
        let mut k = Konfig::fuer_test(ordner.path().join("daten"));
        k.port = adresse.port();
        k.projekt_ordner = Some(projekt);
        k.passwort = passwort.map(String::from);
        k.client = client().unwrap_or_else(|| wurzel().join("dist"));
        let g = server::bauen(k).await?;
        let office = g.office.clone();
        server::hintergrund(&office);
        tokio::spawn(async move {
            let _ = axum::serve(l, g.router.into_make_service_with_connect_info::<SocketAddr>()).await;
        });
        Ok(Self { adresse, office, ordner })
    }

    pub fn url(&self, pfad: &str) -> String {
        format!("http://{}{pfad}", self.adresse)
    }
}
