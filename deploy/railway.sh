#!/usr/bin/env bash
# Dein eigenes OpenAgenticOffice mit einem Befehl auf Railway bereitstellen, mit der Railway CLI.
#
#   deploy/railway.sh up        Projekt anlegen, das Office bauen und starten, im Browser öffnen
#   deploy/railway.sh open      Tunnel zum Office aufbauen und im Browser öffnen
#   deploy/railway.sh destroy   Projekt, Volume und alles darauf löschen (fragt vorher)
#
# Das Office ist nie im Internet: Es hört im Container auf 127.0.0.1, und alle erreichen es über einen
# SSH-Tunnel zum sshd des Containers, hinter Railways TCP-Proxy. Was es aufbewahrt (Konten, Etagen,
# Projekte, Anmeldungen, Schlüssel der Teammitglieder, der SSH-Hostschlüssel) liegt auf einem Volume
# unter /data, sodass Neustarts und erneute Deployments nichts davon verlieren. Das Image ist
# deploy/container/Dockerfile, gebaut aus diesem Checkout. `deploy/railway.sh help` zeigt alle
# Befehle und Optionen.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NAME="open-agentic-office"
WORKSPACE=""
GH_TOKEN_ARG=""
NO_GH_TOKEN=0
CLAUDE_TOKEN="${CLAUDE_CODE_OAUTH_TOKEN:-}"
ANTHROPIC_KEY=""
YES=0
NO_OPEN=0
SERVICE_NAME="office"
DOCKERFILE="deploy/container/Dockerfile"
SSH_USER="agentoffice" # die Shell deines Schlüssels im Container: der Benutzer, der das Office betreibt
TEAM_USER="office"     # Schlüssel der Teammitglieder melden sich als dieser Benutzer an, der nur zum Office tunneln darf
OFFICE_PORT=4600       # wo das Office im Container lauscht (nur 127.0.0.1)
LOCAL_PORT=4600
LOCAL_PORT_SET=0

usage() {
  cat <<'EOF'
OpenAgenticOffice auf Railway — ein Befehl hoch, ein Befehl runter.

Aufruf: deploy/railway.sh <Befehl> [Optionen]

Das Office ist nie im Internet. Es hört im Container auf 127.0.0.1, und alle erreichen es über einen
SSH-Tunnel auf http://localhost:4600, zum sshd des Containers hinter Railways TCP-Proxy.
Konten, Etagen, Projekte, Anmeldungen und Schlüssel der Teammitglieder liegen auf einem Volume und
überstehen Neustarts und erneute Deployments.

Voraussetzungen: Railway CLI 5 oder neuer (angemeldet), ssh, curl, jq.

Befehle
  up                 Das Railway-Projekt anlegen (oder wiederverwenden), diesen Checkout bauen und das
                     Office starten, dann im Browser öffnen. Die erste Seite zeigt das Office-Passwort
                     EINMAL.
  open               Tunnel zu deinem Office aufbauen und im Browser öffnen (Strg-C schließt den Tunnel)
  update             Diesen Checkout neu bauen und erneut bereitstellen (laufende Arbeiter stoppen und
                     machen danach dort weiter, wo sie waren)
  restart            Den Container des Office neu starten, ohne neu zu bauen
  destroy            Das Railway-Projekt löschen: das Office, sein Volume und alles darauf (du musst
                     vorher den Office-Namen eintippen). `down` tut dasselbe.

  service <port>     Den Webserver eines Arbeiters von der 🌐 Dienste-Tafel des Office auf
                     http://localhost:<port> öffnen (durch das Office; Strg-C schließt den Tunnel)
  invite <gh-user>   Ein Teammitglied mit den SSH-Schlüsseln seines GitHub-Kontos hereinlassen und den
                     einen Befehl ausgeben, den du ihm schickst. Oder: invite <name> <öffentlicher-Schlüssel-Datei>
  uninvite <name>    Die Schlüssel eines Teammitglieds entfernen und offene Tunnel trennen
  team               Auflisten, wer eingeladen ist
  status             Projekt, Deployment, SSH-Adresse zeigen und ob das Office läuft
  ssh                Eine Shell im Container, als der Benutzer, der das Office betreibt
  logs               Die Logs des Office verfolgen
  reset-password     Das Passwort vergessen und ein neues einmal im Browser anzeigen

Optionen
  --name <name>             Name des Office (Standard: open-agentic-office). Mehrere Offices = mehrere Projekte
  --workspace <name|id>     Railway-Workspace für ein neues Projekt (nötig, wenn du in mehreren bist)
  --port <n>                Lokaler Port für den Tunnel (Standard: 4600, sonst der nächste freie)
  --github-token <token>    GitHub-Token für private Repos + die Issue-/PR-Tafeln
                            (Standard: dein lokales `gh auth token`)
  --no-github-token         Kein GitHub-Token ins Office legen
  --claude-token <token>    Claude-Abo-Token von `claude setup-token`
                            (Standard: $CLAUDE_CODE_OAUTH_TOKEN). Ohne einen meldest du dich im
                            Terminal des ersten Arbeiters im Office an.
  --anthropic-api-key <key> Stattdessen einen Anthropic-API-Schlüssel verwenden
  --no-open                 Den Browser nicht öffnen (up: auch den Tunnel nicht öffnen)
  -y, --yes                 Nicht um Bestätigung bitten
EOF
}

