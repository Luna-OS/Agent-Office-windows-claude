//! `office-agenten`: die Kommandozeile, mit der sich die Agenten im OpenAgenticOffice abstimmen.
//! Sie spricht mit dem Hook-Server des Office (OFFICE_HOOK_URL) und weist sich mit
//! OFFICE_ARBEITER_ID und OFFICE_HOOK_TOKEN aus; beides setzt das Office jedem Arbeiter.

use std::io::Read;
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};

const HILFE: &str = "office-agenten – Zusammenarbeit der Agenten im OpenAgenticOffice

Befehle:
  wer                                   Wer ist auf der Etage, wer macht was (Status, Branch, Dateien)
  senden <name|alle> <text>             Nachricht an einen Agenten oder an alle
  posteingang                           Deine ungelesenen Nachrichten
  verlauf                               Die letzten Nachrichten, die dich betreffen
  status <text>                         Melden, woran du gerade arbeitest
  aufgaben                              Das gemeinsame Aufgabenboard
  aufgabe-neu <titel> [notiz]           Eine Aufgabe anlegen
  aufgabe-neu --auto <titel> <auftrag>  Eine Aufgabe, die das Office selbst einem neuen Arbeiter gibt
  aufgabe-weg <id>                      Eine wartende Aufgabe vom Board nehmen
  aufgabe <id> <offen|in_arbeit|erledigt|blockiert> [notiz]
                                        Eine Aufgabe übernehmen, abschließen oder blockieren
  anlegen [--provider <id>] <auftrag>   Einen weiteren Arbeiter mit einem Auftrag holen
  hilfe                                 Diese Hilfe

Intern (für Agent-Hooks): hook <provider> <ereignis>  (liest die Nutzlast von stdin)";

struct Verbindung {
    url: String,
    id: String,
    token: String,
    agent: ureq::Agent,
}

impl Verbindung {
    fn aus_umgebung() -> Result<Self, String> {
        let var = |n: &str| std::env::var(n).ok().filter(|v| !v.is_empty());
        let (Some(url), Some(id), Some(token)) = (var("OFFICE_HOOK_URL"), var("OFFICE_ARBEITER_ID"), var("OFFICE_HOOK_TOKEN")) else {
            return Err("Nicht im Office: OFFICE_HOOK_URL, OFFICE_ARBEITER_ID und OFFICE_HOOK_TOKEN fehlen. \
`office-agenten` läuft nur im Terminal eines Arbeiters."
                .into());
        };
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        Ok(Self { url: url.trim_end_matches('/').to_string(), id, token, agent })
    }

