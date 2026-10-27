#!/usr/bin/env bash
# Dein eigenes OpenAgenticOffice mit einem Befehl auf Fly.io bereitstellen, mit flyctl.
#
#   deploy/fly.sh up        App anlegen, das Office bauen und starten, im Browser öffnen
#   deploy/fly.sh open      Tunnel zum Office aufbauen und im Browser öffnen
#   deploy/fly.sh destroy   App, Volume und alles darauf löschen (fragt vorher)
#
# Das Office ist nie im Internet: Es hört in seiner Fly Machine auf 127.0.0.1, und alle erreichen es
# über einen SSH-Tunnel zum sshd der Maschine, auf der eigenen IPv4-Adresse der App. Was es aufbewahrt
# (Konten, Etagen, Projekte, Anmeldungen, Schlüssel der Teammitglieder, der SSH-Hostschlüssel) liegt
# auf einem Fly-Volume unter /data, sodass Neustarts und erneute Deployments nichts davon verlieren.
# Das Image ist dasselbe wie bei deploy/railway.sh, deploy/container/Dockerfile, gebaut aus diesem
# Checkout. `deploy/fly.sh help` zeigt alle Befehle und Optionen.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NAME="open-agentic-office"
APP_ARG=""
ORG_ARG=""
REGION_ARG=""
VM_SIZE_ARG=""
MEMORY_ARG=""
DISK=50
GH_TOKEN_ARG=""
NO_GH_TOKEN=0
CLAUDE_TOKEN="${CLAUDE_CODE_OAUTH_TOKEN:-}"
ANTHROPIC_KEY=""
YES=0
NO_OPEN=0
DOCKERFILE="deploy/container/Dockerfile"
SSH_USER="agentoffice" # die Shell deines Schlüssels in der Maschine: der Benutzer, der das Office betreibt
TEAM_USER="office"     # Schlüssel der Teammitglieder melden sich als dieser Benutzer an, der nur zum Office tunneln darf
OFFICE_PORT=4600       # wo das Office in der Maschine lauscht (nur 127.0.0.1)
SSHD_PORT=2222         # der sshd der Maschine: Flys eigener (`fly ssh console`) hat dort Port 22
DEFAULT_VM_SIZE="shared-cpu-4x"
DEFAULT_MEMORY_MB=8192
LOCAL_PORT=4600
LOCAL_PORT_SET=0