say() { printf '\033[1;35m▸\033[0m %s\n' "$*"; }
ok() { printf '\033[1;32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!\033[0m %s\n' "$*" >&2; }
die() {
  printf '\033[1;31m✗\033[0m %s\n' "$*" >&2
  exit 1
}

# Das erste Wort, das keine Option ist, ist der Befehl; Optionen dürfen davor oder danach stehen (das
# Office schlägt `deploy/railway.sh --name <name> service <port>` vor).
CMD=""
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name | --workspace | --port | --github-token | --claude-token | --anthropic-api-key)
      [[ $# -ge 2 ]] || die "$1 braucht einen Wert (siehe: deploy/railway.sh help)" ;;
  esac
  case "$1" in
    --name) NAME="$2"; shift 2 ;;
    --workspace) WORKSPACE="$2"; shift 2 ;;
    --port) LOCAL_PORT="$2"; LOCAL_PORT_SET=1; shift 2 ;;
    --github-token) GH_TOKEN_ARG="$2"; shift 2 ;;
    --no-github-token) NO_GH_TOKEN=1; shift ;;
    --claude-token) CLAUDE_TOKEN="$2"; shift 2 ;;
    --anthropic-api-key) ANTHROPIC_KEY="$2"; shift 2 ;;
    --no-open) NO_OPEN=1; shift ;;
    -y | --yes) YES=1; shift ;;
    -h | --help) usage; exit 0 ;;
    -*) die "unbekannte Option $1 (siehe: deploy/railway.sh help)" ;;
    *)
      if [[ -z "$CMD" ]]; then CMD="$1"; else POSITIONAL+=("$1"); fi
      shift
      ;;
  esac
done
CMD="${CMD:-help}"

[[ "$NAME" =~ ^[a-zA-Z0-9-]+$ ]] || die "--name darf nur Buchstaben, Ziffern und Bindestriche enthalten"
[[ "$LOCAL_PORT" =~ ^[0-9]+$ && $LOCAL_PORT -gt 0 && $LOCAL_PORT -lt 65536 ]] || die "--port muss eine Portnummer sein"
STATE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/open-agentic-office/railway/$NAME"
NAME_FLAG=""
[[ "$NAME" != "open-agentic-office" ]] && NAME_FLAG=" --name $NAME"
KEY_FILE="$STATE_DIR/id_ed25519"
KNOWN_HOSTS="$STATE_DIR/known_hosts"
CLAIM_FILE="$STATE_DIR/claim-token"
IDS_FILE="$STATE_DIR/railway.env" # PROJECT_ID ENVIRONMENT_ID SERVICE_ID SSH_HOST SSH_PORT
PROJECT_ID="" ENVIRONMENT_ID="" SERVICE_ID="" SSH_HOST="" SSH_PORT=""

need() { command -v "$1" >/dev/null 2>&1 || die "$1 wird gebraucht (${2:-installiere es zuerst})"; }

# Wertet einen jq-Ausdruck über das JSON auf stdin aus und gibt das Ergebnis aus (Strings roh, Objekte
# kompakt, nichts bei null): json '.id'. Weitere Argumente gehen an jq: json '.id == $x' --arg x "$wert"
json() {
  local filter="$1"
  shift
  jq -rc "$@" "($filter) | select(. != null)"
}

# Die Railway CLI, im Zustandsordner ausgeführt (`railway init` verknüpft ihn mit dem Projekt des Office).
rw() { (cd "$STATE_DIR" && railway "$@"); }
svc() { echo -s "$SERVICE_ID" -e "$ENVIRONMENT_ID"; }

preflight() {
  need railway "https://docs.railway.com/cli"
  railway tcp-proxy --help >/dev/null 2>&1 || die "diese Railway CLI kennt den Befehl tcp-proxy nicht — aktualisiere sie: railway upgrade (oder brew upgrade railway)"
  need ssh
  need curl
  need jq "https://jqlang.github.io/jq/"
  railway whoami >/dev/null 2>&1 || die "die Railway CLI ist nicht angemeldet (führe aus: railway login)"
}

load_ids() {
  # shellcheck disable=SC1090
  [[ -f "$IDS_FILE" ]] && source "$IDS_FILE"
  return 0
}

save_ids() {
  printf 'PROJECT_ID=%q ENVIRONMENT_ID=%q SERVICE_ID=%q SSH_HOST=%q SSH_PORT=%q\n' \
    "$PROJECT_ID" "$ENVIRONMENT_ID" "$SERVICE_ID" "$SSH_HOST" "$SSH_PORT" >"$IDS_FILE"
}

