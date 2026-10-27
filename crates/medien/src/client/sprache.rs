//! Voice: WebRTC-Mesh zwischen allen auf der Etage, Signale über den WebSocket (`medien::Client::Rtc`).
//!
//! „Perfect negotiation“ wie im Original: jede Seite darf jederzeit Spuren hinzufügen; bei einer
//! Kollision gibt die höfliche Seite (kleinere ID) nach. Lautstärke nach Nähe kommt aus der Welt
//! ([`Sprache::abstaende`]), der Pegel für die Mundbewegung aus einem Analyser je Verbindung.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use protokoll::ClientNachricht;
use protokoll::medien::{Client, IceKandidat, IceServer, RtcSignal, SdpBeschreibung};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AnalyserNode, AudioContext, HtmlAudioElement, MediaStream, MediaStreamConstraints,
    MediaStreamTrack, RtcConfiguration, RtcIceCandidateInit, RtcIceConnectionState, RtcIceServer,
    RtcPeerConnection, RtcPeerConnectionIceEvent, RtcRtpSender, RtcSdpType,
    RtcSessionDescriptionInit, RtcSignalingState, RtcTrackEvent,
};

use super::{Lauscher, Senden, fehlertext, fenster, setzen, sicher};
use crate::naehe::sprach_lautstaerke;

type Rueckruf = Closure<dyn FnMut(JsValue)>;

struct Verbindung {
    pc: RtcPeerConnection,
    hoeflich: bool,
    mache_angebot: Cell<bool>,
    ignoriere_angebot: Cell<bool>,
    audio: HtmlAudioElement,
    mic_sender: RefCell<Option<RtcRtpSender>>,
    analyser: RefCell<Option<AnalyserNode>>,
    rueckrufe: RefCell<Vec<Rueckruf>>,
    _klick: RefCell<Option<Lauscher>>,
}

#[derive(Default)]
struct Innen {
    ich: String,
    ice: Vec<IceServer>,
    mic: Option<MediaStream>,
    lokal: Option<AnalyserNode>,
    ctx: Option<AudioContext>,
    verbindungen: HashMap<String, Rc<Verbindung>>,
    stumm: bool,
    /// Push-to-talk gedrückt: Loslassen schaltet wieder stumm.
    spricht: bool,
    tritt_bei: bool,
    beobachter: Vec<Rc<dyn Fn()>>,
}

/// Voice im Browser. Billig zu klonen.
#[derive(Clone)]
pub struct Sprache {
    innen: Rc<RefCell<Innen>>,
    senden: Senden,
}

impl Sprache {
    /// `ich` ist die eigene Personen-ID (aus der Begrüßung des Servers).
    pub fn neu(senden: Senden, ich: &str) -> Self {
        let innen = Innen {
            ich: ich.into(),
            ..Default::default()
        };
        Self {
            innen: Rc::new(RefCell::new(innen)),
            senden,
        }
    }

    pub fn ich_setzen(&self, ich: &str) {
        self.innen.borrow_mut().ich = ich.into();
    }

    /// Aus `medien::Server::IceServer`.
    pub fn ice_setzen(&self, server: Vec<IceServer>) {
        self.innen.borrow_mut().ice = server;
    }

    pub fn aktiv(&self) -> bool {
        self.innen.borrow().mic.is_some()
    }

    pub fn stumm(&self) -> bool {
        self.innen.borrow().stumm
    }

    /// Wird nach jeder Änderung gerufen (beigetreten, stumm, Verbindungen).
    pub fn bei_aenderung(&self, f: impl Fn() + 'static) {
        self.innen.borrow_mut().beobachter.push(Rc::new(f));
    }

    fn geaendert(&self, melden: bool) {
        let (beobachter, aktiv, stumm) = {
            let i = self.innen.borrow();
            (i.beobachter.clone(), i.mic.is_some(), i.stumm)
        };
        for f in beobachter {
            f();
        }
        if melden {
            (self.senden)(ClientNachricht::Medien(Client::Sprache { aktiv, stumm }));
        }
    }