usage() {
  cat <<'EOF'
OpenAgenticOffice auf Fly.io — ein Befehl hoch, ein Befehl runter.

Aufruf: deploy/fly.sh <Befehl> [Optionen]

Das Office ist nie im Internet. Es hört in seiner Fly Machine auf 127.0.0.1, und alle erreichen es
über einen SSH-Tunnel auf http://localhost:4600, zum sshd der Maschine auf der eigenen IPv4-Adresse
der App. Konten, Etagen, Projekte, Anmeldungen und Schlüssel der Teammitglieder liegen auf einem
Volume und überstehen Neustarts und erneute Deployments.

Voraussetzungen: flyctl (angemeldet), ssh, curl, jq.

Befehle
  up                 Die Fly-App anlegen (oder wiederverwenden), diesen Checkout bauen und das Office
                     starten, dann im Browser öffnen. Die erste Seite zeigt das Office-Passwort EINMAL.
  open               Tunnel zu deinem Office aufbauen und im Browser öffnen (Strg-C schließt den Tunnel)
  update             Diesen Checkout neu bauen und erneut bereitstellen (laufende Arbeiter stoppen und
                     machen danach dort weiter, wo sie waren)
  restart            Die Maschine des Office neu starten, ohne neu zu bauen
  resize <größe> [speicher]
                     Die Maschine ändern, z. B. resize performance-2x oder resize shared-cpu-8x 16gb
                     (Größen: fly platform vm-sizes). Der Speicher bleibt gleich, wenn du keinen angibst
  pause              Die Maschine anhalten, und damit die Rechnung für ihre CPU und ihren Speicher
  resume             Wieder starten und öffnen
  destroy            Die Fly-App löschen: das Office, sein Volume und alles darauf (du musst vorher den
                     Office-Namen eintippen). `down` tut dasselbe.

  service <port>     Den Webserver eines Arbeiters von der 🌐 Dienste-Tafel des Office auf
                     http://localhost:<port> öffnen (durch das Office; Strg-C schließt den Tunnel)
  invite <gh-user>   Ein Teammitglied mit den SSH-Schlüsseln seines GitHub-Kontos hereinlassen und den
                     einen Befehl ausgeben, den du ihm schickst. Oder: invite <name> <öffentlicher-Schlüssel-Datei>
  uninvite <name>    Die Schlüssel eines Teammitglieds entfernen und offene Tunnel trennen
  team               Auflisten, wer eingeladen ist
  status             App, Maschine, SSH-Adresse zeigen und ob das Office läuft
  ssh                Eine Shell in der Maschine, als der Benutzer, der das Office betreibt
  logs               Die Logs des Office verfolgen
  reset-password     Das Passwort vergessen und ein neues einmal im Browser anzeigen

Optionen
  --name <name>             Name des Office (Standard: open-agentic-office). Mehrere Offices = mehrere Apps
  --app <name>              Fly-App-Name für ein neues Office. App-Namen sind auf ganz Fly.io eindeutig
                            (Standard: der Office-Name plus ein zufälliges Suffix)
  --org <slug>              Fly-Organisation für ein neues Office (nötig, wenn du in mehreren bist)
  --region <code>           Fly-Region für ein neues Office (Standard: die nächste zu dir; siehe: fly
                            platform regions). Sein Volume bleibt dort
  --vm-size <größe>         Maschinengröße (Standard: shared-cpu-4x)
  --memory <größe>          Speicher der Maschine, etwa 8gb oder 16384 (MB) (Standard: 8gb)
  --disk <GB>               Volume-Größe für ein neues Office (Standard: 50)
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
# Office schlägt `deploy/fly.sh --name <name> service <port>` vor).
CMD=""
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name | --app | --org | --region | --vm-size | --memory | --disk | --port | --github-token | --claude-token | --anthropic-api-key)
      [[ $# -ge 2 ]] || die "$1 braucht einen Wert (siehe: deploy/fly.sh help)" ;;
  esac
  case "$1" in
    --name) NAME="$2"; shift 2 ;;
    --app) APP_ARG="$2"; shift 2 ;;
    --org) ORG_ARG="$2"; shift 2 ;;
    --region) REGION_ARG="$2"; shift 2 ;;
    --vm-size) VM_SIZE_ARG="$2"; shift 2 ;;
    --memory) MEMORY_ARG="$2"; shift 2 ;;
    --disk) DISK="$2"; shift 2 ;;
    --port) LOCAL_PORT="$2"; LOCAL_PORT_SET=1; shift 2 ;;
    --github-token) GH_TOKEN_ARG="$2"; shift 2 ;;
    --no-github-token) NO_GH_TOKEN=1; shift ;;
    --claude-token) CLAUDE_TOKEN="$2"; shift 2 ;;
    --anthropic-api-key) ANTHROPIC_KEY="$2"; shift 2 ;;
    --no-open) NO_OPEN=1; shift ;;
    -y | --yes) YES=1; shift ;;
    -h | --help) usage; exit 0 ;;
    -*) die "unbekannte Option $1 (siehe: deploy/fly.sh help)" ;;
    *)
      if [[ -z "$CMD" ]]; then CMD="$1"; else POSITIONAL+=("$1"); fi
      shift
      ;;
  esac
done
CMD="${CMD:-help}"

# 8gb, 16g, 8192mb oder 8192 → Megabyte
memory_mb() {
  local m
  m=$(tr '[:upper:]' '[:lower:]' <<<"$1")
  m=${m%b}
  case "$m" in
    *g) m=${m%g}; [[ "$m" =~ ^[0-9]+$ ]] && echo $((m * 1024)) ;;
    *m) m=${m%m}; [[ "$m" =~ ^[0-9]+$ ]] && echo "$m" ;;
    *) [[ "$m" =~ ^[0-9]+$ ]] && echo "$m" ;;
  esac
}

[[ "$NAME" =~ ^[a-zA-Z0-9-]+$ ]] || die "--name darf nur Buchstaben, Ziffern und Bindestriche enthalten"
[[ -z "$APP_ARG" || "$APP_ARG" =~ ^[a-z0-9][a-z0-9-]{1,62}$ ]] || die "--app darf nur Kleinbuchstaben, Ziffern und Bindestriche enthalten"
[[ -z "$ORG_ARG" || "$ORG_ARG" =~ ^[a-zA-Z0-9_-]+$ ]] || die "--org erwartet den Slug einer Organisation (siehe: fly orgs list)"
[[ -z "$REGION_ARG" || "$REGION_ARG" =~ ^[a-z]{3}$ ]] || die "--region erwartet einen dreibuchstabigen Regionscode (siehe: fly platform regions)"
[[ -z "$VM_SIZE_ARG" || "$VM_SIZE_ARG" =~ ^[a-z0-9-]+$ ]] || die "--vm-size erwartet eine Größe wie shared-cpu-4x (siehe: fly platform vm-sizes)"
[[ -z "$MEMORY_ARG" || $(memory_mb "$MEMORY_ARG") -ge 256 ]] 2>/dev/null || die "--memory erwartet eine Größe wie 8gb oder 16384 (MB)"
[[ "$DISK" =~ ^[0-9]+$ && $DISK -ge 1 && $DISK -le 500 ]] || die "--disk muss eine Größe in GB sein, 1 bis 500"
[[ "$LOCAL_PORT" =~ ^[0-9]+$ && $LOCAL_PORT -gt 0 && $LOCAL_PORT -lt 65536 ]] || die "--port muss eine Portnummer sein"
STATE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/open-agentic-office/fly/$NAME"
NAME_FLAG=""
[[ "$NAME" != "open-agentic-office" ]] && NAME_FLAG=" --name $NAME"
KEY_FILE="$STATE_DIR/id_ed25519"
KNOWN_HOSTS="$STATE_DIR/known_hosts"
CLAIM_FILE="$STATE_DIR/claim-token"
TOML="$STATE_DIR/fly.toml"          # vor jedem Deploy aus den Einstellungen unten geschrieben
IDS_FILE="$STATE_DIR/fly.env"        # APP ORG REGION VM_SIZE VM_MEMORY SSH_HOST SSH_PORT
APP="" ORG="" REGION="" VM_SIZE="" VM_MEMORY="" SSH_HOST="" SSH_PORT=""
FLYCTL=""

