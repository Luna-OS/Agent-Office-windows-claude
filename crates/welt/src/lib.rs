//! Die 3D-Welt von OpenAgenticOffice, gebaut mit kiss3d (WebGPU im Browser).
//!
//! Reine Logik (immer da, auch ohne Feature `3d`): `grundriss`, `kollision`, `bewegung`,
//! `register`, `zustand`. Die 3D-Seite (Feature `3d`) baut daraus die Szene und läuft im Browser.

pub mod bewegung;
pub mod grundriss;
pub mod kollision;
pub mod register;
pub mod zustand;

#[cfg(feature = "3d")]
pub mod grafik;