    /// Mit wem verbunden sein soll: alle anderen auf der Etage (ohne die 2D-Ansicht). Neue werden
    /// verbunden, Fehlende getrennt.
    pub fn personen_abgleichen(&self, personen: &[String]) {
        let ich = self.innen.borrow().ich.clone();
        for id in personen.iter().filter(|p| **p != ich) {
            if !self.innen.borrow().verbindungen.contains_key(id) {
                self.verbinden(id);
            }
        }
        let weg: Vec<String> = self
            .innen
            .borrow()
            .verbindungen
            .keys()
            .filter(|k| !personen.contains(k))
            .cloned()
            .collect();
        for id in weg {
            self.trennen(&id);
        }
    }

    /// Alle Verbindungen schließen (Etagenwechsel, Abmelden).
    pub fn zuruecksetzen(&self) {
        let alle: Vec<String> = self.innen.borrow().verbindungen.keys().cloned().collect();
        for id in alle {
            self.trennen(&id);
        }
    }

    /// Voice beitreten (Mikrofon anfragen). `stumm` für Push-to-talk.
    pub async fn beitreten(&self, stumm: bool) -> Result<(), String> {
        if self.aktiv() || self.innen.borrow().tritt_bei {
            return Ok(());
        }
        if !sicher() {
            return Err("Voice braucht HTTPS (oder localhost). Bitte den Betreiber des Office, TLS einzuschalten.".into());
        }
        let geraete = fenster()
            .navigator()
            .media_devices()
            .map_err(|_| "Kein Mikrofon verfügbar")?;
        let audio = js_sys::Object::new();
        setzen(&audio, "echoCancellation", true);
        setzen(&audio, "noiseSuppression", true);
        setzen(&audio, "autoGainControl", true);
        let wunsch = MediaStreamConstraints::new();
        wunsch.set_audio(&audio);
        let versprechen = geraete
            .get_user_media_with_constraints(&wunsch)
            .map_err(|e| fehlertext(&e))?;
        self.innen.borrow_mut().tritt_bei = true;
        let ergebnis = JsFuture::from(versprechen).await;
        self.innen.borrow_mut().tritt_bei = false;
        let mic: MediaStream = ergebnis
            .map_err(|e| format!("Mikrofon nicht verfügbar: {}", fehlertext(&e)))?
            .unchecked_into();
        let spur = erste_spur(&mic, true);
        if let Some(s) = &spur {
            s.set_enabled(!stumm);
        }
        let lokal = self.ctx().and_then(|c| analyser_fuer(&c, &mic));
        let verbindungen: Vec<Rc<Verbindung>> = {
            let mut i = self.innen.borrow_mut();
            i.mic = Some(mic.clone());
            i.lokal = lokal;
            i.stumm = stumm;
            i.spricht = false;
            i.verbindungen.values().cloned().collect()
        };
        if let Some(s) = spur {
            for v in verbindungen {
                let sender = v.pc.add_track_0(&s, &mic);
                if stumm {
                    let _ = sender.replace_track(None);
                }
                *v.mic_sender.borrow_mut() = Some(sender);
            }
        }
        self.geaendert(true);
        Ok(())
    }

    pub fn verlassen(&self) {
        let (mic, verbindungen) = {
            let mut i = self.innen.borrow_mut();
            i.lokal = None;
            i.spricht = false;
            (
                i.mic.take(),
                i.verbindungen.values().cloned().collect::<Vec<_>>(),
            )
        };
        let Some(mic) = mic else { return };
        for v in verbindungen {
            if let Some(s) = v.mic_sender.borrow_mut().take() {
                v.pc.remove_track(&s);
            }
        }
        for s in mic.get_tracks().iter() {
            if let Ok(s) = s.dyn_into::<MediaStreamTrack>() {
                s.stop();
            }
        }
        self.geaendert(true);
    }

    pub fn stumm_setzen(&self, stumm: bool) {
        let mic = {
            let mut i = self.innen.borrow_mut();
            i.spricht = false;
            if i.mic.is_none() || i.stumm == stumm {
                return;
            }
            i.stumm = stumm;
            i.mic.clone()
        };
        if let Some(s) = mic.as_ref().and_then(|m| erste_spur(m, true)) {
            s.set_enabled(!stumm);
            self.senden_lassen(if stumm { None } else { Some(&s) });
        }
        self.geaendert(true);
    }

