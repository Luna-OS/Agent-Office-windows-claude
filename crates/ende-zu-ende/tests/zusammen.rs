//! Zusammenarbeit im echten Browser: ein Arbeiter (eigener Befehl als Fake-Provider) erscheint,
//! eine Agenten-Nachricht kommt bei anderen an, ein Video aufs Dach, Bildschirm teilen.

use std::time::Duration;

use ende_zu_ende::helfer::Helfer;
use ende_zu_ende::office::{client, fotos};
use ende_zu_ende::{Browser, Seite, TestOffice};
use protokoll::{ClientNachricht, ServerNachricht, agenten, arbeiter, dach, etagen, medien};

const LANG: Duration = Duration::from_secs(30);

fn bereit() -> bool {
    let ok = ende_zu_ende::browser::chromium_finden().is_some() && client().is_some();
    if !ok {
        eprintln!("übersprungen: kein Chromium oder kein gebauter Client in dist/");
    }
    ok
}

/// Öffnet einen Eintrag aus dem Menü (Tab).
async fn menue(s: &Seite, eintrag: &str) -> anyhow::Result<()> {
    s.taste("Tab").await?;
    s.warte_auf("document.querySelector('[role=dialog]')", LANG)
        .await?;
    s.klick(&format!("text={eintrag}")).await?;
    s.warte_auf(&format!("[...document.querySelectorAll('[role=dialog]')].some(d => d.getAttribute('aria-label') && d.getAttribute('aria-label').includes({}))", serde_json::json!(eintrag)), LANG).await
}

#[tokio::test(flavor = "multi_thread")]
async fn arbeiter_agenten_dach_teilen() -> anyhow::Result<()> {
    if !bereit() {
        return Ok(());
    }
    // Der Fake-Provider: ein eigener Befehl, dem der Auftrag ins Terminal getippt wird.
    // SAFETY: nur dieser Test setzt die Variable, bevor das Office startet.
    unsafe {
        std::env::set_var(
            "OFFICE_AGENT_EIGEN",
            if cfg!(windows) { "cmd.exe" } else { "sh" },
        )
    };
    let t = TestOffice::starten(None).await?;
    let b = Browser::starten().await?;
    let s = b.seite(&t.url("/?name=Testerin")).await?;
    s.warte_auf("document.getElementById('hud')", LANG).await?;
    let mut h = Helfer::kommen(t.adresse, "Helfer").await?;

    eprintln!("Schritt: Arbeiter anlegen (eigener Befehl)");
    let tisch = welt_tisch();
    h.schicken(ClientNachricht::Arbeiter(arbeiter::Client::Anlegen {
        tisch,
        prompt: Some("echo hallo".into()),
        worktree: Some(false),
        art: Default::default(),
        provider: Some("eigen".into()),
        modell: None,
        aufwand: None,
        issue: None,
    }))
    .await?;
    s.warte_auf(
        "document.getElementById('hud').innerText.match(/Arbeiter\\s*1/)",
        LANG,
    )
    .await?;
    s.foto(&fotos().join("zusammen-arbeiter.png")).await?;

    eprintln!("Schritt: Agenten-Nachricht aus dem Browser kommt beim Helfer an");
    menue(&s, "Agenten").await?;
    s.klick("[role=dialog] input").await?;
    s.tippe("Hallo Agenten").await?;
    s.taste("Enter").await?;
    h.warten(LANG, |n| matches!(n, ServerNachricht::Agenten(agenten::Server::Nachricht { .. }) if serde_json::to_string(n).unwrap_or_default().contains("Hallo Agenten")).then_some(())).await?;
    s.taste("Escape").await?;

    eprintln!(
        "Schritt: mit dem Aufzug aufs Dach, dann ein Video auf die Leinwand (der Helfer steht auch oben)"
    );
    h.schicken(ClientNachricht::Etagen(etagen::Client::Gehen {
        etage: etagen::DACH.into(),
        an: None,
    }))
    .await?;
    menue(&s, "Etagen").await?;
    s.klick("knopf=Dachterrasse").await?;
    let start = std::time::Instant::now();
    let browser_etage = || {
        t.office
            .personen()
            .into_iter()
            .find(|p| p.id != h.id)
            .and_then(|p| p.etage)
    };
    while browser_etage().as_deref() != Some(etagen::DACH) {
        anyhow::ensure!(start.elapsed() < LANG, "Die Fahrt aufs Dach kam nicht an");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    menue(&s, "Dach-Leinwand").await?;
    s.klick("[role=dialog] input").await?;
    s.tippe("https://www.youtube.com/watch?v=M7lc1UVf-VE")
        .await?;
    s.taste("Enter").await?;
    let id = h
        .warten(LANG, |n| match n {
            ServerNachricht::Dach(dach::Server::Leinwand { zustand }) => zustand.video_id.clone(),
            _ => None,
        })
        .await?;
    assert_eq!(id, "M7lc1UVf-VE");
    s.taste("Escape").await?;

    eprintln!("Schritt: Bildschirm teilen (Fake-Bildschirm), der Helfer auf der Etage sieht es");
    menue(&s, "Etagen").await?;
    s.klick("text=mein-projekt").await?;
    h.schicken(ClientNachricht::Etagen(etagen::Client::Gehen {
        etage: "mein-projekt".into(),
        an: None,
    }))
    .await?;
    menue(&s, "Bildschirm teilen").await?;
    s.klick("knopf=Bildschirm teilen").await?;
    h.warten(Duration::from_secs(60), |n| match n {
        ServerNachricht::Medien(medien::Server::Bildschirme { liste }) if !liste.is_empty() => {
            Some(())
        }
        _ => None,
    })
    .await?;
    s.foto(&fotos().join("zusammen-teilen.png")).await?;

    let fehler: Vec<String> = s
        .konsole
        .lock()
        .unwrap()
        .iter()
        .filter(|z| z.starts_with("[Ausnahme]") || z.starts_with("[error]"))
        .cloned()
        .collect();
    assert!(
        fehler.is_empty(),
        "Fehler in der Konsole:\n{}",
        fehler.join("\n")
    );
    Ok(())
}

/// Der erste Schreibtisch der Etage.
fn welt_tisch() -> String {
    server::etagen::arbeitsplaetze(0)
        .into_iter()
        .next()
        .unwrap_or_else(|| "desk-1".into())
}
