//! Bausteine nach den Komponenten von rust-ui (https://rust-ui.com), für Leptos 0.8 umgesetzt.
//!
//! rust-ui ist ein Baukasten zum Kopieren: Tailwind-Klassen, mit `tw_merge` zusammengeführt. Die
//! Farben (`bg-primary`, `border` …) kommen aus den CSS-Variablen in `crates/client/stil/thema.css`.

mod abzeichen;
mod eingabe;
mod karte;
mod knopf;
mod reiter;

pub use abzeichen::{Abzeichen, AbzeichenArt};
pub use eingabe::{Eingabe, Textfeld};
pub use karte::{Karte, KarteInhalt, KarteKopf, KarteTitel};
pub use knopf::{Knopf, KnopfArt, KnopfGroesse};
pub use reiter::{Reiter, ReiterLeiste};