    fn anfrage(&self, methode: &str, pfad: &str, koerper: Option<Value>) -> Result<Value, String> {
        let url = format!("{}{pfad}", self.url);
        let auth = format!("Bearer {}", self.token);
        let antwort = match (methode, koerper) {
            ("GET", _) => self.agent.get(&url).header("x-office-arbeiter", &self.id).header("authorization", &auth).call(),
            (_, k) => self
                .agent
                .post(&url)
                .header("x-office-arbeiter", &self.id)
                .header("authorization", &auth)
                .send_json(k.unwrap_or(json!({}))),
        };
        let mut antwort = antwort.map_err(|e| format!("Das Office ist nicht erreichbar ({e})"))?;
        let status = antwort.status().as_u16();
        let wert: Value = antwort.body_mut().read_json().unwrap_or(Value::Null);
        if status >= 400 {
            return Err(wert.get("fehler").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("Fehler {status}")));
        }
        Ok(wert)
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn nachrichten_zeigen(liste: &Value) {
    let Some(liste) = liste.as_array().filter(|l| !l.is_empty()) else {
        println!("Keine neuen Nachrichten.");
        return;
    };
    for n in liste {
        let an = n.get("an").and_then(Value::as_str).map(|a| format!(" an {a}")).unwrap_or_else(|| " an alle".into());
        println!("#{} {}{}: {}", n.get("id").and_then(Value::as_u64).unwrap_or(0), s(n, "von"), an, s(n, "text"));
    }
}

fn wer_zeigen(v: &Value) {
    println!("Etage „{}“:", s(v, "etage"));
    for a in v.get("arbeiter").and_then(Value::as_array).into_iter().flatten() {
        let du = if a.get("du").and_then(Value::as_bool) == Some(true) { " (du)" } else { "" };
        println!("- {}{} [{}] – {}", s(a, "name"), du, a.get("provider").and_then(Value::as_str).unwrap_or("shell"), s(a, "status_text"));
        if !s(a, "notiz").is_empty() {
            println!("    Stand: {}", s(a, "notiz"));
        }
        if !s(a, "aktivitaet").is_empty() {
            println!("    Zuletzt: {}", s(a, "aktivitaet"));
        }
        if !s(a, "branch").is_empty() {
            println!("    Branch: {}", s(a, "branch"));
        }
        let dateien: Vec<&str> = a.get("dateien").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        if !dateien.is_empty() {
            println!("    Dateien: {}", dateien.join(", "));
        }
    }
}

fn aufgaben_zeigen(v: &Value) {
    let liste: Vec<&Value> = v.get("aufgaben").and_then(Value::as_array).into_iter().flatten().collect();
    if liste.is_empty() {
        println!("Das Board ist leer.");
    }
    for a in liste {
        let besitzer = a.get("besitzer").and_then(Value::as_str).map(|b| format!(" – {b}")).unwrap_or_default();
        println!("#{} [{}] {}{}", a.get("id").and_then(Value::as_u64).unwrap_or(0), s(a, "status"), s(a, "titel"), besitzer);
        if !s(a, "notiz").is_empty() {
            println!("    {}", s(a, "notiz"));
        }
    }
}

/// Für Agent-Hooks: Nutzlast von stdin an den Hook-Server. Schweigt immer und endet mit 0,
/// damit ein Hook nie den Agenten stört.
fn hook(args: &[String]) -> ExitCode {
    let (Some(provider), Some(ereignis)) = (args.first(), args.get(1)) else { return ExitCode::SUCCESS };
    let mut nutzlast = Vec::new();
    let _ = std::io::stdin().take(1024 * 1024).read_to_end(&mut nutzlast);
    let Ok(v) = Verbindung::aus_umgebung() else { return ExitCode::SUCCESS };
    let url = format!("{}/hooks/{provider}?ereignis={ereignis}", v.url);
    // Startet das Office gerade neu, kurz warten und es noch einmal versuchen.
    for _ in 0..6 {
        let r = v
            .agent
            .post(&url)
            .header("x-office-arbeiter", &v.id)
            .header("authorization", &format!("Bearer {}", v.token))
            .header("content-type", "application/json")
            .send(&nutzlast[..]);
        if r.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    ExitCode::SUCCESS
}

fn ausfuehren(args: &[String]) -> Result<(), String> {
    let befehl = args.first().map(String::as_str).unwrap_or("hilfe");
    let rest = &args[args.len().min(1)..];
    if matches!(befehl, "hilfe" | "help" | "-h" | "--help") {
        println!("{HILFE}");
        return Ok(());
    }
    let v = Verbindung::aus_umgebung()?;
    match befehl {
        "wer" => wer_zeigen(&v.anfrage("GET", "/office/wer", None)?),
        "senden" => {
            let (Some(an), true) = (rest.first(), rest.len() >= 2) else { return Err("So: office-agenten senden <name|alle> <text>".into()) };
            let r = v.anfrage("POST", "/office/senden", Some(json!({ "an": an, "text": rest[1..].join(" ") })))?;
            let empfaenger: Vec<&str> = r.get("empfaenger").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
            if let Some(h) = r.get("hinweis").and_then(Value::as_str) {
                println!("{h}");
            } else if empfaenger.is_empty() {
                println!("Gesendet (gerade ist kein anderer Agent da).");
            } else {
                println!("Gesendet an {}.", empfaenger.join(", "));
            }
        }
        "posteingang" => nachrichten_zeigen(&v.anfrage("GET", "/office/posteingang", None)?),
        "verlauf" => nachrichten_zeigen(&v.anfrage("GET", "/office/verlauf", None)?),
        "status" => {
            v.anfrage("POST", "/office/status", Some(json!({ "text": rest.join(" ") })))?;
            println!("Status gemeldet.");
        }
        "aufgaben" => aufgaben_zeigen(&v.anfrage("GET", "/office/aufgaben", None)?),
        "aufgabe-neu" => {
            let (auto, rest) = match rest.first().map(String::as_str) {
                Some("--auto") => (true, &rest[1..]),
                _ => (false, rest),
            };
            let titel = rest.first().ok_or("So: office-agenten aufgabe-neu [--auto] <titel> [notiz bzw. auftrag]")?;
            let notiz = (rest.len() > 1).then(|| rest[1..].join(" "));
            if auto && notiz.is_none() {
                return Err("Mit --auto braucht die Aufgabe einen vollständigen Auftrag".into());
            }
            let r = v.anfrage("POST", "/office/aufgaben", Some(json!({ "titel": titel, "notiz": notiz, "automatisch": auto })))?;
            println!("Aufgabe #{} angelegt.", r.get("id").and_then(Value::as_u64).unwrap_or(0));
        }
        "aufgabe" => {
            let (Some(id), Some(status)) = (rest.first(), rest.get(1)) else {
                return Err("So: office-agenten aufgabe <id> <offen|in_arbeit|erledigt|blockiert> [notiz]".into());
            };
            let id = id.trim_start_matches('#');
            let notiz = (rest.len() > 2).then(|| rest[2..].join(" "));
            let r = v.anfrage("POST", &format!("/office/aufgaben/{id}"), Some(json!({ "status": status, "notiz": notiz })))?;
            println!("Aufgabe #{} ist jetzt „{}“.", id, s(&r, "status"));
        }
        "aufgabe-weg" => {
            let id = rest.first().ok_or("So: office-agenten aufgabe-weg <id>")?.trim_start_matches('#').to_string();
            v.anfrage("POST", &format!("/office/aufgaben/{id}/weg"), None)?;
            println!("Aufgabe #{id} ist vom Board.");
        }
        "anlegen" => {
            let (provider, auftrag) = match rest {
                [flag, p, auftrag @ ..] if flag == "--provider" => (Some(p.clone()), auftrag.join(" ")),
                _ => (None, rest.join(" ")),
            };
            let r = v.anfrage("POST", "/office/anlegen", Some(json!({ "prompt": auftrag, "provider": provider })))?;
            println!("{} ist angelegt und arbeitet daran.", s(&r, "name"));
        }
        _ => return Err(format!("Unbekannter Befehl „{befehl}“.\n\n{HILFE}")),
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("hook") {
        return hook(&args[1..]);
    }
    match ausfuehren(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            eprintln!("office-agenten: {f}");
            ExitCode::FAILURE
        }
    }
}