require_office() {
  load_ids
  [[ -n "$SERVICE_ID" && -n "$SSH_HOST" ]] || die "es gibt kein Office namens \"$NAME\" auf Railway — führe aus: deploy/railway.sh up$NAME_FLAG"
}

random_token() { od -An -N24 -tx1 /dev/urandom | tr -d ' \n'; }

open_url() {
  local url="$1"
  if [[ $NO_OPEN -eq 1 ]]; then return; fi
  if command -v open >/dev/null 2>&1; then open "$url"
  elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$url" >/dev/null 2>&1 &
  elif command -v wslview >/dev/null 2>&1; then wslview "$url"
  elif command -v cmd.exe >/dev/null 2>&1; then cmd.exe /c start "" "$url"
  fi
}

# --- Railway ---------------------------------------------------------------------------------------

# Ob $PROJECT_ID noch auf Railway existiert (ein gelöschtes bleibt eine Weile gelistet, mit deletedAt).
# Nur eine Liste, die funktioniert hat, kann sagen, dass es weg ist: Kann Railway nicht gefragt werden
# (Netzwerk, Anmeldung, 2FA), bricht das hier ab, damit nichts neu angelegt oder vergessen wird, während
# das Projekt noch läuft.
project_exists() {
  local list found
  list=$(railway list --json </dev/null) ||
    die "die Railway-Projekte konnten nicht aufgelistet werden, also ist unklar, ob \"$NAME\" ($PROJECT_ID) noch existiert — prüfe: railway whoami"
  found=$(json 'if type == "array" then any(.[]; .id == $id and ((.deletedAt // "") == "")) else "unreadable" end' --arg id "$PROJECT_ID" <<<"$list" 2>/dev/null) || found=""
  [[ "$found" == true || "$found" == false ]] ||
    die "Railways Projektliste (railway list --json) war nicht lesbar, also ist unklar, ob \"$NAME\" ($PROJECT_ID) noch existiert"
  [[ "$found" == true ]]
}

# Das Projekt, seine Umgebung und der Dienst des Office: einmal angelegt, dann in $IDS_FILE gemerkt.
# Jede ID wird gespeichert, sobald es sie gibt, damit ein späterer fehlgeschlagener Schritt nie dazu
# führt, dass das nächste `up` ein zweites Projekt anlegt (oder `destroy` das erste verfehlt).
ensure_project() {
  load_ids
  if [[ -n "$PROJECT_ID" ]] && ! project_exists; then
    warn "das Railway-Projekt für \"$NAME\" ist weg; lege ein neues an"
    PROJECT_ID="" ENVIRONMENT_ID="" SERVICE_ID="" SSH_HOST="" SSH_PORT=""
  fi
  if [[ -z "$PROJECT_ID" ]]; then
    if [[ -z "$WORKSPACE" ]]; then
      local spaces
      spaces=$(railway whoami --json | json '.workspaces | map(.name) | join("\n")')
      [[ $(wc -l <<<"$spaces") -eq 1 ]] || die "du bist in mehreren Railway-Workspaces; wähle einen mit --workspace:
$(sed 's/^/   /' <<<"$spaces")"
      WORKSPACE="$spaces"
    fi
    PROJECT_ID=$(rw init --name "$NAME" --workspace "$WORKSPACE" --json </dev/null 2>/dev/null | json '.id') || PROJECT_ID=""
    [[ -n "$PROJECT_ID" ]] ||
      die "das Railway-Projekt konnte nicht angelegt werden (versuche: railway init --name $NAME)"
    save_ids
    ok "Railway-Projekt $NAME"
  fi
  if [[ -z "$ENVIRONMENT_ID" ]]; then
    ENVIRONMENT_ID=$(rw status --json | json '.environments.edges[0].node.id') || ENVIRONMENT_ID=""
    [[ -n "$ENVIRONMENT_ID" ]] || die "die Umgebung des Railway-Projekts wurde nicht gefunden (führe das hier erneut aus)"
    save_ids
  fi
  if [[ -z "$SERVICE_ID" ]]; then
    SERVICE_ID=$(rw add --service "$SERVICE_NAME" --json </dev/null 2>/dev/null | json '.id') || SERVICE_ID=""
    [[ -n "$SERVICE_ID" ]] || die "der Dienst des Office konnte nicht hinzugefügt werden (führe das hier erneut aus)"
    save_ids
    ok "Dienst $SERVICE_NAME"
  fi
  # Verknüpft, damit einfache `railway`-Befehle im Zustandsordner auf den Dienst des Office wirken.
  rw link --project "$PROJECT_ID" --environment "$ENVIRONMENT_ID" --service "$SERVICE_ID" --json </dev/null >/dev/null 2>&1 || true
}

