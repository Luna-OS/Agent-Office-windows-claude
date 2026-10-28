//! Tests der ACP-Anbindung gegen einen nachgebauten Agenten (diese Testdatei startet sich selbst).
use super::*;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Spielt einen ACP-Agenten, wenn der Test sich selbst damit startet (`FALSCHER_ACP=1`).
#[test]
fn falscher_agent() {
    if std::env::var("FALSCHER_ACP").is_err() {
        return;
    }
    let mut aus = std::io::stdout();
    let mut zeilen = BufReader::new(std::io::stdin()).lines();
    let mut senden = |v: Value| {
        let _ = writeln!(aus, "{v}");
        let _ = aus.flush();
    };
    while let Some(Ok(z)) = zeilen.next() {
        let Ok(v) = serde_json::from_str::<Value>(&z) else {
            continue;
        };
        let id = v["id"].clone();
        match v["method"].as_str() {
            Some("initialize") => senden(json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
            Some("session/new") => {
                let modelle = json!([{ "id": "model", "options": [{ "group": "ds", "options": [{ "value": "[\"ds\",\"deepseek-v9\"]", "name": "DeepSeek V9" }] }] }]);
                senden(
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "sessionId": "s1", "configOptions": modelle } }),
                )
            }
            Some("session/set_config_option") => {
                let upd = json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": format!("eingestellt: {}\n", v["params"]["value"].as_str().unwrap_or("")) } });
                senden(
                    json!({ "jsonrpc": "2.0", "method": "session/update", "params": { "sessionId": "s1", "update": upd } }),
                );
                senden(json!({ "jsonrpc": "2.0", "id": id, "result": {} }))
            }
            Some("session/prompt") => {
                let upd = json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Hallo zurück\n" } });
                senden(
                    json!({ "jsonrpc": "2.0", "method": "session/update", "params": { "sessionId": "s1", "update": upd } }),
                );
                let frage = json!({ "sessionId": "s1", "toolCall": { "title": "Datei schreiben" }, "options": [{ "optionId": "ja", "name": "Erlauben" }, { "optionId": "nein", "name": "Ablehnen" }] });
                senden(
                    json!({ "jsonrpc": "2.0", "id": "p1", "method": "session/request_permission", "params": frage }),
                );
                // Auf die Antwort warten.
                for z in zeilen.by_ref().map_while(Result::ok) {
                    let a: Value = serde_json::from_str(&z).unwrap_or(Value::Null);
                    if a["id"] == "p1" {
                        let gewaehlt = a["result"]["outcome"]["optionId"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let upd = json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": format!("gewählt: {gewaehlt}\n") } });
                        senden(
                            json!({ "jsonrpc": "2.0", "method": "session/update", "params": { "sessionId": "s1", "update": upd } }),
                        );
                        break;
                    }
                }
                senden(
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "stopReason": "end_turn" } }),
                );
            }
            _ => {}
        }
    }
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
}

#[test]
fn auftrag_erlaubnis_und_ende() {
    let befehl = Befehl {
        programm: std::env::current_exe().unwrap(),
        argumente: vec![
            "--exact".into(),
            "acp::tests::falscher_agent".into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ],
        verzeichnis: std::env::temp_dir(),
        umgebung: [("FALSCHER_ACP".to_string(), "1".to_string())].into(),
        entfernen: vec![],
    };
    let aus = Arc::new(Mutex::new(String::new()));
    let (ende_tx, ende_rx) = mpsc::channel();
    let a2 = aus.clone();
    let wahl = Wahl {
        modell: Some("deepseek-v9".into()),
        aufwand: Some("max".into()),
    };
    let pty = starten(
        &befehl,
        wahl,
        Some("Hallo".into()),
        None,
        move |d, _| a2.lock().unwrap().push_str(&String::from_utf8_lossy(d)),
        move |c| {
            let _ = ende_tx.send(c);
        },
    )
    .unwrap();
    let warte = |t: &str| {
        for _ in 0..100 {
            if aus.lock().unwrap().contains(t) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("„{t}“ kam nicht: {}", aus.lock().unwrap());
    };
    warte("Erlaubnis gefragt: Datei schreiben");
    let text = aus.lock().unwrap().clone();
    assert!(text.contains("\x1b]7700;sitzung;s1\x07"));
    assert!(text.contains("Hallo zurück\r\n"));
    assert!(text.contains("\x1b]9;4;4\x07"));
    pty.schreiben(b"1\r");
    warte("Fertig");
    let text = aus.lock().unwrap().clone();
    assert!(text.contains("gewählt: ja"));
    assert!(
        text.contains(r#"eingestellt: ["ds","deepseek-v9"]"#),
        "{text}"
    );
    assert!(text.contains("kein reasoning_effort"));
    pty.beenden();
    assert!(ende_rx.recv_timeout(Duration::from_secs(10)).is_ok());
}
