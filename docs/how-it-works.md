# Funktionsweise

Zurück zur [README](../README.md). Der [Code-Aufbau](code-layout.md) sagt, wo der Code liegt und wie sich ein Feature einhängt.

```
Browser (WASM: kiss3d + Leptos) ──HTTPS/WSS──▶ open-agentic-office (axum + tokio)
                         ├─ PTYs ─▶ claude / codex / opencode / … (ein PTY pro Arbeiter, cwd = Projektordner)
                         ├─ Hook-Server (nur 127.0.0.1) ◀── Hooks und `office-agenten` der Arbeiter (Token pro Arbeiter)
                         ├─ Nachrichten-Bus, Status-Feed, Aufgabenboard pro Etage
                         ├─ gh issue/pr list (zwischengespeichert, regelmäßig aktualisiert)
                         ├─ /medien: Bildschirm teilen über HTTP (JPEG-Frames rein, MJPEG raus)
                         ├─ YouTube-Dach: Server hält Video, Wiedergabe und Position
                         ├─ Port-Scan ─▶ 🔌 Dienste-Tafel
                         └─ WebRTC-Signalisierung (Sprache läuft von Browser zu Browser)
```

- **Protokoll.** Alles zwischen Browser und Server sind `ClientNachricht` / `ServerNachricht` aus dem Crate `protokoll`, über den WebSocket als JSON. Jede Nachricht ist intern getaggt (`domaene` + `nachricht`), und jede Domäne (Präsenz, Etagen, Konten, Einstellungen, Spielzeug, Agenten, Arbeiter, Aufgaben, GitHub, Medien, Dach) hat in ihrer Datei ein `Client`- und ein `Server`-Enum mit `#[serde(tag = "typ")]`. Client und Server benutzen dieselben Typen, ein Tippfehler im Protokoll ist ein Compilerfehler.
- **Status.** Claude-Arbeiter starten mit Hook-Einstellungen, die den Hook-Server auf `127.0.0.1` ansprechen; sie werden mit deinen eigenen Claude-Einstellungen zusammengeführt, nicht ersetzt. OpenCode, Codex, Grok, Gemini, Pi und Muse melden über ein generiertes Plugin bzw. eine Erweiterung oder einen Hook-Helfer an denselben Loopback-Server. Aus dem Status werden *arbeitet*, *braucht Eingabe* und *fertig*.
- **Agent-zu-Agent.** Der Hook-Server nimmt auch die Aufrufe der CLI `office-agenten` an (`wer`, `senden`, `posteingang`, `status`, `aufgaben`, `aufgabe-neu`, `aufgabe`). Jeder Arbeiter authentifiziert sich mit seinem eigenen Token (`OFFICE_HOOK_TOKEN`) und kann nur auf seiner Etage handeln. Der Server verteilt Nachrichten und Status an die Beteiligten und an die Browser der Menschen auf der Etage.
- **Gemeinsame Terminals.** Jeder Arbeiter hat ein PTY, gehalten vom Terminal-Wirt `office-pty-wirt`, einem eigenen kleinen Prozess neben dem Server (Port, PID und Token stehen in `pty-wirt.json` und `pty-wirt.token` im Datenordner). Der Wirt beendet sich, wenn 30 Minuten lang kein Server zurückkommt. Startet der Server neu, laufen die Agenten weiter, und der neue Server holt jedes Terminal zurück. Wer das Terminal öffnet, bekommt einen Schnappschuss des Bildschirms, dann den Live-Strom; der PTY nimmt die Größe dessen an, der gerade tippt. Verlauf und Chat werden gespeichert, damit nach einem Neustart alles zurückkommt.
- **Isolierte Branches.** Arbeiter mit Worktree bekommen `<projekt>/.office/worktrees/<name>-<id>` auf `office/<name>-<id>`. Kurz vor dem Anlegen holt das Office `git fetch origin <branch>` und verzweigt von `origin/<branch>`, damit seit dem letzten Pull gemergte PRs enthalten sind. Der Checkout des Projekts selbst wird nie bewegt.
- **Besprechungen.** Die Arbeiter einer Besprechung teilen sich einen eigenen Worktree auf einem Branch `office/besprechung-…`; ihre Notizen liegen im Ordner `.besprechung/` darin, wo die anderen sie lesen. Die Ausgabedatei wird am Ende auf diesem Branch committet.
- **Pull Requests.** **O** pusht den Branch des Arbeiters und führt `gh pr create` aus; die PR-Nummer wird beim Arbeiter gespeichert, damit die Tafel zum Schreibtisch zurückzeigen kann.
- **Änderungen.** Solange jemand das Änderungs-Fenster eines Schreibtischs offen hat, führt das Office alle zwei Sekunden `git status` und `git diff` im Checkout des Arbeiters aus. Der Diff geht gegen die Merge-Basis; Commit ist `git add -A && git commit`, Verwerfen `git restore`/`git clean`.
- **Bildschirm teilen.** Der Browser holt den Bildschirm per `getDisplayMedia`, kodiert einzelne JPEG-Frames (höchstens 2 MB) und schickt sie mit `POST /medien/bildschirm/bild`. Der Server hält das letzte Bild je Spieler (`GET /medien/bildschirm/<spieler>/bild`) und liefert es als MJPEG (`multipart/x-mixed-replace`, `GET /medien/bildschirm/<spieler>/stream`) an alle Zuschauer. Alle Endpunkte verlangen eine Sitzung.
- **YouTube-Dach.** Das Dach ist keine Etage, sondern ein Ort, an dem Leute sind, ohne Arbeiter, Tafeln oder Warteschlange. Der Server merkt sich pro Dach das Video (aus dem Link wird die ID gezogen), den Wiedergabezustand und die Position; Browser holen sich beim Beitritt den Stand und laufen von dort synchron weiter. Die Lautstärke wird im Browser aus der Entfernung zur Leinwand berechnet.
- **Sprache.** WebRTC-Mesh; die Signalisierung läuft über den WebSocket. STUN ist `stun.l.google.com:19302`, eigene TURN-Server stehen in `OFFICE_TURN`. Sprache und Bildschirm teilen brauchen HTTPS oder `localhost`.
- **Jukebox.** `jukebox.json` je Etage sagt nur, was läuft und seit wann; jeder Browser spielt es selbst. Ein Stück wird Note für Note aus Takt und Schritt entschieden, also reicht derselbe Startzeitpunkt, damit alle dasselbe hören.
- **Etagen.** `etagen.json` im Datenordner listet die Etagen. Hinzufügen führt `gh repo view` aus (sieht dieses Login das Repo, wie heißt es genau) und dann `gh repo clone` in den Projektordner; ein schon vorhandener Checkout desselben Repos wird übernommen. Der Klon läuft in einer eigenen Sitzung ohne Terminal, sodass nichts auf eine Frage wartet, die niemand sieht. Jede Etage führt ihre eigenen Arbeiter, GitHub-Tafeln und Warteschlange. Du bekommst nur Bewegungen, Arbeiter-Updates und Terminal-Frames deiner Etage; Chat, Sprache und Budget gehören dem Gebäude.
- **Dienste.** Alle 4 Sekunden listet das Office die TCP-Ports, auf denen Prozesse seines Benutzers lauschen, und schreibt jeden dem Arbeiter zu, dessen Terminal ihn gestartet hat.
- **Bilder.** WebGL darf ein fremdes Bild nur mit CORS-Header zeichnen. Also holt das Office jedes Bild selbst (`/api/image`, bis 15 MB) und liefert es von seiner eigenen Origin aus.
- **Zustand.** Der Datenordner (`~/open-agentic-office`) enthält Passwort-Hash, Sitzungsschlüssel, Konten und offene Einladungen (`konten.json`), die Anmeldungen jedes Kontos (`homes/<konto>/`), die Etagen (`etagen.json`) und pro Etage `etagen/<id>/` mit Arbeitern (`workers.json`), Warteschlange, Bildern, Hund, Jukebox und Whiteboard. Siehe [Konfiguration](configuration.md).

