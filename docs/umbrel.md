# Umbrel-App: was sie ist und wie sie in deinen App Store kommt

Zurück zur [README](../README.md) · [Server](vermittler.md) · [Zusammen spielen](online-lobby.md)

Die Umbrel-App „Agent Office Vermittler“ ist der [Server](vermittler.md), auf dem euer Office läuft, auf
einem [Umbrel](https://umbrel.com): immer offen, ohne Gastgeber. Alle Spieler verbinden sich mit dem
[Connector](connector.md) für Windows oder macOS (Adresse, Passwort, Name) oder im Browser; ihre
Arbeiter laufen auf ihren eigenen PCs. Diese Seite beschreibt, woraus die App besteht, wie sie in einen
Community App Store (wie deinen nrzStore) kommt und wie du sie aktualisierst.

## Woraus sie besteht

- **Das Image** `ghcr.io/luna-os/agent-office`: derselbe Container wie für Railway, Fly.io und
  Dokploy ([`deploy/container/Dockerfile`](../deploy/container/Dockerfile)), für **amd64** (Umbrel
  Home, PCs) und **arm64** (Raspberry Pi). Der Workflow
  [`.github/workflows/umbrel-image.yml`](../.github/workflows/umbrel-image.yml) baut es bei jedem
  Push auf `main` auf je einem Runner der passenden Architektur und veröffentlicht es als
  `0.1.<Commits auf main>` und `latest`. Pull Requests bauen es nur und starten es einmal so, wie
  Umbrel es startet.
- **Die App-Dateien** in [`deploy/umbrel/agent-office-vermittler/`](../deploy/umbrel/agent-office-vermittler):
  - `umbrel-app.yml`: Name, Beschreibung, Port **4600**, Icon, und `deterministicPassword: true`,
    damit Umbrel ein festes Passwort erzeugt und auf der Seite der App zeigt.
  - `docker-compose.yml`: Umbrels `app_proxy` vor dem Office (mit `PROXY_AUTH_ADD: "false"`: das
    Office meldet die Leute selbst an, mit ihrem Namen und seinem Passwort, damit Mitspieler nicht
    das Umbrel-Passwort brauchen) und der Container mit
    - `AGENT_OFFICE_SSHD=off`: kein SSH-Server (die Tunnel der anderen Deployments braucht es hier nicht),
    - `AGENT_OFFICE_HOST=0.0.0.0`: der Proxy erreicht das Office im Container,
    - `AGENT_OFFICE_TRUST_PROXY=1`: das Office liest die Adresse der Spieler aus den
      `X-Forwarded-*`-Headern des Proxys,
    - `AGENT_OFFICE_PASSWORD=${APP_PASSWORD}`: Umbrels Passwort der App ist das Passwort des Servers,
    - `${APP_DATA_DIR}/data` als `/data`: alles, was das Office sich merkt.

## In den Store legen: `deploy/umbrel/publish.sh`

Ein Umbrel Community App Store ist ein Git-Repository mit einer `umbrel-app-store.yml` (mit `id:`
und `name:`) und einem Ordner pro App, dessen Name mit der Store-ID beginnt. Das Skript erledigt das:

```bash
git clone http://100.78.107.162:8101/nicorenezweers/nrzStore
deploy/umbrel/publish.sh nrzStore --pr
```

Es

1. liest die Store-ID aus `nrzStore/umbrel-app-store.yml` (gibt es die Datei noch nicht, legt
   `--store-id <id>` sie an),
2. findet das neueste `x.y.z`-Image auf ghcr.io und seinen Digest,
3. schreibt `nrzStore/<id>-agent-office-vermittler/` mit dieser ID, dem Image (per Digest festgenagelt)
   und seiner Version als App-Version, und
4. mit `--pr`: committet auf einen neuen Branch `agent-office-vermittler-<version>`, pusht ihn und zeigt
   den Link, der den Pull Request öffnet (`<store>/compare/<branch>...agent-office-vermittler-<version>`,
   so bei Gitea wie bei GitHub). Mit `--push` stattdessen direkt auf den aktuellen Branch.

Weitere Optionen: `--tag 0.1.42` (eine bestimmte Version), `--port 4601` (falls 4600 auf deinem
Umbrel schon belegt ist), `--image` (ein anderes Image). `deploy/umbrel/publish.sh --help` listet sie.

Auf dem Umbrel fügst du den Store einmal hinzu: **App Store → ⋯ → Community App Stores**, Adresse
des Stores eintragen. Danach steht **Agent Office Vermittler** darin.

> Das Paket auf ghcr.io muss **öffentlich** sein, damit der Umbrel es ohne Anmeldung laden kann. Ist
> es nach dem ersten Workflow-Lauf privat: auf GitHub unter *Packages → agent-office → Package
> settings → Change visibility* auf *Public* stellen.

## Aktualisieren

Jeder Push auf `main` baut ein neues Image. Dann einfach noch einmal (im aktualisierten Klon des Stores):

```bash
git -C nrzStore pull
deploy/umbrel/publish.sh nrzStore --pr
```

Die App-Version steigt mit, und der Umbrel bietet das Update an. Daten, Konten, Projekte und
Anmeldungen bleiben im Daten-Ordner der App erhalten. Bis 0.1.34 war die App nur ein Vermittler, der zum
Office eines Gastgebers durchreichte; das Update macht sie zum Server, mit demselben Passwort
([umsteigen](vermittler.md#vom-alten-vermittler-0134-umsteigen)).

## Was im Daten-Ordner liegt

Wie bei den anderen Container-Deployments (siehe [`deploy/container/start.sh`](../deploy/container/start.sh)):

- `data/home/agent-office/`: das Office (Konten, Etagen, Board, Chat, die Anmeldungen jedes Kontos
  in `.agent-office/homes/<konto>/`)
- `data/home/workspace/`: die geklonten Projekte
- `data/home/.local`, `.claude`, `.claude.json`: Claude Code und die Claude-Anmeldung des Office
- `data/home/.config/gh`, `.gitconfig`: die GitHub-Anmeldung des Office

## Selbst ausprobieren, ohne Umbrel

```bash
docker run -d -p 4600:4600 -v "$PWD/office-data:/data" \
  -e AGENT_OFFICE_SSHD=off -e AGENT_OFFICE_HOST=0.0.0.0 -e AGENT_OFFICE_TRUST_PROXY=1 \
  -e AGENT_OFFICE_PASSWORD=geheim ghcr.io/luna-os/agent-office:latest
```

Dann im Connector unter **🛰️ Server** `localhost:4600`, `geheim` und deinen Namen eintragen, oder
`http://localhost:4600` im Browser öffnen.