    /// Stumm heißt: gar nichts senden. Ein abgeschalteter Track allein lässt den Codec noch
    /// Komfortrauschen schicken, das bei gleichmäßigen Tönen hörbar ist; darum Track ab vom Sender.
    fn senden_lassen(&self, spur: Option<&MediaStreamTrack>) {
        let verbindungen: Vec<Rc<Verbindung>> =
            self.innen.borrow().verbindungen.values().cloned().collect();
        for v in verbindungen {
            if let Some(sender) = v.mic_sender.borrow().as_ref() {
                let _ = sender.replace_track(spur);
            }
        }
    }

    pub fn stumm_umschalten(&self) {
        self.stumm_setzen(!self.stumm());
    }

    /// Push-to-talk: V gedrückt.
    pub fn sprechen_start(&self) {
        if !self.aktiv() || self.innen.borrow().spricht {
            return;
        }
        self.stumm_setzen(false);
        self.innen.borrow_mut().spricht = true;
    }

    /// Push-to-talk: V losgelassen (oder Fenster verliert den Fokus).
    pub fn sprechen_stopp(&self) {
        if self.innen.borrow().spricht {
            self.stumm_setzen(true);
        }
    }

    /// Lautstärke nach Nähe, pro Frame aus der Welt: (Personen-ID, Abstand in m).
    pub fn abstaende(&self, abstaende: &[(String, f32)]) {
        let i = self.innen.borrow();
        for (id, abstand) in abstaende {
            if let Some(v) = i.verbindungen.get(id) {
                v.audio.set_volume(sprach_lautstaerke(*abstand) as f64);
            }
        }
    }

    /// Sprechpegel (0..~0.5) einer Person, für die Mundbewegung; die eigene ID gibt den eigenen.
    pub fn pegel(&self, id: &str) -> f32 {
        let i = self.innen.borrow();
        if id == i.ich {
            return if i.stumm {
                0.0
            } else {
                i.lokal.as_ref().map_or(0.0, rms)
            };
        }
        i.verbindungen
            .get(id)
            .and_then(|v| v.analyser.borrow().as_ref().map(rms))
            .unwrap_or(0.0)
    }