need() { command -v "$1" >/dev/null 2>&1 || die "$1 wird gebraucht (${2:-installiere es zuerst})"; }

# Wertet einen jq-Ausdruck über das JSON auf stdin aus und gibt das Ergebnis aus (Strings roh, Objekte
# kompakt, nichts bei null): json '.id'. Weitere Argumente gehen an jq: json '.Name == $x' --arg x "$wert"
json() {
  local filter="$1"
  shift
  jq -rc "$@" "($filter) | select(. != null)"
}

# flyctl, im Zustandsordner ausgeführt: Es liest die fly.toml dort, nie eine aus deinem aktuellen Verzeichnis.
fl() {
  (
    cd "$STATE_DIR" 2>/dev/null || cd /
    "$FLYCTL" "$@"
  )
}

preflight() {
  # Homebrew und Flys Installer legen beide Namen in den PATH; die CLI von Concourse CI heißt auch `fly`.
  FLYCTL=$(command -v flyctl || command -v fly) || die "flyctl wird gebraucht (https://fly.io/docs/flyctl/install/)"
  export FLY_NO_UPDATE_CHECK=1
  need ssh
  need curl
  need jq "https://jqlang.github.io/jq/"
  "$FLYCTL" auth whoami >/dev/null 2>&1 </dev/null || die "flyctl ist nicht angemeldet (führe aus: fly auth login)"
}

load_ids() {
  # shellcheck disable=SC1090
  [[ -f "$IDS_FILE" ]] && source "$IDS_FILE"
  return 0
}

save_ids() {
  printf 'APP=%q ORG=%q REGION=%q VM_SIZE=%q VM_MEMORY=%q SSH_HOST=%q SSH_PORT=%q\n' \
    "$APP" "$ORG" "$REGION" "$VM_SIZE" "$VM_MEMORY" "$SSH_HOST" "$SSH_PORT" >"$IDS_FILE"
}