# Alles, was das Office aufbewahrt, liegt auf diesem Volume (siehe deploy/container/start.sh).
ensure_volume() {
  local have
  have=$(rw volume $(svc) list --json | json '[.volumes[] | select(.serviceName == $s and .mountPath == "/data")] | length' --arg s "$SERVICE_NAME")
  if [[ "${have:-0}" -eq 0 ]]; then
    rw volume $(svc) add --mount-path /data --json </dev/null >/dev/null || die "das Volume konnte nicht hinzugefügt werden"
    ok "Volume auf /data"
  fi
}

# Railways TCP-Proxy vor dem sshd des Containers: der einzige Weg hinein.
ensure_tcp_proxy() {
  local out="" i
  for ((i = 0; i < 12; i++)); do
    out=$(rw tcp-proxy list $(svc) --json </dev/null 2>/dev/null | json '.proxies | map(select(.applicationPort == 22)) | .[0]') || out=""
    [[ -n "$out" ]] && break
    # Railway kann ihn direkt nach dem Hinzufügen des Volumes kurz ablehnen: noch einmal versuchen.
    rw tcp-proxy create $(svc) --port 22 --json </dev/null >/dev/null 2>"$STATE_DIR/tcp-proxy.log" || sleep 5
  done
  [[ -n "$out" ]] || die "der TCP-Proxy konnte nicht hinzugefügt werden: $(tail -n 3 "$STATE_DIR/tcp-proxy.log")"
  SSH_HOST=$(json '.domain' <<<"$out")
  SSH_PORT=$(json '.proxyPort' <<<"$out")
  [[ -n "$SSH_HOST" && -n "$SSH_PORT" ]] || die "der TCP-Proxy hat noch keine Adresse"
  save_ids
  ok "SSH unter $SSH_HOST:$SSH_PORT"
}

# Der Status des neuesten Deployments: BUILDING, DEPLOYING, SUCCESS, FAILED, CRASHED, REMOVED… Mit einer
# Deployment-ID WAITING, bis genau dieses das neueste ist.
deploy_status() {
  rw service status $(svc) --json </dev/null 2>/dev/null | json 'if $id != "" and .deploymentId != $id then "WAITING" else .status end' --arg id "${1:-}" || true
}

# Baut diesen Checkout mit deploy/container/Dockerfile und stellt ihn bereit.
deploy() {
  local log="$STATE_DIR/build.log" what
  what=$(git -C "$REPO_DIR" log -1 --format='%h %s' 2>/dev/null || echo 'kein Git-Checkout')
  [[ -z "$(git -C "$REPO_DIR" status --porcelain 2>/dev/null)" ]] || what+=", mit deinen nicht committeten Änderungen"
  say "Baue und stelle diesen Checkout bereit ($what) — ein paar Minuten"
  if ! rw up "$REPO_DIR" --path-as-root $(svc) --ci </dev/null >"$log" 2>&1; then
    tail -n 40 "$log" >&2
    die "der Build ist fehlgeschlagen (das ganze Log: $log)"
  fi
  ok "Gebaut"
  local i st="" id
  # Das Deployment dieses Uploads (aus seinem Link im Log), damit das davor nicht zählt.
  id=$(sed -n 's/.*[?&]id=\([0-9a-f-]*\).*/\1/p' "$log" | head -n 1)
  for ((i = 0; i < 120; i++)); do
    st=$(deploy_status "$id")
    case "$st" in
      SUCCESS) ok "Bereitgestellt"; return ;;
      FAILED | CRASHED | REMOVED) die "das Deployment ist $(tr '[:upper:]' '[:lower:]' <<<"$st") — prüfe: deploy/railway.sh logs$NAME_FLAG" ;;
    esac
    sleep 5
  done
  die "das Deployment ist nach 10 Minuten noch ${st:-am Starten} — prüfe: deploy/railway.sh logs$NAME_FLAG"
}

# --- SSH -------------------------------------------------------------------------------------------

ssh_opts() {
  echo -p "$SSH_PORT" -i "$KEY_FILE" -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$KNOWN_HOSTS" \
    -o ConnectTimeout=10 -o ServerAliveInterval=15 -o LogLevel=ERROR
}

remote() {
  [[ -f "$KEY_FILE" ]] || die "der SSH-Schlüssel dieses Office liegt nicht auf diesem Rechner ($KEY_FILE)"
  # shellcheck disable=SC2046
  ssh $(ssh_opts) "$SSH_USER@$SSH_HOST" "$@"
}

office_get() { remote "curl -fs --max-time 4 http://127.0.0.1:$OFFICE_PORT$1"; }

wait_healthy() {
  local i rc
  for ((i = 0; i < 40; i++)); do
    rc=0
    remote "for i in \$(seq 60); do curl -fs --max-time 4 http://127.0.0.1:$OFFICE_PORT/api/health >/dev/null && exit 0; sleep 2; done; exit 1" \
      2>/dev/null || rc=$?
    [[ $rc -eq 255 ]] || return "$rc" # 255: ssh selbst ist fehlgeschlagen, der Container startet noch
    sleep 5
  done
  return 1
}

