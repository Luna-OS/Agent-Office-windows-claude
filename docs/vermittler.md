# Der Vermittler: eine Lobby, ohne dass der Server rechnet

Zurück zur [README](../README.md) · [Connector](connector.md) · [Online-Lobby](online-lobby.md) · [Umbrel-App](umbrel.md)

Der **Vermittler** ist eine kleine Server-App in Rust (`crates/vermittler`), ohne Oberfläche. Er rechnet
nichts: Die Lobby mit ihren Etagen, Arbeitern, Terminals, dem Board und allem anderen läuft im Office
eines Spielers, dem **Gastgeber**, in seiner App auf seinem Computer. Der Vermittler reicht nur die
Verbindungen aller anderen zu ihm durch. Dafür reicht ein schwacher Server, etwa ein Umbrel oder ein
Raspberry Pi.

```
Browser von Kim ──┐
Browser von Mo  ──┼──▶ Vermittler (Umbrel, Port 4600) ══WebSocket══▶ Office von Luna (Gastgeber)
App von Kim     ──┘         reicht nur Bytes weiter                     Etagen, Arbeiter, Terminals
```

## Einrichten

1. **Den Vermittler starten**, auf einem Server, den alle erreichen (Heimnetz, Tailscale, NetBird):
   - **Umbrel:** die App „Agent Office Vermittler“ aus deinem Community-App-Store
     ([so kommt sie hinein](#umbrel-app)). Ihr Passwort steht auf ihrer Seite im App Store.
   - **Docker:** `docker run -d -p 4600:4600 -e VERMITTLER_PASSWORT=<passwort> ghcr.io/luna-os/agent-office-vermittler`
   - **Direkt:** `cargo run --release -p vermittler -- --passwort <passwort>` (Port 4600, siehe `--hilfe`).
2. **Gastgeber werden:** In der App **Agent Office** auf deinem Computer unter **🛰️ Über den Vermittler
   gastgeben** die Adresse des Vermittlers (z. B. `umbrel.local:4600` oder `100.78.107.162:4600`) und
   sein Passwort eintragen, dann **Gastgeber werden**. Die App startet dein Office neu, meldet es beim
   Vermittler an und zeigt, wie es dort steht (✅ Gastgeber). Ohne App geht es auch:
   `agent-office --vermittler http://umbrel.local:4600 --vermittler-passwort <passwort>`.
3. **Mitspieler einladen:** im Office unter **☰ → 👥 Team** einen Einladungslink machen. Er zeigt auf
   den Vermittler. Wer ihn bekommt, fügt ihn in seiner App unter **🌐 Lobby beitreten** ein, oder trägt
   dort einfach die Adresse des Vermittlers ein und meldet sich mit dem Passwort deines Office an.

Das Passwort des Vermittlers braucht nur, wer gastgeben will. Mitspieler melden sich im Office des
Gastgebers an, mit dessen Passwort, einem eigenen Konto oder einem Einladungslink.

Solange kein Gastgeber angemeldet ist, zeigt der Vermittler jedem Browser „Die Lobby ist gerade
geschlossen“. Melden sich mehrere Gastgeber an, ist der erste dran und die anderen warten als Ersatz;
geht er, übernimmt der nächste. Das ist dann aber dessen Office, mit dessen Etagen und Daten.

## Was wo läuft

| Wo | Was |
| --- | --- |
| Vermittler (Umbrel) | nur das Durchreichen: Verbindungen und WebSockets, Bytes in beide Richtungen |
| Office des Gastgebers | das Office: Etagen (Repos), Board, Warteschlange, Sprache, Dach, und die Arbeiter des Gastgebers |
| Arbeitsplatz jedes Spielers | seine Arbeiter (Claude Code & Co.) mit ihren Terminals und Worktrees, auf seinem PC |
| Browser der Spieler | das 3D-Office (three.js), wie immer |

## Dein Arbeitsplatz: deine Arbeiter auf deinem PC

Bist du mit einem **eigenen Konto** in einer Lobby (Einladungslink, oder ☰ → 👥 Team), startet die App im
Hintergrund dein **Arbeitsplatz**: ein eigenes Office nur auf deinem Computer (unter `<Daten>/arbeitsplatz`),
das sich bei der Lobby meldet. Stellst du dann einen Arbeiter ein, läuft er dort, auf deinem PC, mit **deinem**
Claude und GitHub (die Anmeldungen dieses Computers, die die App unter **🖥️ Deine Arbeiter auf diesem Computer**
zeigt). Beim ersten Arbeiter auf einer Etage klont dein Arbeitsplatz ihr Repo auf deinen PC.

Für alle anderen ist er ein Arbeiter wie jeder andere: Er sitzt am Schreibtisch, sein Laptop zeigt sein
Terminal, und jeder kann es öffnen, hineintippen, ihm Aufgaben geben oder ihn nach Hause schicken. Das geht
alles durch den Vermittler und das Office des Gastgebers zu deinem PC; beide rechnen dabei nichts.

- Ausschalten: **Lieber beim Gastgeber laufen lassen** in der App; dann laufen deine Arbeiter wieder dort,
  mit deinen Anmeldungen dort.
- Ist deine App aus, sind deine Arbeiter in der Lobby weg; startest du sie wieder, sind sie zurück.
- Mit dem Office-Passwort statt eines eigenen Kontos gibt es keinen Arbeitsplatz. Etagen ohne GitHub-Repo
  und Arbeiter über mehrere Repos laufen beim Gastgeber.
- Ohne App: `agent-office --arbeitsplatz http://umbrel.local:4600` mit deiner Sitzung in der Lobby in
  `AGENT_OFFICE_ARBEITSPLATZ_SITZUNG` (das Cookie `ao_session_…`).

Wie es funktioniert (`src/server/arbeitsplatz/`): Der Arbeitsplatz hält einen WebSocket zu
`<Lobby>/arbeitsplatz` (nur mit Konto). Will jemand mit Arbeitsplatz einen Arbeiter einstellen, reicht das
Office des Gastgebers die Nachricht dorthin weiter, ebenso alles für dessen Arbeiter (Terminal, Prompt,
Änderungen, nach Hause). Der Arbeitsplatz lässt es von seinen eigenen Handlern erledigen, als käme es von
Browsern bei ihm, und schickt zurück, was die ganze Etage sehen soll (`worker.update`, `worker.remove`,
`screen`, `toast`) und was nur für einen Browser ist (sein Terminal, Antworten). Der Gastgeber zeigt die
Arbeiter von außen auf seinen Etagen und lässt keinen Arbeitsplatz die eigenen Arbeiter oder die eines
anderen ändern.

## Umbrel-App

`deploy/umbrel/agent-office-vermittler` ist die App, `deploy/vermittler/Dockerfile` ihr Container (eine
Rust-Binärdatei, wenige MB). Der Workflow **Vermittler image** baut ihn für amd64 und arm64 und
veröffentlicht ihn auf main als `ghcr.io/luna-os/agent-office-vermittler:0.1.<Commits>`. In einen
Community-App-Store kommt die App mit:

```bash
git clone <dein App-Store>
deploy/umbrel/publish.sh <dein App-Store> --app agent-office-vermittler --pr
```

Die App läuft hinter dem App-Proxy von Umbrel auf Port 4600 (`VERMITTLER_HINTER_PROXY=1`: die Adresse
des Browsers kommt aus `X-Forwarded-For`). Sie speichert nichts.

## Wie es funktioniert

- **Anmelden:** Das Office des Gastgebers (`src/server/vermittler.ts`) öffnet einen WebSocket zu
  `/vermittler/knoten` und schickt `{"t":"hallo","version":1,"passwort":"…","name":"Luna"}`. Der
  Vermittler antwortet `{"t":"willkommen","version":1,"aktiv":true,"gastgeber":"Luna"}` oder
  `{"t":"abgelehnt","grund":"…"}`. Ein Ersatz bekommt später `{"t":"aktiv"}`, wenn er übernimmt.
- **Durchreichen:** Jede andere Verbindung zum Vermittler wird eine nummerierte Verbindung zum
  Gastgeber: `{"t":"auf","strom":7,"von":"100.64.0.9"}`, danach Binär-Nachrichten mit vier Bytes
  Nummer (big-endian) und den Bytes, in beide Richtungen, bis eine Seite `{"t":"zu","strom":7}` schickt.
  Das Office verbindet jede mit sich selbst (127.0.0.1), als käme sie direkt.
- **Der Anfragekopf:** Der Vermittler liest nur den ersten Kopf jeder Verbindung. `/vermittler/stand`
  (wer Gastgeber ist, als JSON) und `/vermittler/knoten` beantwortet er selbst, alles andere geht zum
  Gastgeber, mit `X-Forwarded-For` (von wo der Browser kommt; was der Browser selbst dort und in
  `X-Forwarded-Host`/`-Proto` schreibt, fliegt raus) und, außer bei WebSockets, `Connection: close`,
  damit jede Anfrage so einen Kopf hat. Das Office des Gastgebers läuft dafür mit `--trust-proxy`.
- **Grenzen:** höchstens 1024 Verbindungen gleichzeitig, 32 Gastgeber, Köpfe bis 64 KiB; eine
  Verbindung, die 15 Sekunden nichts abnimmt, wird beendet; ein Gastgeber, von dem 60 Sekunden nichts
  kommt (er bekommt alle 20 Sekunden ein Ping), ist weg.
- **Sicherheit:** Wer zum Vermittler kommt, steht vor dem Office des Gastgebers wie vor einem Office im
  Netz: Es fragt nach Passwort oder Konto. Das Passwort des Vermittlers wird in gleichbleibender Zeit
  geprüft, nach einem falschen wartet er eine Sekunde. HTTPS geht nicht durch den Vermittler (er muss die
  Köpfe lesen); für Sprache und Bildschirm teilen reicht die App den Vermittler über
  `http://127.0.0.1:48600` an den Browser weiter ([Connector](connector.md)).