require_office() {
  load_ids
  [[ -n "$APP" && -n "$SSH_HOST" ]] || die "es gibt kein Office namens \"$NAME\" auf Fly.io — führe aus: deploy/fly.sh up$NAME_FLAG"
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

# --- Fly.io ----------------------------------------------------------------------------------------

# Ob $APP noch auf Fly.io existiert. Nur eine Liste, die funktioniert hat, kann sagen, dass sie weg ist:
# Kann Fly nicht gefragt werden (Netzwerk, abgelaufene Anmeldung), bricht das hier ab, damit nichts neu
# angelegt oder vergessen wird, während die App noch läuft.
app_exists() {
  local list found
  list=$(fl apps list --json </dev/null) ||
    die "deine Fly.io-Apps konnten nicht aufgelistet werden, also ist unklar, ob \"$NAME\" ($APP) noch existiert — prüfe: fly auth whoami"
  found=$(json 'if type == "array" then any(.[]; (.Name // .name) == $app) elif . == null then false else "unreadable" end' --arg app "$APP" <<<"$list" 2>/dev/null) || found=""
  [[ "$found" == true || "$found" == false ]] ||
    die "Flys App-Liste (fly apps list --json) war nicht lesbar, also ist unklar, ob \"$NAME\" ($APP) noch existiert"
  [[ "$found" == true ]]
}

# Die Fly-Region, die diesem Computer am nächsten ist: die, aus der Flys Edge antwortet.
nearest_region() {
  curl -fsSI --max-time 5 https://debug.fly.dev 2>/dev/null | tr -d '\r' | tr '[:upper:]' '[:lower:]' |
    sed -n 's/^fly-region: *\([a-z]\{3\}\)$/\1/p' | head -n 1
}

# Legt App, Organisation, Region und Maschinengröße fest, gemerkt in $IDS_FILE; legt nichts an.
# Ein neues Office bekommt einen neuen App-Namen, eine Region und die Standardgröße; ein bestehendes
# behält seine eigenen, außer Größe und Speicher, die du angibst.
resolve_settings() {
  load_ids
  if [[ -n "$APP" ]] && ! app_exists; then
    if [[ -n "$SSH_HOST" ]]; then
      warn "die Fly-App für \"$NAME\" ($APP) ist weg; lege eine neue an"
      rm -f "$KNOWN_HOSTS" # eine neue Maschine, mit neuem Hostschlüssel
    fi
    SSH_HOST="" SSH_PORT=""
    [[ -z "$APP_ARG" ]] || APP="" # unten benannt
    [[ -z "$ORG_ARG" ]] || ORG="$ORG_ARG"
    [[ -z "$REGION_ARG" ]] || REGION="$REGION_ARG"
  elif [[ -n "$APP" ]]; then
    [[ -z "$APP_ARG" || "$APP_ARG" == "$APP" ]] || die "Office \"$NAME\" ist schon die Fly-App $APP (--app benennt nur eine neue)"
    [[ -z "$REGION_ARG" || "$REGION_ARG" == "$REGION" ]] || warn "Office \"$NAME\" bleibt in $REGION: Sein Volume liegt dort"
  fi
  if [[ -z "$APP" ]]; then
    APP="${APP_ARG:-$(tr '[:upper:]' '[:lower:]' <<<"$NAME")-$(od -An -N3 -tx1 /dev/urandom | tr -d ' \n')}"
    # Nie eine App, die es schon gibt: Das Office würde sie übernehmen und `destroy` sie löschen.
    ! app_exists || die "du hast schon eine Fly-App namens $APP — gib diesem Office mit --app <name> eine neue"
  fi
  if [[ -z "$ORG" ]]; then
    ORG="$ORG_ARG"
    if [[ -z "$ORG" ]]; then
      local orgs
      orgs=$(fl orgs list --json </dev/null | json 'keys_unsorted | join("\n")') || orgs=""
      [[ -n "$orgs" ]] || die "deine Fly.io-Organisationen konnten nicht aufgelistet werden (fly orgs list)"
      [[ $(wc -l <<<"$orgs") -eq 1 ]] || die "du bist in mehreren Fly.io-Organisationen; wähle eine mit --org:
$(sed 's/^/   /' <<<"$orgs")"
      ORG="$orgs"
    fi
  fi
  if [[ -z "$REGION" ]]; then
    REGION="${REGION_ARG:-$(nearest_region)}"
    REGION="${REGION:-iad}"
  fi
  [[ -z "$VM_SIZE_ARG" ]] || VM_SIZE="$VM_SIZE_ARG"
  [[ -z "$MEMORY_ARG" ]] || VM_MEMORY=$(memory_mb "$MEMORY_ARG")
  VM_SIZE="${VM_SIZE:-$DEFAULT_VM_SIZE}"
  VM_MEMORY="${VM_MEMORY:-$DEFAULT_MEMORY_MB}"
  # Der öffentliche SSH-Port: ein zufälliger, wie bei Railways TCP-Proxy, damit die Scanner, die den
  # ganzen Tag an Port 22 jeder Adresse klopfen, nicht die Logs des Office füllen.
  SSH_PORT="${SSH_PORT:-$((20000 + RANDOM % 40000))}"
  save_ids
}

# Die App selbst. Ihr Name wird vor dem Anlegen gespeichert (resolve_settings hat geprüft, dass sie neu
# ist), damit ein Fehler hier nie dazu führt, dass das nächste `up` eine zweite App anlegt (oder
# `destroy` die erste verfehlt).
ensure_app() {
  app_exists && return
  fl apps create "$APP" --org "$ORG" </dev/null >/dev/null 2>"$STATE_DIR/fly.log" ||
    die "die Fly-App $APP konnte nicht angelegt werden: $(tail -n 3 "$STATE_DIR/fly.log") (App-Namen sind auf ganz Fly.io eindeutig: wähle einen anderen mit --app <name>)"
  ok "Fly-App $APP ($ORG)"
}

# Alles, was das Office aufbewahrt, liegt auf diesem Volume (siehe deploy/container/start.sh).
ensure_volume() {
  local list have
  list=$(fl volumes list -a "$APP" --json </dev/null) || die "die Volumes der App konnten nicht aufgelistet werden"
  have=$(json '(. // []) | map(select(.name == "data" and ((.state // "") | test("destroy") | not))) | length' <<<"$list")
  if [[ "${have:-0}" -eq 0 ]]; then
    fl volumes create data -a "$APP" --region "$REGION" --size "$DISK" --yes </dev/null >/dev/null 2>"$STATE_DIR/fly.log" ||
      die "das Volume in $REGION konnte nicht angelegt werden: $(tail -n 3 "$STATE_DIR/fly.log") (andere Region: --region <code>, siehe: fly platform regions)"
    ok "Volume auf /data ($DISK GB in $REGION)"
  fi
}

# Die eigene IPv4-Adresse der App, oder nichts; bricht ab, wenn Fly es nicht sagen kann.
dedicated_ipv4() {
  local list
  list=$(fl ips list -a "$APP" --json </dev/null) || die "die IP-Adressen der App konnten nicht aufgelistet werden"
  json '(. // []) | map(select((.Type // .type) == "v4")) | (.[0] // {}) | .Address' <<<"$list"
}

# SSH ist der einzige Weg hinein, auf einer Adresse der App selbst: Flys gemeinsame IPv4-Adressen
# tragen nur HTTP und TLS.
ensure_ip() {
  SSH_HOST=$(dedicated_ipv4)
  if [[ -z "$SSH_HOST" ]]; then
    fl ips allocate-v4 -a "$APP" --yes </dev/null >/dev/null 2>"$STATE_DIR/fly.log" ||
      die "die App bekam keine IPv4-Adresse: $(tail -n 3 "$STATE_DIR/fly.log")"
    SSH_HOST=$(dedicated_ipv4)
    [[ -n "$SSH_HOST" ]] || die "die App hat noch keine IPv4-Adresse (führe das hier erneut aus)"
  fi
  save_ids
  ok "SSH unter $SSH_HOST:$SSH_PORT"
}

# Die Konfiguration der App, aus den Einstellungen in $IDS_FILE. Dein Schlüssel bekommt eine Shell in
# der Maschine, und die Panels des Office schlagen die Befehle dieses Skripts vor.
write_toml() {
  cat >"$TOML" <<EOF
# Von deploy/fly.sh für Office "$NAME" geschrieben. Es schreibt diese Datei vor jedem Deploy neu: Ändere
# die Maschine mit \`deploy/fly.sh resize\` und Einstellungen mit \`fly secrets set\`.
app = "$APP"
primary_region = "$REGION"
kill_signal = "SIGTERM"
kill_timeout = 30

[env]
  OAO_ADMIN_KEYS = "$(cat "$KEY_FILE.pub")"
  OFFICE_DEPLOY_SKRIPT = "deploy/fly.sh$NAME_FLAG"
  OFFICE_OEFFENTLICHER_HOST = "$SSH_HOST:$SSH_PORT"
  OAO_SSHD_PORT = "$SSHD_PORT"
  # Flys init ist PID 1, also sammelt tini als Subreaper ein, was Arbeiter hinterlassen.
  TINI_SUBREAPER = "1"

[[mounts]]
  source = "data"
  destination = "/data"

# SSH und sonst nichts: die IPv4-Adresse der App auf Port $SSH_PORT, zum sshd der Maschine. Die Maschine
# läuft immer; nur \`deploy/fly.sh pause\` hält sie an, und nur \`resume\` startet sie wieder.
[[services]]
  protocol = "tcp"
  internal_port = $SSHD_PORT
  auto_stop_machines = "off"
  auto_start_machines = false

  [[services.ports]]
    port = $SSH_PORT

[[restart]]
  policy = "always"

[[vm]]
  size = "$VM_SIZE"
  memory = "${VM_MEMORY}mb"
EOF
}

# Die Maschine des Office, als "<id> <zustand>" (started, stopped, …), oder nichts vor dem ersten Deploy.
machine() {
  local list
  list=$(fl machine list -a "$APP" --json </dev/null) || die "die Maschinen der App konnten nicht aufgelistet werden"
  json '(. // []) | map(select((.state // "") | test("destroy") | not)) | map(.id + " " + .state) | .[0]' <<<"$list"
}

machine_id() {
  local m
  m=$(machine)
  [[ -n "$m" ]] || die "Office \"$NAME\" hat keine Maschine — führe aus: deploy/fly.sh update$NAME_FLAG"
  echo "${m%% *}"
}

require_running() {
  local m
  m=$(machine)
  [[ "${m#* }" != stopped && "${m#* }" != suspended ]] || die "Office \"$NAME\" ist pausiert — starte es mit: deploy/fly.sh resume$NAME_FLAG"
}

# Baut diesen Checkout mit deploy/container/Dockerfile auf Flys Buildern und stellt ihn bereit.
deploy() {
  local log="$STATE_DIR/deploy.log" what
  what=$(git -C "$REPO_DIR" log -1 --format='%h %s' 2>/dev/null || echo 'kein Git-Checkout')
  [[ -z "$(git -C "$REPO_DIR" status --porcelain 2>/dev/null)" ]] || what+=", mit deinen nicht committeten Änderungen"
  say "Baue und stelle diesen Checkout bereit ($what) — ein paar Minuten"
  write_toml
  # Eine Maschine (--ha=false): Das Volume kann nur an einer hängen. Die IP-Adresse kommt von ensure_ip.
  if ! fl deploy "$REPO_DIR" --config "$TOML" --dockerfile "$REPO_DIR/$DOCKERFILE" \
    --ignorefile "$REPO_DIR/$DOCKERFILE.dockerignore" --remote-only --ha=false --no-public-ips \
    --wait-timeout 10m --yes </dev/null >"$log" 2>&1; then
    tail -n 40 "$log" >&2
    die "das Deployment ist fehlgeschlagen (das ganze Log: $log)"
  fi
  ok "Bereitgestellt"
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
    [[ $rc -eq 255 ]] || return "$rc" # 255: ssh selbst ist fehlgeschlagen, die Maschine startet noch
    sleep 5
  done
  return 1
}

# Wann der erste Prozess der Maschine gestartet ist: ändert sich, wenn die Maschine neu startet.
started() { remote 'stat -c %Y /proc/1' 2>/dev/null || true; }

# Nach einem Neustart oder erneuten Deployment: wartet auf die neu gestartete Maschine, dann auf das Office darauf.
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

# localhost:<port> zum Office in der Maschine weiterleiten, den Browser öffnen und bis Strg-C halten.
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
    die "der Tunnel ist offen, aber das Office antwortet nicht darüber — prüfe: deploy/fly.sh logs$NAME_FLAG"
  fi
  ok "Dein Office: http://localhost:$port$path"
  echo "   (per SSH nach $SSH_HOST:$SSH_PORT getunnelt — lass das laufen, solange du es benutzt; Strg-C schließt es)"
  open_url "http://localhost:$port$path"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn wieder mit: deploy/fly.sh open$NAME_FLAG"
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
  warn "Der Tunnel ist abgebrochen — öffne ihn wieder mit: deploy/fly.sh service $port$NAME_FLAG"
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

  mkdir -p "$STATE_DIR"
  chmod 700 "$STATE_DIR"
  resolve_settings

  say "OpenAgenticOffice \"$NAME\" auf Fly.io ($("$FLYCTL" auth whoami 2>/dev/null </dev/null | tail -n 1))"
  echo "   App:      dieser Checkout, gebaut mit $DOCKERFILE, als Fly-App $APP ($ORG, $REGION)"
  echo "   Maschine: $VM_SIZE mit $((VM_MEMORY / 1024)) GB (ändern mit: deploy/fly.sh resize <größe>)"
  echo "   Daten:    ein Volume auf /data: Konten, Etagen, Projekte (~/workspace), Anmeldungen, Team-Schlüssel"
  echo "   Zugang:   nur per SSH-Tunnel (das Office ist nie offen), auf einer eigenen IPv4-Adresse"
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

  [[ -f "$KEY_FILE" ]] || ssh-keygen -q -t ed25519 -N '' -C "open-agentic-office-$NAME" -f "$KEY_FILE"
  [[ -f "$CLAIM_FILE" ]] || (umask 077 && random_token >"$CLAIM_FILE")

  ensure_app
  ensure_volume
  ensure_ip

  # Secrets, von stdin gelesen, damit kein Token in einer Befehlszeile steht. Der Beanspruchen-Link zeigt
  # das erzeugte Passwort einmal; das Deployment legt sie alle in die Maschine.
  {
    printf 'OFFICE_BEANSPRUCH_TOKEN=%s\n' "$(cat "$CLAIM_FILE")"
    [[ -z "$CLAUDE_TOKEN" ]] || printf 'CLAUDE_CODE_OAUTH_TOKEN=%s\n' "$CLAUDE_TOKEN"
    [[ -z "$ANTHROPIC_KEY" ]] || printf 'ANTHROPIC_API_KEY=%s\n' "$ANTHROPIC_KEY"
  } | fl secrets import --stage -a "$APP" >/dev/null || die "die Secrets der App konnten nicht gesetzt werden"

  deploy
  say "Warte, dass das Office über SSH antwortet"
  wait_healthy || die "das Office ist nicht hochgekommen — prüfe: deploy/fly.sh logs$NAME_FLAG"

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

  ok "Dein Office läuft auf Fly.io (nur über SSH erreichbar)"
  echo
  echo "   Später öffnen:        deploy/fly.sh open$NAME_FLAG"
  echo "   Teammitglied holen:   der 👥-Einladen-Knopf im Office, oder deploy/fly.sh invite <github-benutzername>$NAME_FLAG"
  echo "   Aktualisieren:        deploy/fly.sh update$NAME_FLAG"
  echo "   Nicht mehr zahlen:    deploy/fly.sh pause$NAME_FLAG (oder destroy$NAME_FLAG)"
  echo
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_open() {
  preflight
  require_office
  require_running
  wait_healthy || die "das Office antwortet nicht — prüfe: deploy/fly.sh logs$NAME_FLAG"
  open_office
}

cmd_update() {
  preflight
  require_office
  require_running
  deploy
  wait_healthy || die "das Office ist nicht zurückgekommen — prüfe: deploy/fly.sh logs$NAME_FLAG"
  ok "Aktualisiert (Arbeiter, die liefen, machen dort weiter, wo sie waren)"
}

cmd_restart() {
  preflight
  require_office
  require_running
  local was id
  id=$(machine_id)
  was=$(started)
  say "Starte neu"
  fl machine restart "$id" -a "$APP" </dev/null >/dev/null || die "das Office konnte nicht neu gestartet werden"
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/fly.sh logs$NAME_FLAG"
  ok "Neu gestartet"
}

cmd_resize() {
  preflight
  [[ ${#POSITIONAL[@]} -ge 1 && ${#POSITIONAL[@]} -le 2 && "${POSITIONAL[0]}" =~ ^[a-z0-9-]+$ ]] ||
    die "Aufruf: deploy/fly.sh resize <größe> [speicher]   (z. B. resize performance-2x, resize shared-cpu-8x 16gb; Größen: fly platform vm-sizes)"
  local mem was
  require_office
  require_running
  mem="$VM_MEMORY"
  if [[ ${#POSITIONAL[@]} -eq 2 ]]; then
    mem=$(memory_mb "${POSITIONAL[1]}")
    [[ -n "$mem" && $mem -ge 256 ]] || die "Speicher ist eine Größe wie 8gb oder 16384 (MB)"
  fi
  was=$(started)
  say "Ändere auf ${POSITIONAL[0]} mit $((mem / 1024)) GB (das Office startet neu)"
  fl scale vm "${POSITIONAL[0]}" --vm-memory "$mem" -a "$APP" </dev/null >"$STATE_DIR/fly.log" 2>&1 ||
    die "die Größe konnte nicht geändert werden: $(tail -n 3 "$STATE_DIR/fly.log")"
  VM_SIZE="${POSITIONAL[0]}" VM_MEMORY="$mem"
  save_ids # und damit beim nächsten Deploy auch in der fly.toml
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/fly.sh logs$NAME_FLAG"
  ok "Jetzt $VM_SIZE mit $((VM_MEMORY / 1024)) GB"
}

cmd_pause() {
  preflight
  require_office
  local m
  m=$(machine)
  [[ -n "$m" ]] || die "Office \"$NAME\" hat keine Maschine"
  if [[ "${m#* }" == stopped ]]; then
    ok "Office \"$NAME\" ist schon pausiert"
    return
  fi
  fl machine stop "${m%% *}" -a "$APP" </dev/null >/dev/null || die "die Maschine konnte nicht angehalten werden"
  ok "Pausiert: Fly berechnet CPU und Speicher nicht mehr (das Volume und die IP-Adresse aber weiter). Wieder starten mit: deploy/fly.sh resume$NAME_FLAG"
}

cmd_resume() {
  preflight
  require_office
  local m
  m=$(machine)
  [[ -n "$m" ]] || die "Office \"$NAME\" hat keine Maschine — führe aus: deploy/fly.sh update$NAME_FLAG"
  if [[ "${m#* }" != started ]]; then
    say "Starte die Maschine"
    fl machine start "${m%% *}" -a "$APP" </dev/null >/dev/null || die "die Maschine konnte nicht gestartet werden"
  fi
  wait_healthy || die "das Office ist nicht hochgekommen — prüfe: deploy/fly.sh logs$NAME_FLAG"
  ok "Läuft"
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_service() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 && "${POSITIONAL[0]}" =~ ^[0-9]+$ && ${POSITIONAL[0]} -gt 0 && ${POSITIONAL[0]} -lt 65536 ]] ||
    die "Aufruf: deploy/fly.sh service <port>   (ein Port von der 🌐 Dienste-Tafel des Office)"
  [[ "${POSITIONAL[0]}" -ne $OFFICE_PORT ]] || die "$OFFICE_PORT ist das Office selbst — nimm: deploy/fly.sh open"
  require_office
  service_tunnel "${POSITIONAL[0]}"
}

cmd_status() {
  preflight
  load_ids
  if [[ -z "$APP" ]]; then
    echo "Es gibt kein Office namens \"$NAME\" auf Fly.io."
    return
  fi
  local m
  m=$(machine)
  echo "Office:      $NAME (Fly-App $APP, $ORG, $REGION)"
  echo "Maschine:    ${m:-keine} — $VM_SIZE mit $((VM_MEMORY / 1024)) GB"
  echo "SSH:         ${SSH_HOST:-keins}:${SSH_PORT}  (Office öffnen mit: deploy/fly.sh open$NAME_FLAG)"
  if [[ -n "$SSH_HOST" && -f "$KEY_FILE" && "${m#* }" == started ]] && office_get /api/health >/dev/null 2>&1; then
    echo "Volume:      $(remote "df -h /data | awk 'NR == 2 {print \$6 \", \" \$3 \" von \" \$2 \" belegt\"}'")"
    echo "Office:      läuft"
    local team
    team=$(team_members 2>/dev/null | awk '{printf "%s%s", sep, $1; sep=", "}')
    echo "Team:        ${team:-noch niemand eingeladen}"
  else
    echo "Volume:      $(fl volumes list -a "$APP" --json </dev/null | json '(. // []) | map(select((.state // "") | test("destroy") | not)) | map("/data, " + (.size_gb | tostring) + " GB") | join("; ") | if . == "" then "keins" else . end')"
    if [[ "${m#* }" == stopped ]]; then echo "Office:      pausiert (deploy/fly.sh resume$NAME_FLAG)"; else echo "Office:      antwortet nicht"; fi
  fi
}

cmd_invite() {
  preflight
  [[ ${#POSITIONAL[@]} -ge 1 && ${#POSITIONAL[@]} -le 2 ]] ||
    die "Aufruf: deploy/fly.sh invite <github-benutzername>   oder   deploy/fly.sh invite <name> <öffentlicher-Schlüssel-Datei>"
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
  # Die Maschine behält nur gültige Schlüssel und beschränkt jeden darauf, den Tunnel zu öffnen.
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
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/fly.sh uninvite <name>"
  local who="${POSITIONAL[0]}" out rc=0
  valid_member "$who"
  require_office
  out=$(remote "open-agentic-office-team remove $who" 2>&1) || rc=$?
  [[ $rc -eq 66 ]] && die "$who ist nicht eingeladen (siehe: deploy/fly.sh team$NAME_FLAG)"
  [[ $rc -eq 0 ]] || die "die Schlüssel konnten nicht entfernt werden: $out"
  ok "Die Schlüssel von $who sind entfernt und offene Tunnel getrennt (andere Teammitglieder verbinden sich einfach neu)"
  echo "   Es kennt weiterhin das Office-Passwort. Zum Ändern: deploy/fly.sh reset-password$NAME_FLAG"
}

cmd_team() {
  preflight
  require_office
  local list
  list=$(team_members)
  if [[ -z "$list" ]]; then
    echo "Noch niemand ist eingeladen. Hol jemanden: deploy/fly.sh invite <github-benutzername>$NAME_FLAG"
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
  fl logs -a "$APP"
}

cmd_reset_password() {
  preflight
  require_office
  require_running
  (umask 077 && random_token >"$CLAIM_FILE")
  say "Setze das Office-Passwort zurück"
  local was
  was=$(started)
  remote "/opt/open-agentic-office/open-agentic-office --daten ~/open-agentic-office --passwort-zuruecksetzen >/dev/null" || die "das Zurücksetzen ist fehlgeschlagen"
  # Das Setzen des Secrets startet die Maschine neu, damit das Office mit dem neuen Beanspruchen-Token
  # startet (und das neue Passwort erzeugt).
  printf 'OFFICE_BEANSPRUCH_TOKEN=%s\n' "$(cat "$CLAIM_FILE")" | fl secrets import -a "$APP" >/dev/null ||
    die "das neue Beanspruchen-Token konnte nicht gesetzt werden"
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/fly.sh logs$NAME_FLAG"
  ok "Alle wurden abgemeldet"
  open_office
}

cmd_down() {
  preflight
  load_ids
  if [[ -z "$APP" ]]; then
    echo "Für \"$NAME\" gibt es nichts zu löschen."
    rm -rf "$STATE_DIR"
    return
  fi
  # Der Zustandsordner (SSH-Schlüssel, Beanspruchen-Token und Einstellungen dieses Office) verschwindet
  # erst, wenn die App weg ist: app_exists bricht ab, wenn Fly es nicht sagen kann, und ein
  # fehlgeschlagenes Löschen hält hier an.
  if app_exists; then
    say "Das löscht Office \"$NAME\" endgültig: Fly-App $APP, ihr Volume und alles darauf."
    echo "   Alles im Office, was nicht nach GitHub gepusht ist, geht verloren."
    if [[ $YES -ne 1 ]]; then
      read -r -p "   Tippe zur Bestätigung den Office-Namen ($NAME) ein: " answer
      [[ "$answer" == "$NAME" ]] || die "abgebrochen"
    fi
    fl apps destroy "$APP" --yes </dev/null >/dev/null || die "die App konnte nicht gelöscht werden (versuche: fly apps destroy $APP)"
    ok "Fly-App gelöscht (ihre Maschine, ihr Volume und ihre IP-Adresse gehen mit)"
  else
    ok "Die Fly-App $APP ist schon weg"
  fi
  rm -rf "$STATE_DIR"
  ok "Alles weg"
}

case "$CMD" in
  up) cmd_up ;;
  open) cmd_open ;;
  update) cmd_update ;;
  restart) cmd_restart ;;
  resize) cmd_resize ;;
  pause) cmd_pause ;;
  resume) cmd_resume ;;
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
  *) die "unbekannter Befehl \"$CMD\" (siehe: deploy/fly.sh help)" ;;
esac
