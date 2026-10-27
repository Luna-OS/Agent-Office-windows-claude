//! Browser-Tests von OpenAgenticOffice: ein echter Server mit Wegwerf-Daten, der gebaute Client aus
//! `dist/` und ein kopfloses Chromium über das DevTools-Protokoll (eigener kleiner Client, `cdp`).
//!
//! Ohne Chromium oder ohne gebauten Client werden die Tests übersprungen (mit Hinweis).
//! Chromium: `OFFICE_CHROMIUM` oder das Playwright-Chromium unter %LOCALAPPDATA%\ms-playwright.
//! Bildschirmfotos landen in `target/ende-zu-ende/`.

pub mod browser;
pub mod cdp;
pub mod helfer;
pub mod office;
pub mod seite;

pub use browser::Browser;
pub use office::TestOffice;
pub use seite::Seite;
