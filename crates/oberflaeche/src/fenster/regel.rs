//! Prüft die Regel für alle Fenster, auch die, die nur aus der Welt aufgehen: Jedes Fenster hat oben
//! rechts ein ✕, und ✕ oder Esc schließt es. Das gilt, weil jedes Fenster seinen Inhalt in den
//! gemeinsamen [`Rahmen`](crate::modal::Rahmen) legt; der Esc-Griff hängt am Fensterstapel, nicht am
//! einzelnen Fenster.

/// Alle Quellen, in denen Fenster gebaut werden.
const QUELLEN: &[&str] = &[
    include_str!("aenderungen.rs"),
    include_str!("agenten.rs"),
    include_str!("anlegen.rs"),
    include_str!("arcade/mod.rs"),
    include_str!("aufgaben.rs"),
    include_str!("aufzug.rs"),
    include_str!("bar.rs"),
    include_str!("buecher.rs"),
    include_str!("einstellungen.rs"),
    include_str!("github.rs"),
    include_str!("hilfe.rs"),
    include_str!("kerker.rs"),
    include_str!("konten.rs"),
    include_str!("medien.rs"),
    include_str!("meeting.rs"),
    include_str!("menue.rs"),
    include_str!("minen/mod.rs"),
    include_str!("nutzung.rs"),
    include_str!("palette.rs"),
    include_str!("station.rs"),
    include_str!("team.rs"),
    include_str!("tisch.rs"),
    include_str!("whiteboard/mod.rs"),
    include_str!("../terminal/mod.rs"),
];

/// Die Varianten, die `ansicht` (fenster/mod.rs) öffnet.
fn varianten() -> Vec<String> {
    let quelle = include_str!("mod.rs");
    let start = quelle.find("pub fn ansicht").expect("ansicht");
    quelle[start..]
        .lines()
        .filter_map(|z| z.trim().strip_prefix("Fenster::"))
        .map(|rest| {
            rest.split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or("")
                .to_string()
        })
        .filter(|v| !v.is_empty())
        .collect()
}

#[test]
fn jedes_fenster_liegt_im_rahmen() {
    let v = varianten();
    assert!(v.len() >= 30, "zu wenige Fenster gefunden: {v:?}");
    let alle = QUELLEN.concat();
    let fehlen: Vec<_> = v
        .iter()
        .filter(|name| !alle.contains(&format!("fenster=Fenster::{name}")))
        .collect();
    assert!(
        fehlen.is_empty(),
        "Diese Fenster legen ihren Inhalt nicht in den Rahmen: {fehlen:?}"
    );
}

#[test]
fn der_rahmen_hat_das_kreuz_und_esc_schliesst() {
    let modal = include_str!("../modal.rs");
    // Das ✕ sitzt im Kopf, nach dem Titel (rechts), und schließt genau dieses Fenster.
    let rahmen = &modal[modal.find("pub fn Rahmen").expect("Rahmen")..];
    let titel = rahmen.find("<h2").expect("Titel");
    let kreuz = rahmen.find("\"✕\"").expect("✕");
    assert!(kreuz > titel, "das ✕ steht rechts vom Titel");
    assert!(rahmen.contains("on:click=move |_| schliessen(&f_zu)"));
    // Esc wird im Capture-Schritt abgefangen und schließt das oberste Fenster.
    assert!(modal.contains("e.key() == \"Escape\" && offen()"));
    assert!(
        modal.contains("add_event_listener_with_callback_and_bool(")
            && modal.contains("\"keydown\"")
    );
    // Ist danach keins mehr offen, fängt die Welt die Maus sofort wieder.
    assert!(modal.contains("crate::bruecke::fenster(false)"));
}
