use super::*;
use protokoll::medien::RtcSignal;

fn medien() -> (Medien, PathBuf) {
    let ordner =
        std::env::temp_dir().join(format!("medien-test-{}-{}", std::process::id(), jetzt_ms()));
    (
        Medien::neu(MedienEinstellungen {
            datenordner: ordner.clone(),
            ice: vec![],
        }),
        ordner,
    )
}

fn wer(id: &str, dach: bool) -> Absender {
    Absender {
        verbindung: id.into(),
        name: format!("Name {id}"),
        etage: Some("e1".into()),
        konto: None,
        auf_dach: dach,
    }
}

#[test]
fn rtc_geht_nur_an_den_empfaenger() {
    let (m, _) = medien();
    let mut rx = m.ereignisse();
    m.nachricht_medien(
        &wer("a", false),
        MedienClient::Rtc {
            an: "b".into(),
            daten: RtcSignal::default(),
        },
    )
    .unwrap();
    let e = rx.try_recv().unwrap();
    assert_eq!(e.ziel, Ziel::Verbindung("b".into()));
    assert!(
        matches!(e.nachricht, ServerNachricht::Medien(MedienServer::Rtc { von, .. }) if von == "a")
    );
}

#[test]
fn leinwand_nur_auf_dem_dach_und_gespeichert() {
    let (m, ordner) = medien();
    let mut rx = m.ereignisse();
    let setzen = DachClient::VideoSetzen {
        url: "https://youtu.be/dQw4w9WgXcQ?t=5".into(),
    };
    assert!(m.nachricht_dach(&wer("a", false), setzen.clone()).is_err());
    m.nachricht_dach(&wer("a", true), setzen).unwrap();
    let e = rx.try_recv().unwrap();
    assert_eq!(e.ziel, Ziel::Dach);
    // Neustart: die Leinwand ist noch da.
    let m2 = Medien::neu(MedienEinstellungen {
        datenordner: ordner,
        ice: vec![],
    });
    let begruessung = m2.etage_betreten(&wer("b", true));
    assert!(begruessung.iter().any(|n| matches!(n,
        ServerNachricht::Dach(DachServer::Leinwand { zustand }) if zustand.video_id.as_deref() == Some("dQw4w9WgXcQ"))));
}

#[test]
fn jukebox_meldet_der_etage() {
    let (m, _) = medien();
    let mut rx = m.ereignisse();
    m.nachricht_medien(
        &wer("a", false),
        MedienClient::JukeboxSpielen {
            stueck: Some("late-commit".into()),
            url: None,
        },
    )
    .unwrap();
    let e = rx.try_recv().unwrap();
    assert_eq!(e.ziel, Ziel::Etage("e1".into()));
    assert_eq!(m.jukebox_titel("e1"), "Später Commit");
    let mut lobby = wer("a", false);
    lobby.etage = None;
    assert!(
        m.nachricht_medien(&lobby, MedienClient::JukeboxWeiter)
            .is_err()
    );
}

#[test]
fn turn_aus_text() {
    let l = ice_server("turn:nutzer:geheim@turn.example.com:3478, turns:turn2.example.com:5349");
    assert_eq!(l.len(), 3);
    assert_eq!(l[1].urls, vec!["turn:turn.example.com:3478".to_string()]);
    assert_eq!(l[1].username.as_deref(), Some("nutzer"));
    assert_eq!(l[1].credential.as_deref(), Some("geheim"));
    assert_eq!(l[2].urls, vec!["turns:turn2.example.com:5349".to_string()]);
    assert_eq!(l[2].username, None);
}

#[test]
fn pfad_sicher() {
    assert_eq!(pfad_teil("ab-c_1"), "ab-c_1");
    assert_eq!(pfad_teil("a/b c"), "a%2Fb%20c");
}