# Wann der erste Prozess des Containers gestartet ist: ändert sich, wenn Railway den Container ersetzt.
started() { remote 'stat -c %Y /proc/1' 2>/dev/null || true; }

# Nach einem Neustart oder erneuten Deployment: wartet auf den neuen Container, dann auf das Office darin.
wait_replaced() {
  local was="$1" i now
  for ((i = 0; i < 60; i++)); do
    now=$(started)
    [[ -n "$now" && "$now" != "$was" ]] && break
    sleep 5
  done
  wait_healthy
}

port_busy() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null || (exec 3<>"/dev/tcp/::1/$1") 2>/dev/null; }

pick_port() {
  local p
  if [[ $LOCAL_PORT_SET -eq 1 ]]; then
    port_busy "$LOCAL_PORT" && die "localhost:$LOCAL_PORT ist schon belegt"
    echo "$LOCAL_PORT"
    return
  fi
  for ((p = LOCAL_PORT; p < LOCAL_PORT + 50; p++)); do
    port_busy "$p" || { echo "$p"; return; }
  done
  die "ab $LOCAL_PORT ist kein lokaler Port frei (wähle einen mit --port)"
}

# localhost:<port> zum Office im Container weiterleiten, den Browser öffnen und bis Strg-C halten.
tunnel() {
  local path="$1" port pid i up=0
  port=$(pick_port) || exit 1
  # shellcheck disable=SC2046
  ssh $(ssh_opts) -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$SSH_HOST" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "der SSH-Tunnel zu $SSH_HOST:$SSH_PORT konnte nicht geöffnet werden"
    curl -fs --max-time 2 "http://localhost:$port/api/health" >/dev/null 2>&1 && { up=1; break; }
    sleep 0.5
  done
  if [[ $up -ne 1 ]]; then
    kill "$pid" 2>/dev/null
    die "der Tunnel ist offen, aber das Office antwortet nicht darüber — prüfe: deploy/railway.sh logs$NAME_FLAG"
  fi
  ok "Dein Office: http://localhost:$port$path"
  echo "   (per SSH nach $SSH_HOST:$SSH_PORT getunnelt — lass das laufen, solange du es benutzt; Strg-C schließt es)"
  open_url "http://localhost:$port$path"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn wieder mit: deploy/railway.sh open$NAME_FLAG"
}

# Der Server eines Arbeiters von der 🌐 Dienste-Tafel: localhost:<port> tunnelt zum Office, das ihn
# über diesen Port weiterreicht, also muss der lokale Port dem des Dienstes entsprechen.
service_tunnel() {
  local port="$1" pid i up=0
  port_busy "$port" && die "localhost:$port ist auf diesem Computer schon belegt — beende zuerst, was dort läuft"
  # shellcheck disable=SC2046
  ssh $(ssh_opts) -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$SSH_HOST" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "der SSH-Tunnel zu $SSH_HOST:$SSH_PORT konnte nicht geöffnet werden"
    port_busy "$port" && { up=1; break; }
    sleep 0.5
  done
  [[ $up -eq 1 ]] || { kill "$pid" 2>/dev/null; die "der Tunnel ist nicht hochgekommen"; }
  ok "Der Server des Arbeiters: http://localhost:$port"
  echo "   (durch das Office — melde dich mit dem Office-Passwort an, falls es fragt; Strg-C schließt es)"
  open_url "http://localhost:$port"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn wieder mit: deploy/railway.sh service $port$NAME_FLAG"
}

open_office() {
  local claimable path="/"
  claimable=$(office_get /api/beanspruchen 2>/dev/null || true)
  if [[ "$claimable" == *'"claimable":true'* && -f "$CLAIM_FILE" ]]; then
    path="/beanspruchen?t=$(cat "$CLAIM_FILE")"
    say "Öffne die Einmal-Passwort-Seite — schreib das Passwort auf, es wird nie wieder angezeigt"
  fi
  tunnel "$path"
}

valid_member() { [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,38}$ ]] || die "Namen bestehen aus Buchstaben, Ziffern, Punkten, Bindestrichen und Unterstrichen: $1"; }

team_members() { remote "open-agentic-office-team list"; } # "<name> <Anzahl Schlüssel>" pro Zeile

# --- Befehle ---------------------------------------------------------------------------------------

