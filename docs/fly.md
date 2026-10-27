# Fly.io-Referenz

Die ganze Geschichte hinter `deploy/fly.sh`. Die Kurzfassung steht in der [README](../README.md#auf-flyio-bereitstellen).

Du brauchst ein aktuelles **flyctl, angemeldet** (`fly auth login`; [installieren](https://fly.io/docs/flyctl/install/) mit `brew install flyctl` oder `curl -L https://fly.io/install.sh | sh`, ein altes aktualisierst du mit `fly version upgrade`), dazu `ssh`, `curl`, Node.js (nur als kleines JSON-Hilfswerkzeug für die Antworten von flyctl, nicht für das Office) und einen Klon dieses Repos:

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
deploy/fly.sh up --claude-token "$(claude setup-token)"
```

Danach:

```bash
deploy/fly.sh open      # Tunnel zum Office und im Browser öffnen
deploy/fly.sh update    # diesen Checkout neu bauen und neu ausrollen
deploy/fly.sh destroy   # die App, ihr Volume und alles darauf löschen (fragt nach)
```

Was `up` tut, beim ersten Mal in ein paar Minuten:

1. Erzeugt ein SSH-Schlüsselpaar (liegt in `~/.config/open-agentic-office/fly/<name>/`).
2. Legt eine Fly-App an, nach dem Office benannt, mit zufälligem Anhang, etwa `open-agentic-office-3f9a1c` (App-Namen sind auf ganz Fly.io eindeutig; wähle deinen mit `--app`). In mehr als einer Fly-Organisation? Wähle eine mit `--org <slug>`.
3. Fügt ein **50-GB-Volume auf `/data`** hinzu (`--disk` für eine andere Größe), in der Fly-Region, die dir am nächsten ist (`--region` für eine andere; `fly platform regions` listet sie). Alles, was das Office behält, liegt darauf (siehe unten).
4. Gibt der App eine **dedizierte IPv4-Adresse**, mit SSH auf einem zufälligen Port, etwa `137.66.12.34:31337`. Das ist der einzige Weg hinein. Die App hat keinen HTTP-Dienst, unter `<app>.fly.dev` antwortet also nichts, und das Office selbst hängt nie im Internet. Die Adresse ist dediziert, weil Flys geteilte IPv4-Adressen nur HTTP und TLS tragen.
5. Baut diesen Checkout mit [`deploy/container/Dockerfile`](../deploy/container/Dockerfile) auf Flys Remote-Builder: dasselbe Image wie auf [Railway](railway.md), ein Rust-Multi-Stage-Build (Server mit cargo, Client mit trunk), und die Laufzeit mit git, GitHub CLI, Build-Werkzeugen und sshd. **Claude Code** wird beim ersten Start des Office auf das Volume installiert, es aktualisiert sich also selbst. Es läuft auf einer Maschine, einer `shared-cpu-4x` mit 8 GB (`--vm-size` und `--memory` ändern das). Fly hält sie nie wegen Leerlauf an, und ihre Neustart-Richtlinie ist *always*, ein Absturz holt sie sofort zurück.
6. Meldet die GitHub CLI mit deinem lokalen `gh auth token` an (liegt auf dem Volume) und gibt git deinen Namen und deine E-Mail.
7. Öffnet einen SSH-Tunnel und deinen Browser auf `http://localhost:4600`. **Die erste Seite zeigt das Office-Passwort einmalig. Schreib es auf.** Das Office behält danach nur einen Hash.

Das Office lauscht in seiner Maschine auf `127.0.0.1:4600`, und alle erreichen es durch den Tunnel. `localhost` gilt als sicherer Ursprung, also funktionieren Sprachchat und Bildschirm teilen, ohne Zertifikate. Dein Schlüssel meldet sich als `agentoffice` an, der Benutzer, unter dem das Office läuft (`deploy/fly.sh ssh` gibt dir seine Shell). Wäre der sshd des Office je aus, bringt dich `fly ssh console -a <app>` über Flys eigenes SSH trotzdem in eine root-Shell, das Port 22 in der Maschine behält (der sshd des Office lauscht dort auf 2222, und der öffentliche Port der App führt dorthin). Das Office startet ohne Etagen: Fahre mit dem Aufzug und wähle eines der Repositories, die dein GitHub-Token sieht.

**Was einen Neustart überlebt.** Ein Neustart, neues Ausrollen oder Größenänderung startet die Maschine aus einer frischen Kopie des Images; das Volume bleibt. Darauf liegt:

| Auf dem Volume | Was darin steckt |
| --- | --- |
| `/data/home/open-agentic-office` | Die eigenen Daten des Office: Passwort-Hash, Konten und Einladungen, Etagen, Chat, Einstellungen, Arcade-Rekorde, die eigenen Claude- und GitHub-Anmeldungen jedes Kontos |
| `/data/home/workspace` | Die Projekte, geklont als `<owner>/<repo>`, mit den Worktrees der Arbeiter |
| `/data/home/.local`, `.claude`, `.claude.json` | Claude Code selbst, seine Anmeldung und die Sitzungen, die Arbeiter fortsetzen |
| `/data/home/.config/gh`, `.gitconfig` | Die Anmeldung der GitHub CLI sowie Name und E-Mail für git |
| `/data/ssh` | Der SSH-Host-Schlüssel, damit ssh dem Office nach einem neuen Ausrollen weiter vertraut |
| `/data/team` | SSH-Schlüssel der Mitspieler, aus **👥 Mitspieler einladen** |

`/data/home` ist das Home-Verzeichnis von `agentoffice`, also liegt alles andere, was ein Arbeiter in `~` ablegt, ebenfalls auf dem Volume. Der Rest der Maschine (der Office-Code in `/opt/open-agentic-office`, alles systemweit Installierte) kommt jedes Mal frisch aus dem Image. Ein Neustart oder neues Ausrollen hält laufende Arbeiter an; sie kommen an ihren Schreibtischen zurück, wenn das Office es tut. Fly macht außerdem täglich einen Snapshot des Volumes und behält die der letzten fünf Tage (`fly volumes snapshots list -a <app> <volume-id>`; `fly volumes list -a <app>` zeigt die ID).

**Dein Team einladen.** Mitspieler brauchen weder ein Fly-Konto noch dieses Repo, nur `ssh`. Klicke im Office im **☰**-Menü auf **👥 Mitspieler einladen** und tippe ihren GitHub-Benutzernamen, oder führe `deploy/fly.sh invite octocat` aus. In beiden Fällen bekommen sie einen Befehl wie:

```
ssh -L 4600:localhost:4600 ssh://office@137.66.12.34:31337
```

Sie lassen ihn laufen, öffnen http://localhost:4600 und melden sich an. Ihre Schlüssel melden sich als Benutzer `office` an, der **nur** zum Office-Port weiterleiten darf: keine Shell, keine anderen Ports, kein `-R`. Die Adresse antwortet jeder IP, aber sshd nimmt nur deinen und eingeladene Schlüssel an, nie ein Passwort. Tunnel der **🌐 Dienste** zu den Webservern der Arbeiter funktionieren genauso (`deploy/fly.sh service 5173` für dich).

```bash
deploy/fly.sh open                    # Tunnel + Office im Browser öffnen
deploy/fly.sh service 5173            # den Webserver eines Arbeiters von der 🌐 Dienste-Tafel öffnen
deploy/fly.sh invite <gh-user>        # einen Mitspieler hineintunneln lassen (oder: invite <name> <key.pub>)
deploy/fly.sh uninvite <name>         # seine Schlüssel entfernen und offene Tunnel kappen
deploy/fly.sh team                    # wer eingeladen ist
deploy/fly.sh status                  # Maschine, SSH-Adresse, Volume-Belegung, Office oben?, Team
deploy/fly.sh update                  # diesen Checkout neu bauen und neu ausrollen
deploy/fly.sh restart                 # die Maschine ohne Neubau neu starten
deploy/fly.sh resize performance-2x   # andere Maschinengröße (Speicher: resize <größe> 16gb)
deploy/fly.sh pause                   # die Maschine anhalten, und damit das Bezahlen ihrer CPU und ihres Speichers
deploy/fly.sh resume                  # wieder starten und öffnen
deploy/fly.sh reset-password          # neues Passwort, einmalig angezeigt; meldet alle ab
deploy/fly.sh ssh | logs              # eine Shell in der Maschine / den Logs des Office folgen
```

**Aktualisieren.** Hole die neueste Fassung von OpenAgenticOffice in deinen Klon und führe `deploy/fly.sh update` aus. **⬆️ Office aktualisieren** im **☰**-Menü ist für Server, die mit `deploy/provision.sh` eingerichtet wurden: Der Code einer Fly-Maschine kommt aus ihrem Image.

**Maschinengröße.** Eine `shared-cpu-4x` mit 8 GB reicht für ein paar Arbeiter. Geteilte CPUs sind aber für Lastspitzen gemacht, und Fly bremst eine, die lange ausgelastet bleibt. Bauen und testen die Arbeiter den ganzen Tag, wechsle auf dedizierte CPUs: `deploy/fly.sh resize performance-2x` (der Speicher bleibt, außer du fügst welchen hinzu: `resize performance-4x 16gb`). `fly platform vm-sizes` listet die Größen. Das Volume kann wachsen, nie schrumpfen: `fly volumes extend -a <app> <volume-id> -s 100`.

**Pausieren.** `deploy/fly.sh pause` hält die Maschine an, und die Arbeiter mit ihr. Fly berechnet dann nur das Volume, die IP-Adresse und etwas Speicher für die angehaltene Maschine. Nur `deploy/fly.sh resume` startet sie wieder, und öffnet auch das Office.

**Claude-Anmeldung.** Gib `--claude-token "$(claude setup-token)"` an (dein Claude-Abo) oder `--anthropic-api-key <key>`; beides wird ein Secret der App. Oder gib keins an und führe `/login` im Terminal des ersten Arbeiters aus: Diese Anmeldung bleibt auf dem Volume.

**GitHub.** Standardmäßig meldet dein lokales `gh auth token` die GitHub CLI im Office an. Jeder, der das Office nutzen kann, kann dieses Token nutzen, gib also `--github-token <feingranulares Token>` oder `--no-github-token` an, wenn das zu viel ist. Codex und OpenCode sind nicht im Image: Trage sie in `deploy/container/Dockerfile` ein, wenn du sie nutzt.

**Weitere Einstellungen.** Das Office liest dieselben Umgebungsvariablen wie überall ([Konfiguration](configuration.md)). Setze sie als Secrets der App: `fly secrets set -a <app> OFFICE_STADT="Portland, Oregon"`. Das startet die Maschine neu. Bearbeite nicht die `fly.toml` in `~/.config/open-agentic-office/fly/<name>/`, denn `deploy/fly.sh` schreibt sie vor jedem Ausrollen neu.

**Kosten.** Fly berechnet die Maschine sekundengenau, solange sie läuft, dazu das Volume, die dedizierte IPv4-Adresse und die Snapshots des Volumes. Nichts pausiert von selbst: `deploy/fly.sh pause` stoppt den Anteil der Maschine, `deploy/fly.sh destroy` alles. Das Löschen entfernt die App samt Maschine, Volume und Adresse und allem darauf.

**Jeder andere Docker-Host.** Siehe die [Hinweise zu Railway](railway.md): Das Image ist dasselbe.
