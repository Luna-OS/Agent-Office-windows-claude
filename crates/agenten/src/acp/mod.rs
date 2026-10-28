//! Agenten ohne Terminal, die ACP sprechen (Agent Client Protocol: JSON-RPC 2.0 zeilenweise über
//! stdin/stdout), z. B. DeepSeek Harness. Das Office hält die ACP-Verbindung und zeichnet, was der
//! Agent tut, selbst in das Terminal des Arbeiters; was ein Mensch dort tippt, wird zum Auftrag.
//!
//! Status geht über dieselben Signale wie bei Terminal-Agenten: OSC 9;4;3 (arbeitet), 9;4;0
//! (fertig), 9;4;4 (wartet auf einen Menschen), und die Sitzungs-ID über OSC 7700;sitzung;<id>.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::pty::{Antworter, Befehl};

enum Ereignis {
    Zeile(String),
    Eingabe(Vec<u8>),
    Ende,
}

#[derive(Clone, Copy, PartialEq)]
enum Zweck {
    Init,
    Fortsetzen,
    Neu,
    Auftrag,
    Einstellung,
}

/// Ein ACP-Agent, wie ihn ein [`crate::pty::Pty`] hält.
pub struct AcpPty {
    pid: u32,
    tx: mpsc::Sender<Ereignis>,
    kind: std::sync::Arc<std::sync::Mutex<Child>>,
}

impl AcpPty {
    pub fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }
    pub fn schreiben(&self, daten: &[u8]) {
        let _ = self.tx.send(Ereignis::Eingabe(daten.to_vec()));
    }
    pub fn beenden(&self) {
        let _ = self.kind.lock().unwrap_or_else(|e| e.into_inner()).kill();
    }
}

struct Sitzung<D: FnMut(&[u8], &Antworter)> {
    stdin: std::process::ChildStdin,
    aus: D,
    naechste: u64,
    offen: HashMap<u64, Zweck>,
    sitzung: Option<String>,
    erster: Option<String>,
    fortsetzen: Option<String>,
    cwd: String,
    zeile: String,
    laeuft: bool,
    /// Offene Erlaubnis-Anfrage: (RPC-ID, Options-IDs).
    erlaubnis: Option<(Value, Vec<String>)>,
    /// Gewünschte Einstellungen (`model`, `reasoning_effort`) und die angebotenen Optionen.
    wahl: Vec<(String, String)>,
    optionen: Vec<Value>,
}

/// Was für einen ACP-Agenten gewählt ist: Modell und Denk-Aufwand.
#[derive(Debug, Clone, Default)]
pub struct Wahl {
    pub modell: Option<String>,
    pub aufwand: Option<String>,
}

/// Der Aufwand des Office auf der Leiter von DeepSeek Harness (`off`, `low`, `high`, `max`).
pub fn aufwand_stufe(a: &str) -> &'static str {
    match a {
        "low" => "low",
        "xhigh" | "max" => "max",
        _ => "high",
    }
}

/// Die Werte, die eine Konfigurations-Option anbietet: (Wert, Name). Modelle kommen gruppiert.
fn werte(option: &Value) -> Vec<(String, String)> {
    let mut aus = Vec::new();
    let mut nimm = |e: &Value| {
        if let Some(w) = e.get("value").and_then(Value::as_str) {
            aus.push((
                w.to_string(),
                e.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ));
        }
    };
    for e in option
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match e.get("options").and_then(Value::as_array) {
            Some(innen) => innen.iter().for_each(&mut nimm),
            None => nimm(e),
        }
    }
    aus
}

/// Welcher angebotene Wert zu `gewuenscht` passt: genau, nach Namen, oder als Modell in einer
/// Route (`["anbieter","modell"]`). Ohne Angebot gilt der Wunsch, wie er ist.
pub fn wert_finden(gewuenscht: &str, option: &Value) -> String {
    let g = gewuenscht.trim();
    let w = werte(option);
    if let Some((v, _)) = w.iter().find(|(v, _)| v == g) {
        return v.clone();
    }
    if let Some((v, _)) = w.iter().find(|(_, n)| n.eq_ignore_ascii_case(g)) {
        return v.clone();
    }
    let route = |v: &str| {
        serde_json::from_str::<Vec<String>>(v)
            .ok()
            .and_then(|r| r.last().cloned())
    };
    w.iter()
        .find(|(v, _)| route(v).is_some_and(|m| m.eq_ignore_ascii_case(g)))
        .map(|(v, _)| v.clone())
        .unwrap_or_else(|| g.to_string())
}

