//! Grundlagen im echten Browser: anmelden, 3D sichtbar, Chat, Fenster mit Esc und ✕ schließen
//! (danach wieder Maus-Steuerung), Etage wechseln.

use std::time::Duration;

use ende_zu_ende::office::{client, fotos};
use ende_zu_ende::{Browser, TestOffice};
use serde_json::json;

const LANG: Duration = Duration::from_secs(30);

/// Ohne Chromium oder Client: überspringen statt scheitern.
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
async fn anmelden_3d_chat_fenster_etage() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    let t = TestOffice::starten(Some("geheim123")).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url("/")).await?;

    // Anmelden mit dem gemeinsamen Passwort.
    eprintln!("Schritt: Anmelden mit dem gemeinsamen Passwort.");
    s.warte_auf("document.querySelector('input[type=password]')", LANG).await?;
    s.klick("input[type=password]").await?;
    s.tippe("geheim123").await?;
    s.klick("text=Anmelden").await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;
    assert_eq!(t.office.personen().len(), 1, "genau ein Mensch im Office");

    // Die 3D-Welt zeichnet: Canvas mit Größe, und das Bild ist nicht einfarbig (PNG nicht winzig).
    eprintln!("Schritt: Die 3D-Welt zeichnet: Canvas mit Größe, und das Bild ist nicht einfarbig (PNG nicht winzig).");
    s.warte_auf("(() => { const c = document.getElementById('welt'); return c && c.width > 100 && c.height > 100; })()", LANG).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    let png = s.foto(&fotos().join("grundlagen-3d.png")).await?;
    assert!(png.len() > 15_000, "Das 3D-Bild sieht leer aus ({} Bytes, siehe target/ende-zu-ende/grundlagen-3d.png)", png.len());

    // Chat: T öffnet das Eingabefeld, Enter schickt.
    eprintln!("Schritt: Chat: T öffnet das Eingabefeld, Enter schickt.");
    s.taste("t").await?;
    s.warte_auf("document.activeElement && document.activeElement.tagName === 'INPUT'", LANG).await?;
    s.tippe("Hallo aus dem Test").await?;
    s.taste("Enter").await?;
    s.warte_auf_text("Hallo aus dem Test", LANG).await?;
    assert!(t.office.chat_letzte(5).iter().any(|z| z.text == "Hallo aus dem Test"), "der Server hat die Chat-Zeile");

    // Ein Fenster (Menü) öffnen und mit Esc schließen.
    eprintln!("Schritt: Ein Fenster (Menü) öffnen und mit Esc schließen.");
    // Mit gefangener Maus geht ein Klick in die Welt, darum öffnet Tab das Menü.
    s.taste("Tab").await?;
    s.warte_auf("document.querySelector('[role=dialog]')", LANG).await?;
    s.taste("Escape").await?;
    s.warte_auf("!document.querySelector('[role=dialog]')", LANG).await?;
    // Wieder öffnen und mit ✕ schließen; danach fängt die Welt die Maus sofort wieder.
    eprintln!("Schritt: Wieder öffnen und mit ✕ schließen; danach fängt die Welt die Maus sofort wieder.");
    s.taste("Tab").await?;
    s.warte_auf("document.querySelector('[role=dialog]')", LANG).await?;
    let vorher = s.js("window.__mausFangen").await?.as_u64().unwrap_or(0);
    s.klick("[role=dialog] .schliessen").await?;
    s.warte_auf("!document.querySelector('[role=dialog]')", LANG).await?;
    // Die Welt will die Maus sofort wieder (kopflos klappt das Fangen selbst nicht immer).
    eprintln!("Schritt: Die Welt will die Maus sofort wieder (kopflos klappt das Fangen selbst nicht immer).");
    s.warte_auf(&format!("window.__mausFangen > {vorher} && window.__mausZiel === 'welt'"), Duration::from_secs(5)).await?;

    // Eine zweite Etage, dann mit dem Aufzug-Fenster hinfahren.
    eprintln!("Schritt: Eine zweite Etage, dann mit dem Aufzug-Fenster hinfahren.");
    let zweite = t.ordner.path().join("zweites-projekt");
    std::fs::create_dir_all(&zweite)?;
    let def = t.office.etagen.uebernehmen("test/zweites-projekt", zweite, "Test");
    server::ws::etagen::liste_an_alle(&t.office);
    s.warte_auf_text("zweites-projekt", LANG).await.ok();
    s.taste("Tab").await?;
    s.warte_auf("document.querySelector('[role=dialog]')", LANG).await?;
    s.taste("Escape").await?;
    s.js("document.exitPointerLock()").await?;
    s.klick("[title='Etagen: zu einem anderen Projekt fahren']").await?;
    s.warte_auf("document.querySelector('[role=dialog]')", LANG).await?;
    s.klick(&format!("text={}", def.name)).await.or(s.js(&format!("[...document.querySelectorAll('[role=dialog] button')].find(b => b.innerText.includes({})).click()", json!(def.name))).await.map(|_| ()))?;
    let start = std::time::Instant::now();
    while t.office.personen().first().and_then(|p| p.etage.clone()).as_deref() != Some(def.id.as_str()) {
        anyhow::ensure!(start.elapsed() < LANG, "Die Fahrt zur zweiten Etage kam nicht an");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    s.foto(&fotos().join("grundlagen-etage2.png")).await?;
    let fehler: Vec<String> = s.konsole.lock().unwrap().iter().filter(|z| z.starts_with("[Ausnahme]") || z.starts_with("[error]")).cloned().collect();
    assert!(fehler.is_empty(), "Fehler in der Konsole:\n{}", fehler.join("\n"));
    Ok(())
}