cmd_up() {
  preflight
  need ssh-keygen
  [[ -f "$REPO_DIR/$DOCKERFILE" ]] || die "führe das in einem Klon von open-agentic-office aus ($DOCKERFILE fehlt)"

  local gh_token="$GH_TOKEN_ARG"
  if [[ -z "$gh_token" && $NO_GH_TOKEN -eq 0 ]] && command -v gh >/dev/null 2>&1; then
    gh_token=$(gh auth token 2>/dev/null || true)
  fi
  [[ $NO_GH_TOKEN -eq 1 ]] && gh_token=""

  say "OpenAgenticOffice \"$NAME\" auf Railway ($(railway whoami --json 2>/dev/null | json 'if (.name // "") != "" then .name else .email end' || true))"
  echo "   App:      dieser Checkout, gebaut mit $DOCKERFILE"
  echo "   Daten:    ein Volume auf /data: Konten, Etagen, Projekte (~/workspace), Anmeldungen, Team-Schlüssel"
  echo "   Zugang:   nur per SSH-Tunnel (das Office ist nie offen), über Railways TCP-Proxy"
  if [[ -n "$gh_token" ]]; then
    echo "   GitHub:   dein GitHub-Token kommt ins Office (private Klone, Issue-/PR-Tafeln, Pushes)"
  else
    echo "   GitHub:   kein Token — private Repos und die Tafeln gehen erst, wenn gh angemeldet ist"
  fi
  if [[ -n "$CLAUDE_TOKEN" || -n "$ANTHROPIC_KEY" ]]; then
    echo "   Claude:   mit dem angegebenen Token angemeldet"
  else
    echo "   Claude:   nicht angemeldet — melde dich im Terminal des ersten Arbeiters an (oder gib --claude-token an)"
  fi

  mkdir -p "$STATE_DIR"
  chmod 700 "$STATE_DIR"
  [[ -f "$KEY_FILE" ]] || ssh-keygen -q -t ed25519 -N '' -C "open-agentic-office-$NAME" -f "$KEY_FILE"
  [[ -f "$CLAIM_FILE" ]] || (umask 077 && random_token >"$CLAIM_FILE")

  ensure_project
  ensure_volume
  ensure_tcp_proxy

  # Dein Schlüssel bekommt eine Shell im Container; der Beanspruchen-Link zeigt das erzeugte Passwort
  # einmal; die Panels des Office schlagen die Befehle dieses Skripts vor. (Der Container findet seine
  # SSH-Adresse in den Variablen, die Railway für den TCP-Proxy setzt.)
  rw variable set $(svc) --skip-deploys "RAILWAY_DOCKERFILE_PATH=$DOCKERFILE" \
    "OAO_ADMIN_KEYS=$(cat "$KEY_FILE.pub")" "OFFICE_BEANSPRUCH_TOKEN=$(cat "$CLAIM_FILE")" \
    "OFFICE_DEPLOY_BEFEHL=deploy/railway.sh$NAME_FLAG" </dev/null >/dev/null || die "die Variablen des Dienstes konnten nicht gesetzt werden"
  [[ -z "$CLAUDE_TOKEN" ]] || printf '%s' "$CLAUDE_TOKEN" | rw variable set $(svc) --skip-deploys --stdin CLAUDE_CODE_OAUTH_TOKEN >/dev/null
  [[ -z "$ANTHROPIC_KEY" ]] || printf '%s' "$ANTHROPIC_KEY" | rw variable set $(svc) --skip-deploys --stdin ANTHROPIC_API_KEY >/dev/null
  # Railways Standard startet ein abgestürztes Office nur ein paar Mal neu.
  rw environment edit -e "$ENVIRONMENT_ID" --service-config "$SERVICE_ID" deploy.restartPolicyType ALWAYS --json </dev/null >/dev/null 2>&1 ||
    warn "die Neustart-Richtlinie konnte nicht auf ALWAYS gesetzt werden"

  deploy
  say "Warte, dass das Office über SSH antwortet"
  wait_healthy || die "das Office ist nicht hochgekommen — prüfe: deploy/railway.sh logs$NAME_FLAG"

  if [[ -n "$gh_token" ]]; then
    say "Melde die GitHub CLI an"
    # In der eigenen gh-Konfiguration auf dem Volume abgelegt, damit gh, git, die Tafeln und die Arbeiter es alle nutzen.
    printf '%s' "$gh_token" | remote 'gh auth login --hostname github.com --git-protocol https --with-token &&
      gh auth setup-git --hostname github.com && gh api user --jq "\"    als \" + .login"' || warn "die GitHub CLI konnte nicht angemeldet werden"
  fi
  local git_name git_email
  git_name=$(git config user.name 2>/dev/null || true)
  git_email=$(git config user.email 2>/dev/null || true)
  [[ -z "$git_name" ]] || remote "git config --global user.name $(printf '%q' "$git_name")"
  [[ -z "$git_email" ]] || remote "git config --global user.email $(printf '%q' "$git_email")"

  ok "Dein Office läuft auf Railway (nur über SSH erreichbar)"
  echo
  echo "   Später öffnen:        deploy/railway.sh open$NAME_FLAG"
  echo "   Teammitglied holen:   der 👥-Einladen-Knopf im Office, oder deploy/railway.sh invite <github-benutzername>$NAME_FLAG"
  echo "   Aktualisieren:        deploy/railway.sh update$NAME_FLAG"
  echo "   Abbauen:              deploy/railway.sh destroy$NAME_FLAG"
  echo
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_open() {
  preflight
  require_office
  wait_healthy || die "das Office antwortet nicht — prüfe: deploy/railway.sh logs$NAME_FLAG"
  open_office
}

cmd_update() {
  preflight
  require_office
  deploy
  wait_healthy || die "das Office ist nicht zurückgekommen — prüfe: deploy/railway.sh logs$NAME_FLAG"
  ok "Aktualisiert (Arbeiter, die liefen, machen dort weiter, wo sie waren)"
}

cmd_restart() {
  preflight
  require_office
  local was
  was=$(started)
  rw restart $(svc) --yes --json </dev/null >/dev/null || die "das Office konnte nicht neu gestartet werden"
  say "Starte neu"
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/railway.sh logs$NAME_FLAG"
  ok "Neu gestartet"
}

cmd_service() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 && "${POSITIONAL[0]}" =~ ^[0-9]+$ && ${POSITIONAL[0]} -gt 0 && ${POSITIONAL[0]} -lt 65536 ]] ||
    die "Aufruf: deploy/railway.sh service <port>   (ein Port von der 🌐 Dienste-Tafel des Office)"
  [[ "${POSITIONAL[0]}" -ne $OFFICE_PORT ]] || die "$OFFICE_PORT ist das Office selbst — nimm: deploy/railway.sh open"
  require_office
  service_tunnel "${POSITIONAL[0]}"
}

