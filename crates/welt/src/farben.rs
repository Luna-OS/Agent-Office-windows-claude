//! Die Farben der Welt: Etagen-Paletten, Möbel, Haut und Haare (wie im Original).

/// Die Farben einer Etage (Index `EtageInfo::palette`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtagenPalette {
    pub name: &'static str,
    pub wand: &'static str,
    pub leiste: &'static str,
    pub boden: &'static str,
    pub boden2: &'static str,
    pub fuge: &'static str,
}

const fn p(name: &'static str, wand: &'static str, leiste: &'static str, boden: &'static str, boden2: &'static str, fuge: &'static str) -> EtagenPalette {
    EtagenPalette { name, wand, leiste, boden, boden2, fuge }
}

pub const ETAGEN_PALETTEN: [EtagenPalette; 10] = [
    p("Ahorn", "#fff6ea", "#e8a87c", "#f2d7b0", "#e9c89a", "#d9b88c"),
    p("Minze", "#e3f6ec", "#40a878", "#cfe6d9", "#bcdcc9", "#9fc6b0"),
    p("Himmel", "#e7f0ff", "#4f7fe0", "#d6dde9", "#c5cedd", "#aab5c8"),
    p("Lavendel", "#f2eaff", "#9470e0", "#e1d7ef", "#d2c4e7", "#b8a6d6"),
    p("Pfirsich", "#ffefe6", "#ea7352", "#efc6a5", "#e5b48e", "#cf9c76"),
    p("Zitrone", "#fffbe0", "#dcaa16", "#e9d8a4", "#dec98b", "#c8b271"),
    p("Walnuss", "#f5eee5", "#8b5e3c", "#aa7650", "#9b6845", "#7c5236"),
    p("Schiefer", "#edf1f5", "#3d5a80", "#b9c3cd", "#aab5c0", "#8d99a6"),
    p("Rose", "#ffeaf0", "#e0567f", "#eed3da", "#e4c1cb", "#cea5b2"),
    p("Petrol", "#e1f7f6", "#1a9a9a", "#c3e2de", "#b0d7d2", "#92c3bd"),
];

pub fn etagen_palette(i: u32) -> EtagenPalette {
    ETAGEN_PALETTEN[i as usize % ETAGEN_PALETTEN.len()]
}

pub const TISCH: &str = "#f7f3ea";
pub const TISCHBEIN: &str = "#3d405b";
pub const HOLZ: &str = "#c98b5a";
pub const KORK: &str = "#d8a86a";
pub const STUEHLE: [&str; 6] = ["#ff8a5b", "#5bc0eb", "#9bc53d", "#b388eb", "#ffb400", "#f7aef8"];
pub const TEPPICHE: [&str; 4] = ["#bde0fe", "#ffd6a5", "#caffbf", "#ffc6ff"];
pub const PFLANZE: &str = "#5fb760";
pub const PFLANZE_DUNKEL: &str = "#3f8f45";
pub const TOPF: &str = "#e76f51";
pub const TINTE: &str = "#2b2d42";
/// Der Außenanstrich des Gebäudes.
pub const AUSSEN: &str = "#e07a5f";
pub const GLAS: &str = "#d6f1ff";
pub const ALU: &str = "#aab4be";

/// Hauttöne (Index `Aussehen::haut`).
pub const HAUT: [&str; 8] = ["#ffdbac", "#f1c27d", "#e0ac69", "#c68642", "#8d5524", "#ffe0bd", "#d6a07a", "#5c3a1e"];
/// Haarfarben (Index `Aussehen::haare`).
pub const HAARE: [&str; 10] = ["#2b1d0e", "#4a2c17", "#8b5a2b", "#d4a056", "#f1d38b", "#b33a1c", "#1c1c1c", "#9e9e9e", "#e85d9a", "#4f7fe0"];

/// Statusfarben der Arbeiter (Lämpchen über dem Kopf).
pub mod status {
    pub const ARBEITET: &str = "#ffd166";
    pub const WARTET: &str = "#ef476f";
    pub const FERTIG: &str = "#06d6a0";
    pub const BEREIT: &str = "#8ecae6";
    pub const SCHLAEFT: &str = "#adb5bd";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_wickelt() {
        assert_eq!(etagen_palette(10), ETAGEN_PALETTEN[0]);
        assert_eq!(etagen_palette(3).name, "Lavendel");
    }
}
