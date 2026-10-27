//! Die 3D-Welt von OpenAgenticOffice, gebaut mit kiss3d (WebGPU im Browser).
//!
//! Reine Logik (immer da, auch ohne Feature `3d`, z. B. für den Server): `grundriss`, `kollision`,
//! `bewegung`, `register`, `zustand`, `farben`. Die 3D-Seite (Feature `3d`, Standard) baut daraus
//! die Szene und läuft im Browser; nach außen zeigt sie die Funktionen in `api`.
//!
//! Start im Client:
//! ```ignore
//! welt::bei_senden(|n| socket.senden(n));
//! welt::bei_aktion(|a| oberflaeche.oeffnen(a));
//! wasm_bindgen_futures::spawn_local(welt::starten());
//! ```

pub mod bewegung;
pub mod farben;
pub mod grundriss;
pub mod kollision;
pub mod register;
pub mod spiellogik;
pub mod zustand;

#[cfg(feature = "3d")]
pub mod api;
#[cfg(feature = "3d")]
mod bau;
#[cfg(feature = "3d")]
pub mod eingabe;
#[cfg(feature = "3d")]
mod features;
#[cfg(feature = "3d")]
mod figur;
#[cfg(feature = "3d")]
pub mod grafik;
#[cfg(feature = "3d")]
mod kamera;
#[cfg(feature = "3d")]
mod lauf;
#[cfg(feature = "3d")]
pub mod welt;

#[cfg(feature = "3d")]
pub use api::{
    Bildschirm, CANVAS_ID, Hoerer, Klang, Umgebung, bei_hoerer, bei_klang, LeinwandBlick, WeltAktion, bei_abstaende, bei_aktion, bei_hinweis, bei_leinwand, bei_senden, bildschirm_leeren,
    bildschirm_setzen, eingabe_sperren, nachricht, perspektive_setzen, zur_etage,
};
#[cfg(feature = "3d")]
pub use eingabe::{zeiger_fangen, zeiger_freigeben};
#[cfg(feature = "3d")]
pub use lauf::starten;