cmd_status() {
  preflight
  load_ids
  if [[ -z "$PROJECT_ID" ]]; then
    echo "Es gibt kein Office namens \"$NAME\" auf Railway."
    return
  fi
  echo "Office:      $NAME (Railway-Projekt $PROJECT_ID)"
  echo "Deployment:  $(deploy_status)"
  echo "SSH:         ${SSH_HOST:-keins}:${SSH_PORT}  (Office öffnen mit: deploy/railway.sh open$NAME_FLAG)"
  if [[ -n "$SSH_HOST" && -f "$KEY_FILE" ]] && office_get /api/health >/dev/null 2>&1; then
    echo "Volume:      $(remote "df -h /data | awk 'NR == 2 {print \$6 \", \" \$3 \" von \" \$2 \" belegt\"}'")"
    echo "Office:      läuft"
    local team
    team=$(team_members 2>/dev/null | awk '{printf "%s%s", sep, $1; sep=", "}')
    echo "Team:        ${team:-noch niemand eingeladen}"
  else
    echo "Volume:      $(rw volume $(svc) list --json | json '[.volumes[] | select(.serviceName == $s) | .mountPath + ", " + ((.sizeMB / 1000) | round | tostring) + " GB"] | join("; ") | if . == "" then "keins" else . end' --arg s "$SERVICE_NAME")"
    echo "Office:      antwortet nicht"
  fi
}

