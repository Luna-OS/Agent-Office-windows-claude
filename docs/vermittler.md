# Agent Office Vermittler: der Server, auf dem euer Office läuft

Zurück zur [README](../README.md) · [Connector](connector.md) · [Zusammen spielen](online-lobby.md) · [Umbrel-App](umbrel.md)

**Agent Office Vermittler** ist die App auf eurem Server, meist eurem Umbrel. Auf ihr läuft das Office:
die Etagen (eure GitHub-Repos), das Board, die Warteschlange, der Chat, das Dach mit dem DJ. Es ist immer
offen, solange der Server läuft; keiner der Spieler muss gastgeben. Jeder verbindet sich in seiner App
**Agent Office** (Windows oder Mac) mit drei Angaben: der Adresse des Servers, seinem Passwort und dem
eigenen Namen. Mehr braucht es nicht, auch keine Einladungslinks.

Die Arbeiter (Claude Code & Co.) laufen nicht auf dem Server, sondern auf dem PC dessen, der sie einstellt,
mit seinem Claude-Abo: in seinem [Arbeitsplatz](#dein-arbeitsplatz-deine-arbeiter-auf-deinem-pc). Für alle
anderen sitzen sie trotzdem im Office am Schreibtisch, mit ihrem Terminal.

```
App + Browser von Kim ──┐                                        ┌── Arbeitsplatz von Kim (ihre Arbeiter)
App + Browser von Mo  ──┼──▶ Server (Umbrel, Port 4600): Office ◀─┼── Arbeitsplatz von Mo  (seine Arbeiter)
App + Browser von Luna ─┘    Etagen, Board, Chat, Konten          └── Arbeitsplatz von Luna (ihre Arbeiter)
```

## Einrichten

1. **Den Server starten**, so, dass alle ihn erreichen (Heimnetz, Tailscale, NetBird):
   - **Umbrel:** die App „Agent Office Vermittler“ aus deinem Community-App-Store
     ([so kommt sie hinein](#umbrel-app)). Ihr **Passwort** steht auf ihrer Seite im App Store: Das ist
     das Passwort des Servers.
   - **Docker:** `docker run -d -p 4600:4600 -v agent-office:/data -e AGENT_OFFICE_SSHD=off -e AGENT_OFFICE_HOST=0.0.0.0 -e AGENT_OFFICE_PASSWORD=<passwort> ghcr.io/luna-os/agent-office`
   - **Direkt:** `agent-office --host 0.0.0.0 --password <passwort>` (Port 4600).
2. **Verbinden:** In der App **Agent Office** die Adresse des Servers (z. B. `100.78.107.162:4600` oder
   `umbrel.local:4600`), das Passwort und deinen Namen eintragen, dann **Verbinden**. Beim ersten Mal legt
   der Server dein Konto unter diesem Namen an; mit demselben Namen bist du danach immer du. Die App merkt
   sich alles und verbindet sich beim nächsten Start von selbst ([Connector](connector.md)).
3. **Mitspieler:** Gib ihnen die Adresse und das Passwort. Jeder nimmt seinen eigenen Namen.
4. **Projekte:** Wer eine Etage hinzufügt (im Aufzug: Projekt hinzufügen), lässt den Server das Repo mit
   **seinem** GitHub klonen. Melde dich dafür im Connector unter **🔐 Deine Anmeldungen auf dem Server** bei
   GitHub an; ist dein Computer schon angemeldet, übernimmst du das mit einem Klick.

Gebt das Passwort nur Leuten, die mitspielen sollen: Wer es hat, kommt mit jedem Namen herein, als Admin.
Unter **☰ → 🔑 Accounts** sieht man, wer ein Konto hat, und kann Admins zu Mitspielern machen. Im Browser geht
es auch ohne App: Die Adresse des Servers öffnen und mit Name und Passwort anmelden (ohne Namen als Gast,
solange **Passwort ohne Namen** in 🔑 Accounts an ist).

### Vom alten Vermittler (0.1.34) umsteigen

Bis 0.1.34 rechnete die App „Agent Office Vermittler“ nichts und reichte nur zum Office eines Gastgebers
durch. Ein Update im App Store macht sie zum Server; ihr Passwort bleibt dasselbe. Etagen, die bisher im
Office des Gastgebers lagen, fügt ihr auf dem Server einmal neu hinzu. In der App (ab 0.1.63) verbindet ihr
euch wie oben; Lobbys, „Über den Vermittler gastgeben“ und Einladungslinks gibt es nicht mehr.

## Was wo läuft

| Wo | Was |
| --- | --- |
| Server (Umbrel, „Agent Office Vermittler“) | das Office: Etagen (Repos), Board, Warteschlange, Chat, Sprache, Dach, Jukebox, Konten |
| Arbeitsplatz jedes Spielers | seine Arbeiter (Claude Code & Co.) mit ihren Terminals und Worktrees, auf seinem PC |
| App jedes Spielers | die Verbindung zum Server, sein Arbeitsplatz, das Symbol in der Menüleiste |
| Browser der Spieler | das 3D-Office (three.js), wie immer |

## Dein Arbeitsplatz: deine Arbeiter auf deinem PC

Bist du verbunden, startet die App im Hintergrund dein **Arbeitsplatz**: ein eigenes Office nur auf deinem
Computer (unter `<Daten>/arbeitsplatz`), das sich beim Server meldet. Stellst du einen Arbeiter ein, läuft er
dort, auf deinem PC, mit **deinem** Claude und GitHub (die Anmeldungen dieses Computers, die die App unter
**🖥️ Deine Arbeiter auf diesem Computer** zeigt). Beim ersten Arbeiter auf einer Etage klont dein
Arbeitsplatz ihr Repo auf deinen PC.

Für alle anderen ist er ein Arbeiter wie jeder andere: Er sitzt am Schreibtisch, sein Laptop zeigt sein
Terminal, und jeder kann es öffnen, hineintippen, ihm Aufgaben geben oder ihn nach Hause schicken. Das geht
alles über den Server zu deinem PC.

Und die Arbeiter reden miteinander, über die PCs hinweg: Deine sehen mit `list_workers` auch die auf den PCs
der anderen (mit `pc`, wo sie laufen), und `tell_worker` erreicht jeden davon. Die Nachricht kommt mit
Absender an, damit er antworten kann, und alle im Office sehen das Gespräch: Sprechblase, Papierflieger, eine
Zeile im Chat. Damit man jeden ansprechen kann, bekommt ein neuer Arbeiter auf deinem PC einen Namen, den es
auf der Etage noch nicht gibt.

- Ausschalten: **Lieber auf dem Server laufen lassen** in der App; dann laufen deine Arbeiter auf dem Server,
  mit deinen Anmeldungen dort.
- Ist deine App aus (oder dein PC, oder dein Netz weg), bleiben deine Arbeiter im Office an ihren Plätzen
  sitzen und machen **Pause**: mit einem Becher Kaffee, am Handy oder mit einem Donut. Ihre Plätze bleiben
  belegt, und wer sie anspricht, erfährt, dass dein PC gerade aus ist. Startest du die App wieder, ist die Pause
  vorbei, und mit **R** machen sie weiter, wo sie waren. Schickt jemand einen in der Pause nach Hause, geht er aus
  dem Office; auf deinem PC ist er noch und kommt mit ihm zurück. Schließt du nur das Fenster der App, läuft sie
  in der Menüleiste weiter, und deine Arbeiter arbeiten weiter.
- Etagen ohne GitHub-Repo und Arbeiter über mehrere Repos laufen auf dem Server.
- Ohne App: `agent-office --arbeitsplatz http://umbrel.local:4600` mit deiner Sitzung auf dem Server in
  `AGENT_OFFICE_ARBEITSPLATZ_SITZUNG` (das Cookie `ao_session_…`).

Wie es funktioniert (`src/server/arbeitsplatz/`): Der Arbeitsplatz hält einen WebSocket zu
`<Server>/arbeitsplatz` (nur mit Konto). Will jemand mit Arbeitsplatz einen Arbeiter einstellen, reicht das
Office auf dem Server die Nachricht dorthin weiter, ebenso alles für dessen Arbeiter (Terminal, Prompt,
Änderungen, nach Hause). Der Arbeitsplatz lässt es von seinen eigenen Handlern erledigen, als käme es von
Browsern bei ihm, und schickt zurück, was die ganze Etage sehen soll (`worker.update`, `worker.remove`,
`screen`, `toast`) und was nur für einen Browser ist (sein Terminal, Antworten). Der Server zeigt die
Arbeiter von außen auf seinen Etagen und lässt keinen Arbeitsplatz die Arbeiter eines anderen ändern.

Der Server schickt jedem Arbeitsplatz alle 20 Sekunden ein Ping. Geht die Verbindung zu, oder kommt bis zum
nächsten Ping keine Antwort, merkt er sich dessen Arbeiter in `arbeitsplatz-pausen.json` (in seinen Daten,
auch über einen Neustart) und zeigt sie in der Pause (`WorkerInfo.pause`). Meldet sich der Arbeitsplatz wieder,
schickt er erst seine Arbeiter und dann ihre Liste (`bestand`); wer dort fehlt, geht. Meldet sich derselbe
Spieler von einem zweiten Computer an, ist der neue sein Arbeitsplatz; der alte drängelt sich nicht zurück,
und was nur auf dem alten läuft, macht Pause.

Reden (`src/server/arbeitsplatz/reden.ts`, `src/server/gespraech.ts`): Fragt ein Arbeiter auf einem Arbeitsplatz,
wer auf der Etage sitzt, oder will er einem etwas sagen, der nicht bei ihm läuft, fragt sein Office den Server
(`wer`, `sagen`); der antwortet (`die`, `gesagt`) und reicht eine Nachricht an einen anderen Arbeitsplatz als
`worker.prompt` weiter. Den Absender nimmt der Server aus seinen eigenen Büchern, kein Arbeitsplatz spricht für
einen anderen. Reden zwei Arbeiter auf demselben Arbeitsplatz, sieht das Office es über `worker.says`.

## Anmelden mit Name und Passwort

`POST /api/login` mit `{ "name": "Luna", "password": "<Passwort des Servers>" }` meldet als das Konto
**Luna** an; gibt es das noch nicht, legt der Server es an, als Admin (`Accounts.ensure` in
`src/server/accounts.ts`). Ein Name ohne Buchstaben geht nicht, und Groß- und Kleinschreibung zählt nicht
(„luna“ ist Luna). Ohne Namen meldet das Passwort als Gast ohne eigenes Konto an, solange
**Passwort ohne Namen** an ist (`agent-office accounts password on|off`). Wer ein Konto löscht, löscht es
nur bis zum nächsten Anmelden mit diesem Namen: Wer das Passwort kennt, kommt wieder herein.

## Umbrel-App

`deploy/umbrel/agent-office-vermittler` ist die App: der Container aus `deploy/container/Dockerfile` (das
Office, wie `agent-office`), ohne sshd, hinter dem App-Proxy von Umbrel auf Port 4600, das Passwort aus
`APP_PASSWORD`, alle Daten im Daten-Ordner der App. Der Workflow **Umbrel image** baut den Container für
amd64 und arm64 und veröffentlicht ihn auf main als `ghcr.io/luna-os/agent-office:0.1.<Commits>`. In einen
Community-App-Store kommt die App mit:

```bash
git clone <dein App-Store>
deploy/umbrel/publish.sh <dein App-Store> --pr
```

Mehr dazu in [umbrel.md](umbrel.md).