    /// Für Tests und Entwicklerwerkzeuge: je Verbindung (ID, ICE-Zustand, kommt Ton an, Pegel).
    pub fn diagnose(&self) -> Vec<(String, String, bool, f32)> {
        let i = self.innen.borrow();
        let mut v: Vec<_> = i
            .verbindungen
            .iter()
            .map(|(id, v)| {
                let ice = format!("{:?}", v.pc.ice_connection_state()).to_lowercase();
                let ton = v.audio.src_object().is_some();
                let pegel = v.analyser.borrow().as_ref().map_or(0.0, rms);
                (id.clone(), ice, ton, pegel)
            })
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Ein Signal von `von` (aus `medien::Server::Rtc`).
    pub fn signal(&self, von: String, daten: RtcSignal) {
        let ich = self.clone();
        spawn_local(async move {
            if let Err(e) = ich.signal_behandeln(&von, daten).await {
                web_sys::console::warn_1(&format!("Voice-Signal fehlgeschlagen: {e}").into());
            }
        });
    }

    async fn signal_behandeln(&self, von: &str, daten: RtcSignal) -> Result<(), String> {
        let v = match self.innen.borrow().verbindungen.get(von).cloned() {
            Some(v) => v,
            None => self.verbinden(von),
        };
        if let Some(b) = daten.beschreibung {
            let angebot = b.typ == "offer";
            let kollision = angebot
                && (v.mache_angebot.get() || v.pc.signaling_state() != RtcSignalingState::Stable);
            v.ignoriere_angebot.set(!v.hoeflich && kollision);
            if v.ignoriere_angebot.get() {
                return Ok(());
            }
            let init = RtcSessionDescriptionInit::new(sdp_typ(&b.typ));
            init.set_sdp(&b.sdp);
            JsFuture::from(v.pc.set_remote_description(&init))
                .await
                .map_err(|e| fehlertext(&e))?;
            if angebot {
                lokale_beschreibung_setzen(&v.pc).await?;
                self.beschreibung_senden(von, &v.pc);
            }
        } else if let Some(k) = daten.kandidat {
            let init = RtcIceCandidateInit::new(&k.candidate);
            init.set_sdp_mid(k.sdp_mid.as_deref());
            init.set_sdp_m_line_index(k.sdp_m_line_index);
            let r =
                JsFuture::from(v.pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init)))
                    .await;
            if let Err(e) = r
                && !v.ignoriere_angebot.get()
            {
                return Err(fehlertext(&e));
            }
        } else if daten.ende {
            let _ =
                JsFuture::from(v.pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(None)).await;
        }
        Ok(())
    }

    fn rtc_senden(&self, an: &str, daten: RtcSignal) {
        (self.senden)(ClientNachricht::Medien(Client::Rtc {
            an: an.into(),
            daten,
        }));
    }

    fn beschreibung_senden(&self, an: &str, pc: &RtcPeerConnection) {
        if let Some(d) = pc.local_description() {
            let typ = match d.type_() {
                RtcSdpType::Offer => "offer",
                RtcSdpType::Answer => "answer",
                RtcSdpType::Pranswer => "pranswer",
                _ => "rollback",
            };
            let beschreibung = Some(SdpBeschreibung {
                typ: typ.into(),
                sdp: d.sdp(),
            });
            self.rtc_senden(
                an,
                RtcSignal {
                    beschreibung,
                    ..Default::default()
                },
            );
        }
    }

    fn ctx(&self) -> Option<AudioContext> {
        let mut i = self.innen.borrow_mut();
        if i.ctx.is_none() {
            i.ctx = AudioContext::new().ok();
        }
        if let Some(c) = &i.ctx {
            let _ = c.resume();
        }
        i.ctx.clone()
    }

    fn verbinden(&self, id: &str) -> Rc<Verbindung> {
        let (ich, ice, mic) = {
            let i = self.innen.borrow();
            (i.ich.clone(), i.ice.clone(), i.mic.clone())
        };
        let konfig = RtcConfiguration::new();
        let liste = js_sys::Array::new();
        for s in &ice {
            let server = RtcIceServer::new();
            let urls = js_sys::Array::new();
            for u in &s.urls {
                urls.push(&u.into());
            }
            server.set_urls(&urls);
            if let Some(n) = &s.username {
                server.set_username(n);
            }
            if let Some(c) = &s.credential {
                server.set_credential(c);
            }
            liste.push(&server);
        }
        konfig.set_ice_servers(&liste);
        let pc = RtcPeerConnection::new_with_configuration(&konfig).expect("RTCPeerConnection");
        let audio = HtmlAudioElement::new().expect("Audio");
        audio.set_autoplay(true);
        let v = Rc::new(Verbindung {
            pc: pc.clone(),
            hoeflich: ich.as_str() < id,
            mache_angebot: Cell::new(false),
            ignoriere_angebot: Cell::new(false),
            audio,
            mic_sender: RefCell::new(None),
            analyser: RefCell::new(None),
            rueckrufe: RefCell::new(Vec::new()),
            _klick: RefCell::new(None),
        });
        self.innen
            .borrow_mut()
            .verbindungen
            .insert(id.into(), v.clone());

        let (ich2, id2, v2) = (self.clone(), id.to_string(), Rc::downgrade(&v));
        self.rueckruf(&v, "onnegotiationneeded", move |_| {
            let (ich, id, v) = (ich2.clone(), id2.clone(), v2.clone());
            spawn_local(async move {
                let Some(v) = v.upgrade() else { return };
                v.mache_angebot.set(true);
                match lokale_beschreibung_setzen(&v.pc).await {
                    Ok(()) => ich.beschreibung_senden(&id, &v.pc),
                    Err(e) => web_sys::console::warn_1(
                        &format!("Voice-Verhandlung fehlgeschlagen: {e}").into(),
                    ),
                }
                v.mache_angebot.set(false);
            });
        });
        let (ich2, id2) = (self.clone(), id.to_string());
        self.rueckruf(&v, "onicecandidate", move |e| {
            let e: RtcPeerConnectionIceEvent = e.unchecked_into();
            let daten = match e.candidate() {
                Some(k) => RtcSignal {
                    kandidat: Some(IceKandidat {
                        candidate: k.candidate(),
                        sdp_mid: k.sdp_mid(),
                        sdp_m_line_index: k.sdp_m_line_index(),
                    }),
                    ..Default::default()
                },
                None => RtcSignal {
                    ende: true,
                    ..Default::default()
                },
            };
            ich2.rtc_senden(&id2, daten);
        });
        let pc2 = pc.clone();
        self.rueckruf(&v, "oniceconnectionstatechange", move |_| {
            if pc2.ice_connection_state() == RtcIceConnectionState::Failed {
                let _ = js_sys::Reflect::get(&pc2, &"restartIce".into())
                    .ok()
                    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
                    .map(|f| f.call0(&pc2));
            }
        });
        let (ich2, v2) = (self.clone(), Rc::downgrade(&v));
        self.rueckruf(&v, "ontrack", move |e| {
            let e: RtcTrackEvent = e.unchecked_into();
            let Some(v) = v2.upgrade() else { return };
            if e.track().kind() != "audio" {
                return;
            }
            let strom: MediaStream = match e.streams().get(0).dyn_into() {
                Ok(s) => s,
                Err(_) => {
                    let s = MediaStream::new().expect("MediaStream");
                    s.add_track(&e.track());
                    s
                }
            };
            v.audio.set_src_object(Some(&strom));
            if v.audio.play().is_err() {
                // Autoplay gesperrt, bis der Mensch klickt.
                let a = v.audio.clone();
                *v._klick.borrow_mut() =
                    Some(Lauscher::neu(&fenster(), "pointerdown", move |_| {
                        let _ = a.play();
                    }));
            }
            if let Some(c) = ich2.ctx() {
                *v.analyser.borrow_mut() = analyser_fuer(&c, &strom);
            }
            ich2.geaendert(false);
        });
        if let Some(mic) = mic
            && let Some(s) = erste_spur(&mic, true)
        {
            let sender = pc.add_track_0(&s, &mic);
            if self.stumm() {
                let _ = sender.replace_track(None);
            }
            *v.mic_sender.borrow_mut() = Some(sender);
        }
        v
    }

    fn rueckruf(&self, v: &Verbindung, ereignis: &str, f: impl FnMut(JsValue) + 'static) {
        let c = Closure::<dyn FnMut(JsValue)>::new(f);
        setzen(&v.pc, ereignis, c.as_ref().clone());
        v.rueckrufe.borrow_mut().push(c);
    }

    fn trennen(&self, id: &str) {
        let Some(v) = self.innen.borrow_mut().verbindungen.remove(id) else {
            return;
        };
        for e in [
            "onnegotiationneeded",
            "onicecandidate",
            "oniceconnectionstatechange",
            "ontrack",
        ] {
            setzen(&v.pc, e, JsValue::NULL);
        }
        v.pc.close();
        v.audio.set_src_object(None);
        self.geaendert(false);
    }
}