cmd_invite() {
  preflight
  [[ ${#POSITIONAL[@]} -ge 1 && ${#POSITIONAL[@]} -le 2 ]] ||
    die "Aufruf: deploy/railway.sh invite <github-benutzername>   oder   deploy/railway.sh invite <name> <öffentlicher-Schlüssel-Datei>"
  local who="${POSITIONAL[0]}" src raw
  valid_member "$who"
  if [[ ${#POSITIONAL[@]} -eq 2 ]]; then
    src="${POSITIONAL[1]}"
    [[ -f "$src" ]] || die "keine solche Datei: $src"
    raw=$(cat "$src")
  else
    src="github.com/$who.keys"
    raw=$(curl -fsS --max-time 10 "https://github.com/$who.keys") || die "https://$src konnte nicht geladen werden"
  fi
  [[ -n "$raw" ]] || die "keine öffentlichen SSH-Schlüssel in $src gefunden"
  require_office
  local n fp
  # Der Container behält nur gültige Schlüssel und beschränkt jeden darauf, den Tunnel zu öffnen.
  n=$(printf '%s\n' "$raw" | remote "open-agentic-office-team add $who") || die "die Schlüssel von $who aus $src konnten nicht hinzugefügt werden"
  ok "$who ist eingeladen ($n Schlüssel aus $src)"
  fp=$(remote "open-agentic-office-team fingerprint")
  echo
  echo "   Schick $who das hier:"
  echo
  echo "     ssh -L 4600:localhost:$OFFICE_PORT ssh://$TEAM_USER@$SSH_HOST:$SSH_PORT"
  echo
  echo "     Lass es laufen, öffne http://localhost:4600 und melde dich mit dem Office-Passwort an."
  echo "     Beim ersten Mal fragt ssh, ob du dem Server vertraust. Sag nur ja, wenn es"
  echo "     ED25519 key fingerprint $fp"
  echo "     anzeigt."
  echo
}

cmd_uninvite() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/railway.sh uninvite <name>"
  local who="${POSITIONAL[0]}" out rc=0
  valid_member "$who"
  require_office
  out=$(remote "open-agentic-office-team remove $who" 2>&1) || rc=$?
  [[ $rc -eq 66 ]] && die "$who ist nicht eingeladen (siehe: deploy/railway.sh team$NAME_FLAG)"
  [[ $rc -eq 0 ]] || die "die Schlüssel konnten nicht entfernt werden: $out"
  ok "Die Schlüssel von $who sind entfernt und offene Tunnel getrennt (andere Teammitglieder verbinden sich einfach neu)"
  echo "   Es kennt weiterhin das Office-Passwort. Zum Ändern: deploy/railway.sh reset-password$NAME_FLAG"
}

cmd_team() {
  preflight
  require_office
  local list
  list=$(team_members)
  if [[ -z "$list" ]]; then
    echo "Noch niemand ist eingeladen. Hol jemanden: deploy/railway.sh invite <github-benutzername>$NAME_FLAG"
    return
  fi
  echo "$list" | awk '{printf "%s  (%d Schlüssel)\n", $1, $2}'
}

cmd_ssh() {
  preflight
  require_office
  # shellcheck disable=SC2046
  exec ssh $(ssh_opts) -t "$SSH_USER@$SSH_HOST" "${POSITIONAL[@]+"${POSITIONAL[@]}"}"
}

cmd_logs() {
  preflight
  require_office
  rw logs $(svc)
}

cmd_reset_password() {
  preflight
  require_office
  (umask 077 && random_token >"$CLAIM_FILE")
  say "Setze das Office-Passwort zurück"
  rw variable set $(svc) --skip-deploys "OFFICE_BEANSPRUCH_TOKEN=$(cat "$CLAIM_FILE")" >/dev/null || die "das neue Beanspruchen-Token konnte nicht gesetzt werden"
  local was
  was=$(started)
  remote "/opt/open-agentic-office/open-agentic-office --daten ~/open-agentic-office --passwort-zuruecksetzen >/dev/null" || die "das Zurücksetzen ist fehlgeschlagen"
  # Ein erneutes Deployment, damit das Office mit dem neuen Beanspruchen-Token startet (und das neue Passwort erzeugt).
  rw redeploy $(svc) --yes --json </dev/null >/dev/null || die "das Office konnte nicht erneut bereitgestellt werden"
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/railway.sh logs$NAME_FLAG"
  ok "Alle wurden abgemeldet"
  open_office
}

cmd_down() {
  preflight
  load_ids
  if [[ -z "$PROJECT_ID" ]]; then
    echo "Für \"$NAME\" gibt es nichts zu löschen."
    rm -rf "$STATE_DIR"
    return
  fi
  # Der Zustandsordner (SSH-Schlüssel, Beanspruchen-Token und IDs dieses Office) verschwindet erst, wenn
  # das Projekt weg ist: project_exists bricht ab, wenn Railway es nicht sagen kann, und ein
  # fehlgeschlagenes Löschen hält hier an.
  if project_exists; then
    say "Das löscht Office \"$NAME\" endgültig: Railway-Projekt $PROJECT_ID, sein Volume und alles darauf."
    echo "   Alles im Office, was nicht nach GitHub gepusht ist, geht verloren."
    if [[ $YES -ne 1 ]]; then
      read -r -p "   Tippe zur Bestätigung den Office-Namen ($NAME) ein: " answer
      [[ "$answer" == "$NAME" ]] || die "abgebrochen"
    fi
    railway delete --project "$PROJECT_ID" --yes </dev/null >/dev/null ||
      die "das Projekt konnte nicht gelöscht werden (mit 2FA führe aus: railway delete --project $PROJECT_ID, dann das hier erneut)"
    ok "Railway-Projekt gelöscht (der Dienst, sein Volume und der TCP-Proxy gehen mit)"
  else
    ok "Das Railway-Projekt $PROJECT_ID ist schon gelöscht"
  fi
  rw unlink --yes </dev/null >/dev/null 2>&1 || true # die Verknüpfung des Zustandsordners in der Konfiguration der CLI vergessen
  rm -rf "$STATE_DIR"
  ok "Alles weg"
}

case "$CMD" in
  up) cmd_up ;;
  open) cmd_open ;;
  update) cmd_update ;;
  restart) cmd_restart ;;
  service) cmd_service ;;
  status) cmd_status ;;
  invite) cmd_invite ;;
  uninvite) cmd_uninvite ;;
  team) cmd_team ;;
  ssh) cmd_ssh ;;
  logs) cmd_logs ;;
  reset-password) cmd_reset_password ;;
  destroy | down) cmd_down ;;
  help | -h | --help) usage ;;
  *) die "unbekannter Befehl \"$CMD\" (siehe: deploy/railway.sh help)" ;;
esac
