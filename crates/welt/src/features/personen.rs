//! Die Menschen: die anderen auf der Etage (weich zu ihrer gemeldeten Position), Namensschilder,
//! Emotes, und die eigene Figur (sichtbar, sobald die Kamera hinter dem Kopf ist).
//! Meldet jeden Frame die Abstände zu allen (für die Lautstärke im Voice).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use kiss3d::prelude::*;
use protokoll::praesenz::{Emote, Person};

use crate::bau::farbe_srgb;
use crate::figur::{Figur, Haltung, Look, winkel_hin};
use crate::grundriss::sitze;
use crate::register::{Phase, Register};
use crate::welt::Kontext;

struct Andere {
    figur: Figur,
    pos: Vec3,
    blick: f32,
    emote: Option<(Emote, f64)>,
    look: Look,
}

#[derive(Default)]
struct Stand {
    andere: BTreeMap<String, Andere>,
    ich: Option<(Figur, Look)>,
}

fn look(p: &Person) -> Look {
    Look {
        hemd: p.farbe.clone(),
        haut: p.aussehen.haut,
        haare: p.aussehen.haare,
        frisur: p.aussehen.frisur,
    }
}

fn emote_text(e: Emote) -> &'static str {
    match e {
        Emote::Winken => "winkt",
        Emote::Daumen => "Daumen hoch",
        Emote::Klatschen => "klatscht",
        Emote::Tanzen => "tanzt",
        Emote::Zeigen => "zeigt",
        Emote::Facepalm => "Facepalm",
    }
}

pub fn installieren(reg: &mut Register<Kontext>) {
    let stand = Rc::new(RefCell::new(Stand::default()));

    let s = stand.clone();
    reg.tick(Phase::Andere, move |kx: &mut Kontext, dt| {
        let mut st = s.borrow_mut();
        if kx.neu_gebaut {
            for (_, mut a) in std::mem::take(&mut st.andere) {
                a.figur.entfernen();
            }
        }
        // Wer nicht mehr hier ist, geht; wer neu ist, kommt; geändertes Aussehen: neu bauen.
        let hier: BTreeMap<String, Person> = kx
            .zustand
            .personen_hier()
            .map(|p| (p.id.clone(), p.clone()))
            .collect();
        st.andere.retain(|id, a| {
            let bleibt = hier.get(id).is_some_and(|p| look(p) == a.look);
            if !bleibt {
                a.figur.entfernen();
            }
            bleibt
        });
        for (id, p) in &hier {
            if !st.andere.contains_key(id) {
                let l = look(p);
                let figur = Figur::neu(&mut kx.etage, &l);
                st.andere.insert(
                    id.clone(),
                    Andere {
                        figur,
                        pos: Vec3::new(p.x, p.y, p.z),
                        blick: p.rot_y,
                        emote: None,
                        look: l,
                    },
                );
            }
        }
        for (id, e) in &kx.aenderungen.emotes {
            if let Some(a) = st.andere.get_mut(id) {
                a.emote = Some((*e, kx.zeit + crate::figur::EMOTE_DAUER as f64));
                a.figur.emote(*e);
            }
        }
        let mut abstaende = Vec::with_capacity(st.andere.len());
        let ich = Vec3::from(kx.spieler.pos());
        for (id, a) in st.andere.iter_mut() {
            let p = &hier[id];
            let sitz = p.sitz.as_deref().and_then(sitze::platz_nach_schluessel);
            let (ziel, blick) = match &sitz {
                Some(pl) => (Vec3::new(pl.x, pl.y, pl.z), pl.rot_y),
                None => (Vec3::new(p.x, p.y, p.z), p.rot_y),
            };
            // Weich hinterher, große Sprünge (Etagenwechsel, Hinsetzen) sofort.
            let k = (dt * 12.0).min(1.0);
            a.pos = if (ziel - a.pos).length() > 4.0 {
                ziel
            } else {
                a.pos + (ziel - a.pos) * k
            };
            a.blick = winkel_hin(a.blick, blick, k);
            a.figur.haltung = match &sitz {
                Some(pl) => Haltung::Sitzen(pl.huefte),
                None if p.laeuft => Haltung::Laufen,
                None => Haltung::Stehen,
            };
            let y = a.pos.y
                + sitz
                    .as_ref()
                    .map(|pl| Figur::sitz_versatz(pl.huefte))
                    .unwrap_or(0.0);
            a.figur.setzen(a.pos.x, y, a.pos.z, a.blick);
            a.figur.halten(crate::figur::halt_von(p));
            a.figur.animieren(dt);
            let mut text = p.name.clone();
            if let Some((e, bis)) = a.emote {
                if kx.zeit < bis {
                    text = format!("{} {}", p.name, emote_text(e));
                } else {
                    a.emote = None;
                }
            }
            let kopf = a.figur.kopf_welt();
            kx.beschriften(text, [kopf.x, kopf.y + 0.2, kopf.z], farbe_srgb(&p.farbe));
            abstaende.push((id.clone(), (a.pos - ich).length()));
        }
        crate::api::abstaende_melden(&abstaende);
    });

    // Die eigene Figur: nur in der Ansicht hinter dem Kopf.
    let s = stand;
    reg.tick(Phase::Ich, move |kx: &mut Kontext, dt| {
        let mut st = s.borrow_mut();
        let gewuenscht = kx.zustand.ich_selbst.as_ref().map(look).unwrap_or(Look {
            hemd: "#4f7fe0".into(),
            haut: 0,
            haare: 0,
            frisur: 0,
        });
        let neu = kx.neu_gebaut || st.ich.as_ref().is_none_or(|(_, l)| *l != gewuenscht);
        if neu {
            if let Some((mut f, _)) = st.ich.take() {
                f.entfernen();
            }
            st.ich = Some((Figur::neu(&mut kx.etage, &gewuenscht), gewuenscht));
        }
        let Some((f, _)) = st.ich.as_mut() else {
            return;
        };
        for (id, e) in &kx.aenderungen.emotes {
            if Some(id) == kx.zustand.ich.as_ref() {
                f.emote(*e);
            }
        }
        f.sichtbar(kx.kamera.abstand > 0.0);
        let p = kx.spieler.pos();
        let sitzt = kx.ich_sitze;
        f.haltung = match sitzt {
            Some(h) => Haltung::Sitzen(h),
            None if kx.spieler.laeuft => Haltung::Laufen,
            None => Haltung::Stehen,
        };
        let y = p[1] + sitzt.map(Figur::sitz_versatz).unwrap_or(0.0);
        f.setzen(p[0], y, p[2], kx.spieler.blick);
        f.halten(
            kx.zustand
                .ich_selbst
                .as_ref()
                .and_then(crate::figur::halt_von),
        );
        f.animieren(dt);
    });
}
