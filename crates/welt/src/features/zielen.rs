//! Worauf man zielt: das benutzbare Ding in Reichweite, das am ehesten in Blickrichtung liegt.
//! Die Schleife benutzt es mit E oder Klick; der Hinweis dazu geht an die Oberfläche.

use crate::register::{Phase, Register, anvisieren};
use crate::welt::Kontext;

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.tick(Phase::Zielen, |kx: &mut Kontext, _| {
        let auge = kx.spieler.auge();
        let blick = kx.spieler.blickrichtung();
        if kx.eingabe.gesperrt {
            kx.ziel = None;
            return;
        }
        let fest = anvisieren(&kx.gebaut.nutzbar, auge, blick).cloned();
        let beweglich = anvisieren(&kx.dinge, auge, blick).cloned();
        // Von beiden das nähere.
        let abstand = |n: &crate::register::Nutzbar| (n.pos[0] - auge[0]).hypot(n.pos[2] - auge[2]);
        kx.ziel = match (fest, beweglich) {
            (Some(a), Some(b)) => Some(if abstand(&b) < abstand(&a) { b } else { a }),
            (a, b) => a.or(b),
        };
    });
}
