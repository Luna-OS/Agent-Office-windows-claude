//! `open-agentic-office konten …`: Konten ohne laufendes Office verwalten (das laufende Office liest
//! konten.json beim nächsten Start; Einladungen gelten sofort, weil Links erst beim Einlösen geprüft werden).

use std::path::Path;

use anyhow::{Result, bail};
use protokoll::konten::Rolle;

use crate::anmeldung::Konten;

fn tag(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64).map(|d| d.with_timezone(&chrono::Local).format("%d.%m.%Y %H:%M").to_string()).unwrap_or_default()
}

pub fn ausfuehren(daten: &Path, args: &[String]) -> Result<()> {
    let mut konten = Konten::laden(daten);
    let admin = args.iter().any(|a| a == "--admin");
    let rest: Vec<&str> = args.iter().map(String::as_str).filter(|a| *a != "--admin").collect();
    match rest.as_slice() {
        [] | ["liste"] => {
            if konten.liste().is_empty() {
                println!("Noch keine Konten (nur das gemeinsame Passwort).");
            }
            for k in konten.liste() {
                let rolle = if k.rolle == Rolle::Admin { "admin" } else { "mitglied" };
                println!("  {:<24}  {:<8}  seit {}, von {}", k.name, rolle, tag(k.erstellt_um), k.erstellt_von);
            }
            for e in konten.offene_einladungen() {
                println!("  Einladung {:<14}  bis {}  /beitreten#{}", e.name.as_deref().unwrap_or("(wählt selbst)"), tag(e.gueltig_bis), e.token);
            }
            println!("Gemeinsames Passwort: {}", if konten.gemeinsames_passwort() { "an" } else { "aus" });
        }
        ["einladen", name @ ..] => {
            let name = name.first().copied();
            let e = konten.einladen("Terminal", if admin { Rolle::Admin } else { Rolle::Mitglied }, name).map_err(anyhow::Error::msg)?;
            println!("Einladungslink (7 Tage gültig, einmal benutzbar): /beitreten#{}", e.token);
            println!("Vor den Link gehört die Adresse des Office, z. B. https://office.example/beitreten#{}", e.token);
        }
        ["entziehen", name] => {
            let Some(id) = konten.nach_name(name).map(|k| k.id.clone()) else { bail!("Kein Konto namens {name}") };
            konten.entziehen(&id);
            println!("Konto {name} entzogen.");
        }
        ["rolle", name, rolle] => {
            let rolle = match *rolle {
                "admin" => Rolle::Admin,
                "mitglied" => Rolle::Mitglied,
                r => bail!("Unbekannte Rolle {r} (admin oder mitglied)"),
            };
            let Some(id) = konten.nach_name(name).map(|k| k.id.clone()) else { bail!("Kein Konto namens {name}") };
            konten.rolle_setzen(&id, rolle).map_err(anyhow::Error::msg)?;
            println!("{name} ist jetzt {}.", if rolle == Rolle::Admin { "Admin" } else { "Mitglied" });
        }
        ["passwort", an @ ("an" | "aus")] => {
            konten.gemeinsames_passwort_setzen(*an == "an").map_err(anyhow::Error::msg)?;
            println!("Gemeinsames Passwort ist {an}.");
        }
        _ => bail!("Unbekannter Konten-Befehl. Möglich: liste, einladen [NAME] [--admin], entziehen NAME, rolle NAME admin|mitglied, passwort an|aus"),
    }
    Ok(())
}
