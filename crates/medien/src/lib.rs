//! Bildschirm teilen über HTTP, YouTube-Leinwand auf dem Dach, Sprache und Sound.
//!
//! Ohne Features nur die reine Logik (URL lesen, Leinwand-Synchronisation, Jukebox, Lautstärke nach Entfernung).
//! Feature `server`: axum-Routen und Zustand. Feature `client`: Browser-Seite über web-sys.

pub mod dnb;
pub mod jukebox;
pub mod leinwand;
pub mod naehe;
pub mod tonfolge;
pub mod youtube;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;