/// Startet einen ACP-Agenten. `erster`: der erste Auftrag; `fortsetzen`: eine Sitzung zum Fortsetzen.
pub fn starten(
    befehl: &Befehl,
    wahl: Wahl,
    erster: Option<String>,
    fortsetzen: Option<String>,
    bei_daten: impl FnMut(&[u8], &Antworter) + Send + 'static,
    bei_ende: impl FnOnce(Option<i32>) + Send + 'static,
) -> Result<AcpPty> {
    let mut cmd = Command::new(&befehl.programm);
    cmd.args(&befehl.argumente)
        .current_dir(&befehl.verzeichnis)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for k in &befehl.entfernen {
        cmd.env_remove(k);
    }
    cmd.envs(&befehl.umgebung);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut kind = cmd
        .spawn()
        .with_context(|| format!("{} startet nicht", befehl.programm.display()))?;
    let stdin = kind.stdin.take().context("keine Eingabe")?;
    let stdout = kind.stdout.take().context("keine Ausgabe")?;
    let pid = kind.id();
    let kind = std::sync::Arc::new(std::sync::Mutex::new(kind));
    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();
    std::thread::Builder::new()
        .name("acp-lesen".into())
        .spawn(move || {
            for zeile in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx2.send(Ereignis::Zeile(zeile)).is_err() {
                    break;
                }
            }
            let _ = tx2.send(Ereignis::Ende);
        })?;
    let mut s = Sitzung {
        stdin,
        aus: bei_daten,
        naechste: 0,
        offen: HashMap::new(),
        sitzung: None,
        erster,
        fortsetzen,
        cwd: befehl.verzeichnis.to_string_lossy().into_owned(),
        zeile: String::new(),
        laeuft: false,
        erlaubnis: None,
        wahl: wahl
            .modell
            .map(|m| ("model".to_string(), m))
            .into_iter()
            .chain(wahl.aufwand.map(|a| {
                (
                    "reasoning_effort".to_string(),
                    aufwand_stufe(&a).to_string(),
                )
            }))
            .collect(),
        optionen: Vec::new(),
    };
    let k2 = kind.clone();
    std::thread::Builder::new().name("acp".into()).spawn(move || {
        s.zeigen("\x1b]9;4;3\x07\x1b[2m  ACP-Agent startet …\x1b[0m\r\n");
        s.anfrage("initialize", json!({ "protocolVersion": 1, "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false } }, "clientInfo": { "name": "open-agentic-office", "version": "0.1.0" } }), Zweck::Init);
        while let Ok(e) = rx.recv() {
            match e {
                Ereignis::Zeile(z) => s.nachricht(&z),
                Ereignis::Eingabe(d) => s.eingabe(&d),
                Ereignis::Ende => break,
            }
        }
        let code = k2.lock().unwrap_or_else(|e| e.into_inner()).wait().ok().and_then(|c| c.code());
        bei_ende(code);
    })?;
    Ok(AcpPty { pid, tx, kind })
}

fn text(v: &Value, zeiger: &str) -> String {
    v.pointer(zeiger)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Text fürs Terminal: Zeilenumbrüche als CRLF, keine Steuerzeichen außer Tab.
fn sauber(t: &str) -> String {
    t.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .replace('\n', "\r\n")
}

impl<D: FnMut(&[u8], &Antworter)> Sitzung<D> {
    fn zeigen(&mut self, t: &str) {
        (self.aus)(t.as_bytes(), &Antworter::stumm());
    }

    fn senden(&mut self, v: Value) {
        let _ = writeln!(self.stdin, "{v}");
        let _ = self.stdin.flush();
    }

    fn anfrage(&mut self, methode: &str, params: Value, zweck: Zweck) {
        self.naechste += 1;
        self.offen.insert(self.naechste, zweck);
        let id = self.naechste;
        self.senden(json!({ "jsonrpc": "2.0", "id": id, "method": methode, "params": params }));
    }

    fn auftrag(&mut self, t: &str) {
        let Some(sitzung) = self.sitzung.clone() else {
            return;
        };
        self.laeuft = true;
        self.zeigen(&format!(
            "\x1b]9;4;3\x07\r\n\x1b[1m> {}\x1b[0m\r\n",
            sauber(t)
        ));
        self.anfrage(
            "session/prompt",
            json!({ "sessionId": sitzung, "prompt": [{ "type": "text", "text": t }] }),
            Zweck::Auftrag,
        );
    }

    fn sitzung_da(&mut self, id: String, ergebnis: &Value) {
        self.zeigen(&format!(
            "\x1b]7700;sitzung;{id}\x07\x1b[2m  Sitzung {id}\x1b[0m\r\n"
        ));
        self.sitzung = Some(id.clone());
        if let Some(o) = ergebnis
            .pointer("/result/configOptions")
            .and_then(Value::as_array)
        {
            self.optionen = o.clone();
        }
        // Modell und Aufwand dieses Arbeiters; die Anfragen gehen vor dem ersten Auftrag hinaus.
        for (schluessel, gewuenscht) in std::mem::take(&mut self.wahl) {
            let Some(option) = self
                .optionen
                .iter()
                .find(|o| o.get("id").and_then(Value::as_str) == Some(&schluessel))
                .cloned()
            else {
                self.zeigen(&format!("\x1b[2m  Der Agent bietet kein {schluessel} an – es bleibt beim Standard\x1b[0m\r\n"));
                continue;
            };
            let wert = wert_finden(&gewuenscht, &option);
            self.zeigen(&format!(
                "\x1b[2m  {schluessel} → {}\x1b[0m\r\n",
                sauber(&wert)
            ));
            self.anfrage(
                "session/set_config_option",
                json!({ "sessionId": id, "configId": schluessel, "value": wert }),
                Zweck::Einstellung,
            );
        }
        match self.erster.take() {
            Some(t) => self.auftrag(&t),
            None => self.zeigen("\x1b]9;4;0\x07"),
        }
    }

    fn nachricht(&mut self, zeile: &str) {
        // Manche Programme schreiben etwas vor ihr JSON: ab der ersten Klammer lesen.
        let zeile = zeile.find('{').map_or("", |i| &zeile[i..]);
        let Ok(v) = serde_json::from_str::<Value>(zeile) else {
            return;
        };
        let methode = v.get("method").and_then(Value::as_str);
        match (methode, v.get("id")) {
            (Some("session/update"), _) => self.aktualisierung(&v["params"]["update"]),
            (Some("session/request_permission"), Some(id)) => {
                self.erlaubnis_frage(id.clone(), &v["params"])
            }
            // Andere Anfragen des Agenten (Dateien, Terminals) bietet das Office nicht an.
            (Some(_), Some(id)) => {
                let id = id.clone();
                self.senden(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "nicht unterstützt" } }));
            }
            (None, Some(id)) => {
                let zweck = id.as_u64().and_then(|i| self.offen.remove(&i));
                self.antwort(zweck, &v);
            }
            _ => {}
        }
    }

    fn antwort(&mut self, zweck: Option<Zweck>, v: &Value) {
        let fehler = v.get("error").map(|e| text(e, "/message"));
        match (zweck, fehler) {
            (Some(Zweck::Init), None) => match self.fortsetzen.take() {
                Some(id) => {
                    let p = json!({ "sessionId": id, "cwd": self.cwd, "mcpServers": [] });
                    self.fortsetzen = Some(id);
                    self.anfrage("session/resume", p, Zweck::Fortsetzen);
                }
                None => self.anfrage(
                    "session/new",
                    json!({ "cwd": self.cwd, "mcpServers": [] }),
                    Zweck::Neu,
                ),
            },
            (Some(Zweck::Init), Some(f)) => self.zeigen(&format!(
                "\x1b[31m  Der Agent antwortet nicht auf initialize: {}\x1b[0m\r\n",
                sauber(&f)
            )),
            (Some(Zweck::Fortsetzen), None) => {
                let id = self.fortsetzen.take().unwrap_or_default();
                self.sitzung_da(id, v);
            }
            (Some(Zweck::Fortsetzen), Some(_)) => {
                self.zeigen("\x1b[2m  Die alte Sitzung ließ sich nicht fortsetzen – neue Sitzung\x1b[0m\r\n");
                self.anfrage(
                    "session/new",
                    json!({ "cwd": self.cwd, "mcpServers": [] }),
                    Zweck::Neu,
                );
            }
            (Some(Zweck::Neu), None) => {
                match v.pointer("/result/sessionId").and_then(Value::as_str) {
                    Some(id) => self.sitzung_da(id.to_string(), v),
                    None => self.zeigen("\x1b[31m  Keine Sitzung bekommen\x1b[0m\r\n"),
                }
            }
            (Some(Zweck::Neu), Some(f)) => self.zeigen(&format!(
                "\x1b[31m  Keine Sitzung: {}\x1b[0m\r\n",
                sauber(&f)
            )),
            (Some(Zweck::Auftrag), f) => {
                self.laeuft = false;
                let wie = match (f, text(v, "/result/stopReason").as_str()) {
                    (Some(f), _) => format!("\x1b[31m  Fehlgeschlagen: {}\x1b[0m", sauber(&f)),
                    (None, "cancelled") => "\x1b[33m  Abgebrochen\x1b[0m".into(),
                    _ => "\x1b[2m  Fertig\x1b[0m".into(),
                };
                self.zeigen(&format!("\r\n{wie}\r\n\x1b]9;4;0\x07"));
            }
            (Some(Zweck::Einstellung), Some(f)) => {
                self.zeigen(&format!(
                    "\x1b[33m  Einstellung abgelehnt: {}\x1b[0m\r\n",
                    sauber(&f)
                ));
            }
            (Some(Zweck::Einstellung), None) | (None, _) => {}
        }
    }

    fn aktualisierung(&mut self, u: &Value) {
        match u.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
            "agent_message_chunk" => {
                let t = sauber(&text(u, "/content/text"));
                self.zeigen(&t);
            }
            "agent_thought_chunk" => {
                let t = sauber(&text(u, "/content/text"));
                self.zeigen(&format!("\x1b[2;3m{t}\x1b[0m"));
            }
            "tool_call" => {
                let t = sauber(&text(u, "/title"));
                self.zeigen(&format!("\r\n\x1b[36m  ⏺ {t}\x1b[0m\r\n"));
            }
            "tool_call_update" if text(u, "/status") == "failed" => {
                let t = sauber(&text(u, "/title"));
                self.zeigen(&format!("\x1b[31m  ✗ {t} fehlgeschlagen\x1b[0m\r\n"));
            }
            _ => {}
        }
    }

    fn erlaubnis_frage(&mut self, id: Value, p: &Value) {
        let titel = sauber(&text(p, "/toolCall/title"));
        let optionen: Vec<(String, String)> = p
            .get("options")
            .and_then(Value::as_array)
            .map(|o| {
                o.iter()
                    .map(|o| (text(o, "/optionId"), text(o, "/name")))
                    .collect()
            })
            .unwrap_or_default();
        let mut t = format!("\x1b]9;4;4\x07\r\n\x1b[33m  Erlaubnis gefragt: {titel}\x1b[0m\r\n");
        for (i, (_, name)) in optionen.iter().enumerate() {
            t.push_str(&format!("    [{}] {}\r\n", i + 1, sauber(name)));
        }
        t.push_str("  Nummer eingeben und Enter (Esc lehnt ab): ");
        self.zeigen(&t);
        self.erlaubnis = Some((id, optionen.into_iter().map(|(o, _)| o).collect()));
    }

    fn erlaubnis_antwort(&mut self, wahl: Option<usize>) {
        let Some((id, optionen)) = self.erlaubnis.take() else {
            return;
        };
        let ergebnis = match wahl.and_then(|i| optionen.get(i)) {
            Some(o) => json!({ "outcome": { "outcome": "selected", "optionId": o } }),
            None => json!({ "outcome": { "outcome": "cancelled" } }),
        };
        self.senden(json!({ "jsonrpc": "2.0", "id": id, "result": ergebnis }));
        self.zeigen("\r\n\x1b]9;4;3\x07");
    }

    /// Was im Terminal getippt wird: eine Zeile, Enter schickt sie als Auftrag (oder als Antwort).
    fn eingabe(&mut self, d: &[u8]) {
        let t = String::from_utf8_lossy(d)
            .replace("\x1b[200~", "")
            .replace("\x1b[201~", "");
        let mut zeichen = t.chars().peekable();
        while let Some(c) = zeichen.next() {
            // Pfeil- und Funktionstasten (ESC [ … Buchstabe/~) gehören nicht in die Zeile.
            if c == '' && zeichen.peek() == Some(&'[') {
                for n in zeichen.by_ref().skip(1) {
                    if n.is_ascii_alphabetic() || n == '~' {
                        break;
                    }
                }
                continue;
            }
            match c {
                '\r' | '\n' => {
                    let zeile = std::mem::take(&mut self.zeile);
                    if self.erlaubnis.is_some() {
                        let wahl = zeile
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|n| n.checked_sub(1));
                        self.erlaubnis_antwort(wahl);
                    } else if self.laeuft {
                        self.zeigen(
                            "\r\n\x1b[2m  (Der Agent arbeitet noch – Esc bricht ab)\x1b[0m\r\n",
                        );
                    } else if !zeile.trim().is_empty() {
                        self.zeigen("\r\x1b[2K");
                        self.auftrag(zeile.trim());
                    }
                }
                '\x1b' => {
                    self.zeile.clear();
                    if self.erlaubnis.is_some() {
                        self.erlaubnis_antwort(None);
                    } else if let (true, Some(s)) = (self.laeuft, self.sitzung.clone()) {
                        self.senden(json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": s } }));
                    }
                }
                '\x7f' | '\x08' => {
                    if self.zeile.pop().is_some() {
                        self.zeigen("\x08 \x08");
                    }
                }
                c if !c.is_control() => {
                    self.zeile.push(c);
                    self.zeigen(&c.to_string());
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