/// `setLocalDescription()` ohne Argument (erzeugt Angebot oder Antwort selbst); web-sys kennt nur die Form mit.
async fn lokale_beschreibung_setzen(pc: &RtcPeerConnection) -> Result<(), String> {
    let f: js_sys::Function = js_sys::Reflect::get(pc, &"setLocalDescription".into())
        .map_err(|e| fehlertext(&e))?
        .unchecked_into();
    let p: js_sys::Promise = f.call0(pc).map_err(|e| fehlertext(&e))?.unchecked_into();
    JsFuture::from(p)
        .await
        .map(|_| ())
        .map_err(|e| fehlertext(&e))
}

fn sdp_typ(t: &str) -> RtcSdpType {
    match t {
        "offer" => RtcSdpType::Offer,
        "answer" => RtcSdpType::Answer,
        "pranswer" => RtcSdpType::Pranswer,
        _ => RtcSdpType::Rollback,
    }
}

fn erste_spur(s: &MediaStream, audio: bool) -> Option<MediaStreamTrack> {
    let spuren = if audio {
        s.get_audio_tracks()
    } else {
        s.get_video_tracks()
    };
    spuren.get(0).dyn_into().ok()
}

fn analyser_fuer(ctx: &AudioContext, s: &MediaStream) -> Option<AnalyserNode> {
    let quelle = ctx.create_media_stream_source(s).ok()?;
    let a = ctx.create_analyser().ok()?;
    a.set_fft_size(1024);
    quelle.connect_with_audio_node(&a).ok()?;
    Some(a)
}

fn rms(a: &AnalyserNode) -> f32 {
    let mut puffer = vec![0u8; a.fft_size() as usize];
    a.get_byte_time_domain_data(&mut puffer);
    let summe: f32 = puffer
        .iter()
        .map(|&v| ((v as f32 - 128.0) / 128.0).powi(2))
        .sum();
    (summe / puffer.len().max(1) as f32).sqrt()
}
