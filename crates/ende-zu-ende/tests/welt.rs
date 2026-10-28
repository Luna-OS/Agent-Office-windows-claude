//! Die 3D-Welt im echten Browser: Karte auf die Burg umstellen, der Kerker ist zu sehen, und E am
//! Treppenloch öffnet das Kerker-Fenster der Oberfläche (WeltAktion::KerkerOeffnen).

use std::time::Duration;

use ende_zu_ende::helfer::Helfer;
use ende_zu_ende::office::{client, fotos};
use ende_zu_ende::{Browser, TestOffice};
use protokoll::ClientNachricht;
use protokoll::einstellungen::Client as E;
use protokoll::spielzeug::{Client as S, DekorPlatz, Wand};

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

/// Vor der Südwand bei der Küche, mit Blick zur Wand (siehe welt::features::dekor).
const VOR_DER_SUEDWAND: &str = "/?name=Malerin&welt_ort=-13.5,0,10.6,0";

#[tokio::test(flavor = "multi_thread")]
async fn bild_an_der_wand() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let t = TestOffice::starten(None).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url(VOR_DER_SUEDWAND)).await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;
    s.warte_auf(
        "(() => { const c = document.getElementById('welt'); return c && c.width > 100 && c.height > 100; })()",
        LANG,
    )
    .await?;

    eprintln!("Schritt: Ein Bild (das Favicon des Office) an die Südwand hängen.");
    let icon = s
        .js("document.querySelector('link[rel=icon]').href")
        .await?
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(icon.starts_with("http"), "kein Favicon gefunden: {icon}");
    let mut h = Helfer::kommen(t.adresse, "Bildaufhaenger").await?;
    h.schicken(ClientNachricht::Spielzeug(S::DekorHinzufuegen {
        platz: DekorPlatz {
            url: icon,
            titel: Some("Testbild".into()),
            wand: Wand::Sued,
            u: -13.5,
            y: 2.0,
            b: 1.2,
            h: 0.9,
            rahmen: 3,
        },
    }))
    .await?;

    eprintln!("Schritt: Die Welt zeigt es, und davor stehend nennt der Hinweis das Bild.");
    s.warte_auf_text("Testbild", LANG).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    let png = s.foto(&fotos().join("welt-wandbild.png")).await?;
    assert!(
        png.len() > 15_000,
        "Das Bild an der Wand sieht leer aus ({} Bytes)",
        png.len()
    );
    Ok(())
}

/// Vor dem Thron der eigenen Testhalle (siehe unten), Blick nach Norden.
const VOR_DEM_THRON: &str = "/?name=Baumeisterin&welt_ort=0,0,-14.5,3.1416";

#[tokio::test(flavor = "multi_thread")]
async fn eigene_karte_aus_json() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let t = TestOffice::starten(None).await?;
    // Eine kürzere Halle ohne Kerker, mit eigenem Teppich, auf der Burg aufbauend.
    let karten = t.ordner.path().join("daten").join("karten");
    std::fs::create_dir_all(&karten)?;
    std::fs::write(
        karten.join("testhalle.json"),
        r##"{ "name": "Testhalle", "laenge": 44, "kerker": false,
              "thron": { "x": 0, "z": -17.4 }, "ankunft": { "x": -2.6, "z": -12, "blick": 3.1 },
              "farben": { "teppich": "#1d3557" } }"##,
    )?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url(VOR_DEM_THRON)).await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;

    eprintln!("Schritt: Karten neu lesen lassen und die eigene Karte wählen.");
    let mut h = Helfer::kommen(t.adresse, "Kartenwechslerin").await?;
    h.schicken(ClientNachricht::Einstellungen(E::KarteSetzen {
        karte: None,
    }))
    .await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    h.schicken(ClientNachricht::Einstellungen(E::KarteSetzen {
        karte: Some("testhalle".into()),
    }))
    .await?;
    h.warten(LANG, |n| match n {
        protokoll::ServerNachricht::Einstellungen(protokoll::einstellungen::Server::Karte {
            stand,
        }) if stand.wahl == "testhalle" => Some(()),
        _ => None,
    })
    .await?;

    eprintln!("Schritt: Die Welt baut die Testhalle; vor ihrem Thron steht der Hinweis.");
    // Unter Last braucht der Neubau der Halle im Software-WebGPU länger.
    s.warte_auf_text("auf den Thron setzen", Duration::from_secs(90))
        .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let png = s.foto(&fotos().join("welt-eigene-karte.png")).await?;
    assert!(
        png.len() > 15_000,
        "Die eigene Karte sieht leer aus ({} Bytes)",
        png.len()
    );
    Ok(())
}
