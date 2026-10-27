# Railway-Referenz

Die ganze Geschichte hinter `deploy/railway.sh`. Die Kurzfassung steht in der [README](../README.md#auf-railway-bereitstellen).

Du brauchst die **Railway CLI 5 oder neuer, angemeldet** (`railway login`; eine ältere aktualisierst du mit `railway upgrade` oder `brew upgrade railway`), dazu `ssh`, `curl`, Node.js (nur als kleines JSON-Hilfswerkzeug für die Antworten der Railway CLI, nicht für das Office) und einen Klon dieses Repos:

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
deploy/railway.sh up --claude-token "$(claude setup-token)"
```

Danach:

```bash
deploy/railway.sh open      # Tunnel zum Office und im Browser öffnen
deploy/railway.sh update    # diesen Checkout neu bauen und neu ausrollen
deploy/railway.sh destroy   # das Projekt, sein Volume und alles darauf löschen (fragt nach)
```

Was `up` tut, beim ersten Mal in etwa fünf Minuten:

1. Erzeugt ein SSH-Schlüsselpaar (liegt in `~/.config/open-agentic-office/railway/<name>/`).
2. Legt ein Railway-Projekt an, nach dem Office benannt, mit einem Dienst `office`. In mehr als einem Railway-Workspace? Wähle einen mit `--workspace <name>`.
3. Fügt ein **Volume auf `/data`** hinzu. Alles, was das Office behält, liegt darauf (siehe unten).
4. Fügt einen **TCP-Proxy** vor Port 22 des Containers hinzu, etwa `zephyr.proxy.rlwy.net:17738`. Das ist der einzige Weg hinein: Der Dienst bekommt nie eine HTTP-Domain, das Office selbst hängt also nie im Internet.
5. Baut diesen Checkout mit [`deploy/container/Dockerfile`](../deploy/container/Dockerfile): ein Rust-Multi-Stage-Build (Server mit cargo, Client mit trunk) und eine schlanke Laufzeit mit git, GitHub CLI, Build-Werkzeugen und sshd. **Claude Code** wird beim ersten Start des Office auf das Volume installiert, es aktualisiert sich also selbst. Die Neustart-Richtlinie ist *Always*, ein Absturz holt es sofort zurück.
6. Meldet die GitHub CLI mit deinem lokalen `gh auth token` an (liegt auf dem Volume) und gibt git deinen Namen und deine E-Mail.
7. Öffnet einen SSH-Tunnel und deinen Browser auf `http://localhost:4600`. **Die erste Seite zeigt das Office-Passwort einmalig. Schreib es auf.** Das Office behält danach nur einen Hash.

Das Office lauscht in seinem Container auf `127.0.0.1:4600`, und alle erreichen es durch den Tunnel. `localhost` gilt als sicherer Ursprung, also funktionieren Sprachchat und Bildschirm teilen, ohne Zertifikate. Dein Schlüssel meldet sich als `agentoffice` an, der Benutzer, unter dem das Office läuft (`deploy/railway.sh ssh` gibt dir seine Shell). Das Office startet ohne Etagen: Fahre mit dem Aufzug und wähle eines der Repositories, die dein GitHub-Token sieht.

**Was einen Neustart überlebt.** Railway ersetzt den Container bei jedem Neustart und jedem neuen Ausrollen; das Volume bleibt. Darauf liegt:

| Auf dem Volume | Was darin steckt |
| --- | --- |
| `/data/home/open-agentic-office` | Die eigenen Daten des Office: Passwort-Hash, Konten und Einladungen, Etagen, Chat, Einstellungen, Arcade-Rekorde, die eigenen Claude- und GitHub-Anmeldungen jedes Kontos |
| `/data/home/workspace` | Die Projekte, geklont als `<owner>/<repo>`, mit den Worktrees der Arbeiter |
| `/data/home/.local`, `.claude`, `.claude.json` | Claude Code selbst, seine Anmeldung und die Sitzungen, die Arbeiter fortsetzen |
| `/data/home/.config/gh`, `.gitconfig` | Die Anmeldung der GitHub CLI sowie Name und E-Mail für git |
| `/data/ssh` | Der SSH-Host-Schlüssel, damit ssh dem Office nach einem neuen Ausrollen weiter vertraut |
| `/data/team` | SSH-Schlüssel der Mitspieler, aus **👥 Mitspieler einladen** |

`/data/home` ist das Home-Verzeichnis von `agentoffice`, also liegt alles andere, was ein Arbeiter in `~` ablegt, ebenfalls auf dem Volume. Der Rest des Containers (der Office-Code in `/opt/open-agentic-office`, alles systemweit Installierte) kommt jedes Mal frisch aus dem Image. Ein Neustart oder neues Ausrollen hält laufende Arbeiter an; sie kommen an ihren Schreibtischen zurück, wenn das Office es tut.

**Dein Team einladen.** Mitspieler brauchen weder ein Railway-Konto noch dieses Repo, nur `ssh`. Klicke im Office im **☰**-Menü auf **👥 Mitspieler einladen** und tippe ihren GitHub-Benutzernamen, oder führe `deploy/railway.sh invite octocat` aus. In beiden Fällen bekommen sie einen Befehl wie:

```
ssh -L 4600:localhost:4600 ssh://office@zephyr.proxy.rlwy.net:17738
```

Sie lassen ihn laufen, öffnen http://localhost:4600 und melden sich an. Ihre Schlüssel melden sich als Benutzer `office` an, der **nur** zum Office-Port weiterleiten darf: keine Shell, keine anderen Ports, kein `-R`. Railways TCP-Proxy antwortet jeder IP, aber sshd nimmt nur deinen und eingeladene Schlüssel an, nie ein Passwort. Tunnel der **🌐 Dienste** zu den Webservern der Arbeiter funktionieren genauso (`deploy/railway.sh service 5173` für dich).

```bash
deploy/railway.sh open                 # Tunnel + Office im Browser öffnen
deploy/railway.sh service 5173         # den Webserver eines Arbeiters von der 🌐 Dienste-Tafel öffnen
deploy/railway.sh invite <gh-user>     # einen Mitspieler hineintunneln lassen (oder: invite <name> <key.pub>)
deploy/railway.sh uninvite <name>      # seine Schlüssel entfernen und offene Tunnel kappen
deploy/railway.sh team                 # wer eingeladen ist
deploy/railway.sh status               # Deployment, SSH-Adresse, Volume-Belegung, Office oben?, Team
deploy/railway.sh update               # diesen Checkout neu bauen und neu ausrollen
deploy/railway.sh restart              # den Container ohne Neubau neu starten
deploy/railway.sh reset-password       # neues Passwort, einmalig angezeigt; meldet alle ab
deploy/railway.sh ssh | logs           # eine Shell im Container / den Logs des Office folgen
```

**Aktualisieren.** Hole die neueste Fassung von OpenAgenticOffice in deinen Klon und führe `deploy/railway.sh update` aus. **⬆️ Office aktualisieren** im **☰**-Menü ist für Server, die mit `deploy/provision.sh` eingerichtet wurden: Der Code eines Containers kommt aus seinem Image.

**Claude-Anmeldung.** Gib `--claude-token "$(claude setup-token)"` an (dein Claude-Abo) oder `--anthropic-api-key <key>`; beides wird eine Variable des Dienstes. Oder gib keins an und führe `/login` im Terminal des ersten Arbeiters aus: Diese Anmeldung bleibt auf dem Volume.

**GitHub.** Standardmäßig meldet dein lokales `gh auth token` die GitHub CLI im Office an. Jeder, der das Office nutzen kann, kann dieses Token nutzen, gib also `--github-token <feingranulares Token>` oder `--no-github-token` an, wenn das zu viel ist. Codex und OpenCode sind nicht im Image: Trage sie in `deploy/container/Dockerfile` ein, wenn du sie nutzt.

**Weitere Einstellungen.** Das Office liest dieselben Umgebungsvariablen wie überall ([Konfiguration](configuration.md)). Setze sie am Dienst aus `~/.config/open-agentic-office/railway/<name>/`, das mit ihm verknüpft ist: `cd ~/.config/open-agentic-office/railway/open-agentic-office && railway variable set OFFICE_STADT="Portland, Oregon"`. Eine gesetzte Variable rollt das Office neu aus.

**Kosten.** Railway berechnet CPU und Speicher des Containers, solange er läuft, dazu das Volume. Nichts pausiert von selbst: Mit `deploy/railway.sh destroy` hörst du auf zu zahlen. Es löscht das Projekt mit Volume und allem darauf; das Office stoppt sofort, und Railway räumt das Projekt ein paar Tage später endgültig ab.

**Jeder andere Docker-Host.** Das Image ist nicht Railway-spezifisch: [`deploy/fly.sh`](fly.md) lässt es auf Fly.io laufen und [`deploy/dokploy.sh`](dokploy.md) auf Dokploy. Baue es aus dem Wurzelordner des Repos mit `docker build -f deploy/container/Dockerfile .`, hänge ein Volume auf `/data`, veröffentliche Port 22 und setze `OAO_ADMIN_KEYS` (dein öffentlicher Schlüssel), `OFFICE_BEANSPRUCH_TOKEN` (eine beliebige Zufallszeichenkette: öffne `/beanspruchen?t=<sie>` durch den Tunnel) und `OFFICE_OEFFENTLICHER_HOST` (`host:port`, zu dem Mitspieler per SSH verbinden).
