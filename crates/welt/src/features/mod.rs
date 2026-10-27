//! Die Features der Welt, jedes ein Modul mit `installieren`, in dieser Reihenfolge registriert
//! (die Reihenfolge ist die der Ticks innerhalb einer Phase und der Tastenbindungen).

pub mod abgang;
pub mod arbeiter;
pub mod arcade;
pub mod aufzug;
pub mod autos;
pub mod ball;
pub mod barspiele;
pub mod dekor;
pub mod golf;
pub mod hund;
pub mod kerker;
pub mod leinwand;
pub mod moebel;
pub mod personen;
pub mod sitzen;
pub mod steuerung;
pub mod zielen;

use crate::register::Register;
use crate::welt::Kontext;

pub fn installieren(reg: &mut Register<Kontext>, _kx: &mut Kontext) {
    steuerung::installieren(reg);
    zielen::installieren(reg);
    personen::installieren(reg);
    arbeiter::installieren(reg);
    aufzug::installieren(reg);
    hund::installieren(reg);
    ball::installieren(reg);
    autos::installieren(reg);
    golf::installieren(reg);
    dekor::installieren(reg);
    arcade::installieren(reg);
    barspiele::installieren(reg);
    kerker::installieren(reg);
    sitzen::installieren(reg);
    moebel::installieren(reg);
    leinwand::installieren(reg);
}
