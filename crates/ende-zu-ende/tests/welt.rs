//! Die 3D-Welt im echten Browser: Karte auf die Burg umstellen, der Kerker ist zu sehen, und E am
//! Treppenloch öffnet das Kerker-Fenster der Oberfläche (WeltAktion::KerkerOeffnen).

use std::time::Duration;

use ende_zu_ende::helfer::Helfer;
use ende_zu_ende::office::{client, fotos};
use ende_zu_ende::{Browser, TestOffice};
use protokoll::ClientNachricht;
use protokoll::einstellungen::Client as E;

const LANG: Duration = Duration::from_secs(30);

/// Am Treppenloch zum Kerker, mit Blick nach Süden hinunter (siehe welt::grundriss::burg).
const AM_KERKER: &str = "/?name=Burgfrau&welt_ort=8.3,0,12.2,0";

fn bereit() -> bool {
    if ende_zu_ende::browser::chromium_finden().is_none() {
        eprintln!("übersprungen: kein Chromium (OFFICE_CHROMIUM setzen)");
        return false;
    }
    if client().is_none() {
        eprintln!("übersprungen: kein gebauter Client in dist/ (trunk build in crates/client)");
        return false;
    }
    true
}

#[tokio::test(flavor = "multi_thread")]
async fn burg_kerker_sichtbar_und_fenster() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let t = TestOffice::starten(None).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url(AM_KERKER)).await?;

    eprintln!("Schritt: Office offen, die Welt zeichnet.");
    s.warte_auf("document.getElementById('hud')", LANG).await?;
    s.warte_auf(
        "(() => { const c = document.getElementById('welt'); return c && c.width > 100 && c.height > 100; })()",
        LANG,
    )
    .await?;

    eprintln!("Schritt: Karte auf die Burg umstellen (über eine zweite Verbindung).");
    let mut h = Helfer::kommen(t.adresse, "Kartenwechsler").await?;
    h.schicken(ClientNachricht::Einstellungen(E::KarteSetzen {
        karte: Some("burg".into()),
    }))
    .await?;
    h.warten(LANG, |n| match n {
        protokoll::ServerNachricht::Einstellungen(protokoll::einstellungen::Server::Karte {
            stand,
        }) if stand.wahl == "burg" => Some(()),
        _ => None,
    })
    .await?;

    eprintln!(
        "Schritt: Der Kerker ist zu sehen (Hinweis der Welt am Treppenloch, Bild nicht leer)."
    );
    s.warte_auf_text("in den Kerker schauen", LANG).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let png = s.foto(&fotos().join("welt-burg-kerker.png")).await?;
    assert!(
        png.len() > 15_000,
        "Das Bild am Kerker sieht leer aus ({} Bytes, siehe target/ende-zu-ende/welt-burg-kerker.png)",
        png.len()
    );

    eprintln!("Schritt: E am Treppenloch öffnet das Kerker-Fenster.");
    s.taste("e").await?;
    s.warte_auf(
        "[...document.querySelectorAll('[role=dialog]')].some(d => d.innerText.includes('Kerker'))",
        LANG,
    )
    .await?;
    s.foto(&fotos().join("welt-burg-kerker-fenster.png"))
        .await?;
    Ok(())
}