## Sicherheitshinweise

Wer sich anmelden kann, kann Claude Code, Codex, OpenCode, Gemini, Grok, Pi, Muse oder DeepSeek Harness in diesem Verzeichnis steuern und damit Befehle als der Benutzer ausführen, der das Office betreibt. Behandle Passwort, Konten und Einladungslinks wie SSH-Zugang:

- Das Office hört auf `127.0.0.1`, solange du nicht `--host` angibst, und `deploy/provision.sh` lässt es dort, hinter SSH, Caddy oder Tailscale Serve. Der Anmelde-Link, den das Terminal ausgibt, funktioniert nur einmal; das Office hält nur einen Hash davon, im Speicher.
- Nimm ein starkes Passwort und HTTPS. Mit `--proxy-vertrauen` sind Cookies `Secure`, sobald der Proxy sagt, die Anfrage kam über https. Aktiviere es nur hinter einem Proxy, der `X-Forwarded-For` anhängt (Caddy und nginx tun das).
- Ein geändertes gemeinsames Passwort meldet alle ab, die damit hereinkamen, weil ihre Sitzungen mit einem daraus abgeleiteten Schlüssel signiert sind. Konto-Sitzungen tragen die Konto-ID und werden bei jeder Anfrage geprüft: Ein entzogenes Konto oder abgeschaltetes Passwort meldet sofort ab, offene Verbindungen eingeschlossen. Passwörter werden als Hash gespeichert; Einladungslinks tragen ihr Token nach dem `#`, sodass es nie in einem Server-Log landet. Anmelde- und Einladungsversuche sind pro Client begrenzt.
- Betreibe das Office als eigenen, unprivilegierten Benutzer, im Projekt, das du teilen willst.
- Der WebSocket prüft das Sitzungs-Cookie und den `Origin`-Header. Der Hook-Server hört nur auf Loopback und verlangt einen zufälligen Token pro Arbeiter.
- Die Endpunkte unter `/medien` verlangen eine Sitzung; ein Frame darf höchstens 2 MB haben.
- Arbeiter erben weder das Office-Passwort noch Variablen übergeordneter Agenten-Sitzungen. Arbeiter eines Kontos laufen auf den Anmeldungen dieses Kontos und sehen nie die Zugangsdaten der Maschine. Aber jeder Arbeiter läuft als derselbe Betriebssystem-Benutzer, also *kann* der Arbeiter einer Person den Ordner einer anderen lesen: Getrennte Anmeldungen halten Arbeit zuordenbar und Pläne getrennt, nicht Geheimnisse voreinander.
- Der Bild-Abrufer (`/api/image`) verlangt eine Office-Sitzung. Bilder werden mit einer sandboxenden `Content-Security-Policy` ausgeliefert, damit eine SVG kein Skript in der Origin des Office ausführen kann.
