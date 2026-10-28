#!/usr/bin/env bash
# Stellt dein eigenes OpenAgenticOffice mit einem Befehl auf einen Dokploy-Server, über Dokploys API.
#
#   deploy/dokploy.sh up --url https://dokploy.example.com   Office anlegen, bauen, öffnen
#   deploy/dokploy.sh open                                   Tunnel zum Office öffnen und es aufrufen
#   deploy/dokploy.sh destroy                                alles samt Volume löschen (fragt vorher)
#
# Das Office ist nie im Internet: Es hört in seinem Container auf 127.0.0.1, und alle erreichen es
# über einen SSH-Tunnel zum sshd des Containers, der auf einem Port des Dokploy-Servers veröffentlicht
# wird (2222, wenn du keinen anderen wählst). Was es aufbewahrt (Konten, Etagen, Projekte, Anmeldungen,
# Schlüssel der Teammitglieder, der SSH-Hostschlüssel), liegt auf einem Docker-Volume unter /data, sodass
# Neustarts und neue Deployments nichts davon verlieren. Dokploy baut deploy/container/Dockerfile aus
# diesem Checkout, der als Zip hochgeladen wird.
# Alle Befehle und Optionen: `deploy/dokploy.sh help`.
# Voraussetzungen: ssh, ssh-keygen, curl, git, jq.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
NAME="open-agentic-office"
URL_ARG="${DOKPLOY_URL:-}"
API_KEY_ARG="${DOKPLOY_API_KEY:-}"
SERVER_ARG=""
SERVER_PICK="" # seine ID in Dokploy
SSH_HOST_ARG=""
SSH_PORT_ARG=""
GH_TOKEN_ARG=""
NO_GH_TOKEN=0
CLAUDE_TOKEN="${CLAUDE_CODE_OAUTH_TOKEN:-}"
ANTHROPIC_KEY=""
YES=0
NO_OPEN=0
DOCKERFILE="deploy/container/Dockerfile"
DEFAULT_SSH_PORT=2222  # der Port auf dem Dokploy-Server, der zum sshd des Containers führt
SSH_USER="agentoffice" # die Shell deines Schlüssels im Container: der Benutzer, der das Office betreibt
TEAM_USER="office"     # die Schlüssel der Teammitglieder melden sich als dieser Benutzer an, der nur zum Office tunneln darf
OFFICE_PORT=4600       # wo das Office im Container hört (nur 127.0.0.1)
LOCAL_PORT=4600
LOCAL_PORT_SET=0

usage() {
  cat <<'EOF'
OpenAgenticOffice auf Dokploy — ein Befehl hoch, ein Befehl runter.

Aufruf: deploy/dokploy.sh <Befehl> [Optionen]

Das Office ist nie im Internet. Es hört in seinem Container auf 127.0.0.1, und alle erreichen es über
einen SSH-Tunnel auf http://localhost:4600, zum sshd des Containers auf einem Port deines Dokploy-
Servers. Konten, Etagen, Projekte, Anmeldungen und die Schlüssel der Teammitglieder liegen auf einem
Docker-Volume und überstehen Neustarts und neue Deployments.

Voraussetzungen: ssh, ssh-keygen, curl, git und jq.

Das erste `up` braucht die Adresse deines Dokploy und einen API-Schlüssel (Dokploy: Settings →
Profile → API/CLI Keys); beides wird danach für dieses Office gemerkt.

Befehle
  up                 Das Dokploy-Projekt und die Anwendung anlegen (oder wiederverwenden), diesen
                     Checkout zum Bauen für Dokploy hochladen, das Office starten und im Browser öffnen.
                     Die erste Seite zeigt das Office-Passwort EINMAL.
  open               Tunnel zu deinem Office öffnen und es im Browser aufrufen (Strg-C schließt den Tunnel)
  update             Diesen Checkout erneut hochladen, bauen und neu bereitstellen (laufende Arbeiter
                     stoppen und machen danach dort weiter, wo sie waren)
  restart            Den Container des Office neu starten, ohne ihn neu zu bauen
  destroy            Die Anwendung des Office und ihr Volume löschen, dazu das Projekt, wenn nichts
                     anderes darin ist (fragt vorher nach dem Office-Namen). `down` tut dasselbe.

  service <port>     Den Webserver eines Arbeiters von der 🌐 Dienste-Tafel des Office auf
                     http://localhost:<port> öffnen (über das Office; Strg-C schließt den Tunnel)
  invite <gh-user>   Ein Teammitglied mit den SSH-Schlüsseln seines GitHub-Kontos hereinlassen und den
                     einen Befehl ausgeben, den du ihm schickst. Oder: invite <name> <öffentliche-Schlüsseldatei>
  uninvite <name>    Die Schlüssel eines Teammitglieds entfernen und offene Tunnel trennen
  team               Auflisten, wer eingeladen ist
  status             Anwendung, letztes Deployment, SSH-Adresse und ob das Office läuft anzeigen
  ssh                Eine Shell im Container, als der Benutzer, der das Office betreibt
  logs               Die Logs des Office verfolgen
  reset-password     Das Passwort vergessen und ein neues einmal im Browser anzeigen

Optionen
  --name <name>             Name des Office (Standard: open-agentic-office). Mehrere Offices = mehrere Projekte
  --url <url>               Dein Dokploy, z. B. https://dokploy.example.com (Standard: $DOKPLOY_URL)
  --api-key <key>           Ein Dokploy-API-Schlüssel (Standard: $DOKPLOY_API_KEY). Lass sein Rate-Limit
                            aus: Das Skript fragt den Build alle paar Sekunden ab.
  --server <name|id>        Das Office auf einem der entfernten Server von Dokploy statt auf Dokploys
                            eigenem betreiben (bei Dokploy Cloud nötig)
  --ssh-port <n>            Port auf dem Server, der zum SSH des Office führt (Standard: 2222)
  --ssh-host <host>         Adresse, zu der du und Teammitglieder per SSH verbinden (Standard: die IP des
                            Servers in Dokploy)
  --port <n>                Lokaler Port für den Tunnel (Standard: 4600 oder der nächste freie)
  --github-token <token>    GitHub-Token für private Repos + die Issue-/PR-Tafeln
                            (Standard: dein lokales `gh auth token`)
  --no-github-token         Kein GitHub-Token ins Office legen
  --claude-token <token>    Claude-Abo-Token von `claude setup-token`
                            (Standard: $CLAUDE_CODE_OAUTH_TOKEN). Ohne meldest du dich im Terminal des
                            ersten Arbeiters im Office an.
  --anthropic-api-key <key> Stattdessen einen Anthropic-API-Schlüssel verwenden
  --no-open                 Den Browser nicht öffnen (bei up: auch den Tunnel nicht öffnen)
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
# Office schlägt `deploy/dokploy.sh --name <name> service <port>` vor).
CMD=""
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name | --url | --api-key | --server | --ssh-port | --ssh-host | --port | --github-token | --claude-token | --anthropic-api-key)
      [[ $# -ge 2 ]] || die "$1 braucht einen Wert (siehe: deploy/dokploy.sh help)" ;;
  esac
  case "$1" in
    --name) NAME="$2"; shift 2 ;;
    --url) URL_ARG="$2"; shift 2 ;;
    --api-key) API_KEY_ARG="$2"; shift 2 ;;
    --server) SERVER_ARG="$2"; shift 2 ;;
    --ssh-port) SSH_PORT_ARG="$2"; shift 2 ;;
    --ssh-host) SSH_HOST_ARG="$2"; shift 2 ;;
    --port) LOCAL_PORT="$2"; LOCAL_PORT_SET=1; shift 2 ;;
    --github-token) GH_TOKEN_ARG="$2"; shift 2 ;;
    --no-github-token) NO_GH_TOKEN=1; shift ;;
    --claude-token) CLAUDE_TOKEN="$2"; shift 2 ;;
    --anthropic-api-key) ANTHROPIC_KEY="$2"; shift 2 ;;
    --no-open) NO_OPEN=1; shift ;;
    -y | --yes) YES=1; shift ;;
    -h | --help) usage; exit 0 ;;
    -*) die "unbekannte Option $1 (siehe: deploy/dokploy.sh help)" ;;
    *)
      if [[ -z "$CMD" ]]; then CMD="$1"; else POSITIONAL+=("$1"); fi
      shift
      ;;
  esac
done
CMD="${CMD:-help}"

[[ "$NAME" =~ ^[a-zA-Z0-9-]+$ ]] || die "--name darf nur Buchstaben, Ziffern und Bindestriche enthalten"
[[ "$LOCAL_PORT" =~ ^[0-9]+$ && $LOCAL_PORT -gt 0 && $LOCAL_PORT -lt 65536 ]] || die "--port muss eine Portnummer sein"
[[ -z "$SSH_PORT_ARG" || ("$SSH_PORT_ARG" =~ ^[0-9]+$ && $SSH_PORT_ARG -gt 0 && $SSH_PORT_ARG -lt 65536) ]] || die "--ssh-port muss eine Portnummer sein"
[[ "$API_KEY_ARG" != *[[:space:]]* ]] || die "der API-Schlüssel enthält Leerzeichen"
STATE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/open-agentic-office/dokploy/$NAME"
NAME_FLAG=""
[[ "$NAME" != "open-agentic-office" ]] && NAME_FLAG=" --name $NAME"
KEY_FILE="$STATE_DIR/id_ed25519"
KNOWN_HOSTS="$STATE_DIR/known_hosts"
CLAIM_FILE="$STATE_DIR/claim-token"
API_KEY_FILE="$STATE_DIR/api-key" # "x-api-key: <Schlüssel>", von curl -H @Datei gelesen, damit der Schlüssel nicht in ps erscheint
IDS_FILE="$STATE_DIR/dokploy.env"  # DOKPLOY_URL PROJECT_ID ENVIRONMENT_ID APPLICATION_ID APP_NAME SERVER_ID VOLUME_NAME SSH_HOST SSH_PORT
DOKPLOY_URL="" PROJECT_ID="" ENVIRONMENT_ID="" APPLICATION_ID="" APP_NAME="" SERVER_ID="" VOLUME_NAME="" SSH_HOST="" SSH_PORT=""

need() { command -v "$1" >/dev/null 2>&1 || die "$1 wird gebraucht (${2:-installiere es zuerst})"; }

# Wendet einen jq-Filter auf das JSON auf stdin an und gibt das Ergebnis aus (Strings roh, Objekte
# kompakt, null gar nicht): json '.id'
json() { jq -rc "($1) | select(. != null)"; }

# Gibt das JSON aus, das ein jq-Ausdruck aus den übrigen Argumenten macht (als $v[0], $v[1] …). Sie gehen
# über die Umgebung an jq, damit Tokens nie in ps erscheinen: mkjson '{id: $v[0]}' "$ID"
mkjson() {
  local expr="$1" i=0 v
  shift
  (
    for v in "$@"; do
      export "J$i=$v"
      i=$((i + 1))
    done
    jq -nc '[$ENV | to_entries[] | select(.key | test("^J[0-9]+$"))]
      | sort_by(.key[1:] | tonumber) | map(.value) as $v | '"$expr"
  )
}

load_ids() {
  # shellcheck disable=SC1090
  [[ -f "$IDS_FILE" ]] && source "$IDS_FILE"
  [[ -z "$URL_ARG" ]] || DOKPLOY_URL="$URL_ARG"
  if [[ -n "$DOKPLOY_URL" ]]; then
    [[ "$DOKPLOY_URL" =~ ^https?:// ]] || DOKPLOY_URL="https://$DOKPLOY_URL"
    DOKPLOY_URL="${DOKPLOY_URL%/}"
  fi
  # Ein jetzt angegebener Schlüssel ersetzt den gemerkten (sobald es ein Office gibt, für das er gemerkt wird).
  if [[ -n "$API_KEY_ARG" && -d "$STATE_DIR" ]]; then
    (umask 077 && printf 'x-api-key: %s\n' "$API_KEY_ARG" >"$API_KEY_FILE")
  fi
  return 0
}

save_ids() {
  printf 'DOKPLOY_URL=%q PROJECT_ID=%q ENVIRONMENT_ID=%q APPLICATION_ID=%q APP_NAME=%q SERVER_ID=%q VOLUME_NAME=%q SSH_HOST=%q SSH_PORT=%q\n' \
    "$DOKPLOY_URL" "$PROJECT_ID" "$ENVIRONMENT_ID" "$APPLICATION_ID" "$APP_NAME" "$SERVER_ID" "$VOLUME_NAME" "$SSH_HOST" "$SSH_PORT" >"$IDS_FILE"
}

preflight() {
  need ssh
  need curl
  need jq "https://jqlang.github.io/jq/download/"
  load_ids
}

require_office() {
  [[ -n "$APPLICATION_ID" && -n "$SSH_HOST" && -n "$SSH_PORT" ]] ||
    die "es gibt kein Office namens \"$NAME\" auf Dokploy — starte: deploy/dokploy.sh up --url <dein Dokploy>$NAME_FLAG"
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

# --- Dokploy ---------------------------------------------------------------------------------------

# Ruft Dokploys API auf: api GET <Prozedur> [Query]   oder   api POST <Prozedur> <JSON-Body>
# Gibt die Antwort aus. Bei einem Fehler sagt es (auf stderr), was Dokploy gesagt hat, und scheitert:
# mit 4, wenn das Gefragte nicht da ist (ohne etwas zu sagen), mit 5, wenn die API dieses Dokploy die
# Prozedur nicht kennt, sonst mit 1.
api() {
  local method="$1" proc="$2" arg="${3:-}" out code body msg
  if [[ "$method" == GET ]]; then
    out=$(curl -sS --max-time 60 -H @"$API_KEY_FILE" -w '\n%{http_code}' "$DOKPLOY_URL/api/$proc${arg:+?$arg}" 2>&1) ||
      { warn "Dokploy unter $DOKPLOY_URL nicht erreichbar: $(head -n 1 <<<"$out")"; return 1; }
  else
    out=$(printf '%s' "$arg" | curl -sS --max-time 120 -H @"$API_KEY_FILE" -H 'Content-Type: application/json' \
      --data-binary @- -w '\n%{http_code}' "$DOKPLOY_URL/api/$proc" 2>&1) ||
      { warn "Dokploy unter $DOKPLOY_URL nicht erreichbar: $(head -n 1 <<<"$out")"; return 1; }
  fi
  code="${out##*$'\n'}"
  body="${out%$'\n'*}"
  if [[ "$code" == 2?? ]]; then
    printf '%s\n' "$body"
    return 0
  fi
  if [[ "$code" == 404 ]]; then
    [[ $(json '(try .data.path catch null) // ""' <<<"$body" 2>/dev/null) == "$proc" ]] && return 4
    warn "Dokploys API unter $DOKPLOY_URL kennt $proc nicht (ist das ein Dokploy, und ein aktuelles?)"
    return 5
  fi
  msg=$(json '.message' <<<"$body" 2>/dev/null) || msg=""
  case "$code" in
    401) msg="Dokploy hat den API-Schlüssel abgelehnt — erzeuge in Dokploy einen (Settings → Profile → API/CLI Keys) und übergib ihn mit --api-key" ;;
    429) msg="dieser API-Schlüssel hat sein Rate-Limit erreicht — erzeuge in Dokploy einen ohne Rate-Limit und übergib ihn mit --api-key" ;;
  esac
  warn "Dokploy ($proc): ${msg:-HTTP $code}"
  return 1
}

# Prüft, dass es ein Dokploy gibt, mit dem man reden kann, und einen Schlüssel, den es annimmt.
connect() {
  [[ -n "$DOKPLOY_URL" ]] || die "welches Dokploy? Übergib --url https://dokploy.example.com (oder setze DOKPLOY_URL)"
  [[ -s "$API_KEY_FILE" ]] ||
    die "kein Dokploy-API-Schlüssel: erzeuge in Dokploy einen (Settings → Profile → API/CLI Keys) und übergib ihn mit --api-key <Schlüssel> (oder setze DOKPLOY_API_KEY)"
  api GET user.get >/dev/null || die "Dokploys API unter $DOKPLOY_URL lässt sich nicht benutzen"
}

# Die Seite des Office in Dokploy: seine Deployments, deren Build-Logs, seine Umgebung und Ports.
app_url() { echo "$DOKPLOY_URL/dashboard/project/$PROJECT_ID/environment/$ENVIRONMENT_ID/services/application/$APPLICATION_ID"; }

# Ob Dokploy etwas noch hat: exists project.one projectId=<id>. Nur eine Antwort kann sagen, dass es weg
# ist: Kann Dokploy nicht gefragt werden, bricht dies ab, damit nichts neu angelegt oder vergessen wird
# bei einem Office, das noch läuft.
exists() {
  local rc=0
  api GET "$1" "$2" >/dev/null || rc=$?
  [[ $rc -eq 0 || $rc -eq 4 ]] || die "Dokploy konnte nicht fragen, ob ${2#*=} noch da ist"
  [[ $rc -eq 0 ]]
}

# Das Projekt (mit seiner Umgebung) und die Anwendung des Office: einmal angelegt, dann in $IDS_FILE
# gemerkt. Jede ID wird gespeichert, sobald es sie gibt, damit ein späterer fehlgeschlagener Schritt nie
# dazu führt, dass das nächste `up` ein zweites Office anlegt (oder `destroy` das erste verpasst).
ensure_project() {
  local out servers
  # Vor allem anderen geprüft. Es platziert nur eine neue Anwendung: eine vorhandene bleibt, wo sie ist.
  if [[ -n "$SERVER_ARG" ]]; then
    servers=$(api GET server.all) || die "Dokploys Server lassen sich nicht auflisten"
    SERVER_PICK=$(mkjson '($v[0] | fromjson) | map(select(.serverId == $v[1] or .name == $v[1]) | .serverId)' "$servers" "$SERVER_ARG" |
      json 'if length == 1 then .[0] else "" end')
    [[ -n "$SERVER_PICK" ]] ||
      die "kein Server \"$SERVER_ARG\" in Dokploy (oder mehrere mit diesem Namen); seine Server: $(json 'map(.name) | join(", ") | if . == "" then "keine" else . end' <<<"$servers")"
  fi
  if [[ -n "$PROJECT_ID" ]] && ! exists project.one "projectId=$PROJECT_ID"; then
    warn "das Dokploy-Projekt für \"$NAME\" ist weg; lege ein neues an"
    PROJECT_ID="" ENVIRONMENT_ID="" APPLICATION_ID="" APP_NAME=""
  fi
  if [[ -z "$PROJECT_ID" ]]; then
    out=$(api POST project.create "$(mkjson '{name: $v[0], description: "OpenAgenticOffice (deploy/dokploy.sh)"}' "$NAME")") ||
      die "das Dokploy-Projekt ließ sich nicht anlegen"
    PROJECT_ID=$(json '.project.projectId' <<<"$out")
    ENVIRONMENT_ID=$(json '.environment.environmentId' <<<"$out")
    [[ -n "$PROJECT_ID" && -n "$ENVIRONMENT_ID" ]] || die "Dokploy hat das Projekt angelegt, aber seine IDs nicht genannt: $out"
    save_ids
    ok "Dokploy-Projekt $NAME"
  fi
  if [[ -n "$APPLICATION_ID" ]] && ! exists application.one "applicationId=$APPLICATION_ID"; then
    # Das Volume (falls es noch da ist) wird samt Inhalt wieder eingehängt.
    warn "die Anwendung des Office in Dokploy ist weg; lege eine neue an"
    APPLICATION_ID="" APP_NAME=""
  fi
  if [[ -z "$APPLICATION_ID" ]]; then
    [[ -z "$SERVER_PICK" ]] || SERVER_ID="$SERVER_PICK"
    out=$(api POST application.create "$(mkjson '{name: "office", appName: $v[0], description: "OpenAgenticOffice (deploy/dokploy.sh)", environmentId: $v[1], serverId: (if $v[2] == "" then null else $v[2] end)}' \
      "$NAME" "$ENVIRONMENT_ID" "$SERVER_ID")") ||
      die "die Anwendung des Office ließ sich nicht anlegen$([[ -n "$SERVER_ID" ]] || echo " (bei Dokploy Cloud wähle einen deiner Server mit --server <name>)")"
    APPLICATION_ID=$(json '.applicationId' <<<"$out")
    APP_NAME=$(json '.appName' <<<"$out")
    [[ -n "$APPLICATION_ID" && -n "$APP_NAME" ]] || die "Dokploy hat die Anwendung angelegt, aber ihre ID nicht genannt: $out"
    save_ids
    ok "Anwendung office ($APP_NAME)"
  fi
}

# Wie Dokploy es baut und betreibt, wo es seine Daten ablegt und wie SSH es erreicht.
configure_app() {
  local app port want out
  app=$(api GET application.one "applicationId=$APPLICATION_ID") || die "die Anwendung des Office ließ sich nicht lesen"
  SERVER_ID=$(json '.serverId // ""' <<<"$app")
  [[ -z "$SERVER_PICK" || "$SERVER_PICK" == "$SERVER_ID" ]] ||
    warn "--server platziert nur ein neues Office; dieses bleibt, wo es ist (lösche es zuerst, um es zu verschieben)"

  # deploy/container/Dockerfile, mit dem ganzen Upload als Kontext.
  api POST application.saveBuildType "$(mkjson '{applicationId: $v[0], buildType: "dockerfile", dockerfile: $v[1], dockerContextPath: ".",
    dockerBuildStage: "", herokuVersion: null, railpackVersion: null, publishDirectory: null, isStaticSpa: null}' "$APPLICATION_ID" "$DOCKERFILE")" >/dev/null ||
    die "wie Dokploy das Office baut, ließ sich nicht festlegen"
  # Den alten Container stoppen, bevor der neue startet: Dokploys Standard startet den neuen zuerst,
  # was kurz zwei Offices auf einem Volume (und einem SSH-Port) liefe.
  api POST application.update "$(mkjson '{applicationId: $v[0],
    updateConfigSwarm: {Parallelism: 1, Order: "stop-first", FailureAction: "rollback"},
    rollbackConfigSwarm: {Parallelism: 1, Order: "stop-first"}}' "$APPLICATION_ID")" >/dev/null ||
    die "wie Dokploy den Container des Office ersetzt, ließ sich nicht festlegen"

  # Alles, was das Office aufbewahrt, kommt auf dieses Volume (siehe deploy/container/start.sh).
  if [[ $(json '.mounts | any(.mountPath == "/data")' <<<"$app") != true ]]; then
    VOLUME_NAME="${VOLUME_NAME:-$APP_NAME-data}"
    api POST mounts.create "$(mkjson '{type: "volume", volumeName: $v[0], mountPath: "/data", serviceType: "application", serviceId: $v[1]}' \
      "$VOLUME_NAME" "$APPLICATION_ID")" >/dev/null || die "das Volume ließ sich nicht hinzufügen"
    save_ids
    ok "Volume $VOLUME_NAME auf /data"
  else
    VOLUME_NAME=$(json 'first(.mounts[] | select(.mountPath == "/data") | .volumeName) // ""' <<<"$app")
  fi

  # Der sshd des Containers auf einem Port des Servers: der einzige Weg hinein. Im Host-Modus
  # veröffentlicht, direkt vom Port des Servers zum Container.
  port=$(json 'first(.ports[] | select(.targetPort == 22)) // ""' <<<"$app")
  want="${SSH_PORT_ARG:-${SSH_PORT:-$DEFAULT_SSH_PORT}}"
  if [[ -z "$port" ]]; then
    api POST port.create "$(mkjson '{publishedPort: ($v[0] | tonumber), publishMode: "host", targetPort: 22, protocol: "tcp", applicationId: $v[1]}' \
      "$want" "$APPLICATION_ID")" >/dev/null || die "SSH ließ sich nicht auf Port $want veröffentlichen"
  elif [[ $(json '.publishedPort' <<<"$port") != "$want" ]]; then
    api POST port.update "$(mkjson '{portId: ($v[0] | fromjson | .portId), publishedPort: ($v[1] | tonumber), publishMode: "host", targetPort: 22, protocol: "tcp"}' \
      "$port" "$want")" >/dev/null || die "SSH ließ sich nicht auf Port $want verschieben"
  fi
  SSH_PORT="$want"

  # Wohin du und Teammitglieder per SSH verbinden: der Server, auf dem das Office läuft.
  if [[ -n "$SSH_HOST_ARG" ]]; then
    SSH_HOST="$SSH_HOST_ARG"
  elif [[ -z "$SSH_HOST" ]]; then
    if [[ -n "$SERVER_ID" ]]; then
      SSH_HOST=$(json '.server.ipAddress? // ""' <<<"$app")
    else
      out=$(api GET settings.getIp) && SSH_HOST=$(json '.' <<<"$out")
    fi
    # Dokploys eigene Adresse, wenn es seine IP nicht kennt.
    [[ -n "$SSH_HOST" ]] || SSH_HOST=$(sed -E 's#^[a-z]+://([^/:]+).*#\1#' <<<"$DOKPLOY_URL")
  fi
  save_ids
  ok "SSH unter $SSH_HOST:$SSH_PORT"
}

# Setzt KEY=VALUE-Umgebungsvariablen an der Anwendung des Office und behält alle anderen (deine, aus
# Dokploys Environment-Reiter). Sie erreichen den Container beim nächsten Deploy oder Neustart.
set_env() {
  local app
  app=$(api GET application.one "applicationId=$APPLICATION_ID") || die "die Anwendung des Office ließ sich nicht lesen"
  api POST application.saveEnvironment "$(mkjson '
    def trim: gsub("^\\s+|\\s+$"; "");
    def key: sub("^\\s*export\\s+"; "") | split("=")[0] | trim;
    ($v[0] | fromjson) as $app | $v[1:] as $set
    | ($set | map(key)) as $names
    | (($app.env // "") | split("\n")
        | map(select(trim != "" and (key as $k | $names | any(. == $k) | not)))) as $kept
    | ($set | map(. as $kv | (index("=")) as $i | $kv[$i + 1:] as $value
        | if ($value | test("[\u0027\n]")) then error($kv[:$i] + " darf keine Anführungszeichen oder Zeilenumbrüche enthalten")
          else $kv[:$i] + "=\u0027" + $value + "\u0027" end)) as $lines
    # createEnvFile aus: Dokploy würde diese sonst als .env in den Build-Ordner schreiben.
    | {applicationId: $app.applicationId, env: (($kept + $lines) | join("\n")),
       buildArgs: ($app.buildArgs // ""), buildSecrets: ($app.buildSecrets // ""), createEnvFile: false}' \
    "$app" "$@")" >/dev/null || die "die Umgebungsvariablen des Office ließen sich nicht setzen"
}

# "<id> <status>" des letzten Deployments des Office (running, done, error, cancelled) oder "none".
latest_deployment() {
  api GET deployment.all "applicationId=$APPLICATION_ID" | json 'if length > 0 then .[0].deploymentId + " " + .[0].status else "none" end'
}

# Dieser Checkout als Zip, wie `git add -A` ihn sähe: versionierte Dateien mit deinen uncommitteten
# Änderungen und neue Dateien, die .gitignore nicht ausschließt, ohne lokale Geheimnisse (wie
# Dockerfile.dockerignore). Mit einem Hilfsindex gemacht, sodass deiner unberührt bleibt.
make_zip() {
  local dir="$STATE_DIR/upload" index
  mkdir -p "$dir"
  rm -f "$dir/index" "$dir/checkout.zip"
  (
    cd "$REPO_DIR"
    index=$(git rev-parse --path-format=absolute --git-path index)
    [[ ! -f "$index" ]] || cp "$index" "$dir/index"
    export GIT_INDEX_FILE="$dir/index"
    git add -A .
    git rm -r -q --cached --ignore-unmatch -- ':(glob)**/.env*' ':(glob)**/.claude/**' ':(glob)**/.open-agentic-office/**' \
      ':(glob)**/*.pem' ':(glob)**/*.key' ':(glob)**/id_*' ':(glob)**/pw.txt'
    git archive --format=zip -o "$dir/checkout.zip" "$(git write-tree)"
  ) >&2 || die "dieser Checkout ließ sich nicht packen"
  echo "$dir/checkout.zip"
}

# Lädt diesen Checkout hoch, damit Dokploy ihn mit deploy/container/Dockerfile baut, und wartet, bis das
# Office des neuen Containers antwortet.
deploy() {
  need git
  git -C "$REPO_DIR" rev-parse --git-dir >/dev/null 2>&1 || die "starte dies aus einem Git-Klon von open-agentic-office (es lädt die Dateien des Checkouts hoch)"
  local what zip before was out code i line id st=""
  what=$(git -C "$REPO_DIR" log -1 --format='%h %s')
  [[ -z "$(git -C "$REPO_DIR" status --porcelain)" ]] || what+=", mit deinen uncommitteten Änderungen"
  zip=$(make_zip)
  before=$(latest_deployment) || die "die Deployments des Office ließen sich nicht auflisten"
  was=$(started)
  say "Lade diesen Checkout hoch ($what, $(du -h "$zip" | cut -f1 | tr -d ' ')), damit Dokploy ihn baut — ein paar Minuten"
  out=$(curl -sS --max-time 1800 -H @"$API_KEY_FILE" -F "applicationId=$APPLICATION_ID" -F "zip=@$zip;type=application/zip" \
    -w '\n%{http_code}' "$DOKPLOY_URL/api/application.dropDeployment" 2>&1) || die "der Upload zu Dokploy ist fehlgeschlagen: $(head -n 1 <<<"$out")"
  rm -f "$zip" "${zip%/*}/index"
  code="${out##*$'\n'}"
  [[ "$code" == 2?? ]] || die "Dokploy hat den Upload nicht angenommen: $(json '.message' <<<"${out%$'\n'*}" 2>/dev/null || echo "HTTP $code")"
  for ((i = 0; i < 240; i++)); do
    line=$(latest_deployment 2>/dev/null) || line=""
    id="${line%% *}" st="${line#* }"
    if [[ -n "$line" && "$line" != none && "$id" != "${before%% *}" ]]; then
      case "$st" in
        done) break ;;
        error | cancelled)
          die "der Build $([[ $st == error ]] && echo "ist fehlgeschlagen" || echo "wurde abgebrochen") — sein Log steht im Reiter Deployments des Office in Dokploy: $(app_url)" ;;
      esac
    fi
    sleep 5
  done
  [[ "$st" == done ]] || die "der Build läuft nach 20 Minuten noch — verfolge ihn in Dokploy: $(app_url)"
  ok "Gebaut"
  say "Warte, bis das Office des neuen Containers über SSH antwortet"
  if [[ -n "$was" ]]; then wait_replaced "$was"; else wait_healthy; fi ||
    die "das Office ist nicht hochgekommen — prüfe: deploy/dokploy.sh logs$NAME_FLAG (und dass Port $SSH_PORT auf dem Server frei und in jeder Firewall davor offen ist)"
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
    [[ $rc -eq 255 ]] || return "$rc" # 255: ssh selbst ist gescheitert, der Container startet noch
    sleep 5
  done
  return 1
}

# Wann der erste Prozess des Containers gestartet ist: ändert sich, wenn Dokploy den Container ersetzt.
started() { remote 'stat -c %Y /proc/1' 2>/dev/null || true; }

# Nach einem Neustart oder Deploy: wartet auf den neuen Container, dann auf das Office darin.
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
    kill -0 "$pid" 2>/dev/null || die "der SSH-Tunnel zu $SSH_HOST:$SSH_PORT ließ sich nicht öffnen"
    curl -fs --max-time 2 "http://localhost:$port/api/health" >/dev/null 2>&1 && { up=1; break; }
    sleep 0.5
  done
  if [[ $up -ne 1 ]]; then
    kill "$pid" 2>/dev/null
    die "der Tunnel ist offen, aber das Office antwortet nicht darüber — prüfe: deploy/dokploy.sh logs$NAME_FLAG"
  fi
  ok "Dein Office: http://localhost:$port$path"
  echo "   (über SSH getunnelt nach $SSH_HOST:$SSH_PORT — lass das laufen, solange du es benutzt; Strg-C schließt es)"
  open_url "http://localhost:$port$path"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn erneut mit: deploy/dokploy.sh open$NAME_FLAG"
}

# Der Server eines Arbeiters von der 🌐 Dienste-Tafel: localhost:<port> tunnelt zum Office, das ihn über
# diesen Port weiterreicht, also muss der lokale Port zu dem des Dienstes passen.
service_tunnel() {
  local port="$1" pid i up=0
  port_busy "$port" && die "localhost:$port ist auf diesem Rechner schon belegt — stoppe zuerst, was dort läuft"
  # shellcheck disable=SC2046
  ssh $(ssh_opts) -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$SSH_HOST" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "der SSH-Tunnel zu $SSH_HOST:$SSH_PORT ließ sich nicht öffnen"
    port_busy "$port" && { up=1; break; }
    sleep 0.5
  done
  [[ $up -eq 1 ]] || { kill "$pid" 2>/dev/null; die "der Tunnel ist nicht hochgekommen"; }
  ok "Der Server des Arbeiters: http://localhost:$port"
  echo "   (über das Office — melde dich mit dem Office-Passwort an, falls es fragt; Strg-C schließt es)"
  open_url "http://localhost:$port"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn erneut mit: deploy/dokploy.sh service $port$NAME_FLAG"
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

team_members() { remote "open-agentic-office-team list"; } # "<Name> <Anzahl Schlüssel>" pro Zeile

# --- Befehle ---------------------------------------------------------------------------------------

cmd_up() {
  need ssh-keygen
  need git
  [[ -f "$REPO_DIR/$DOCKERFILE" ]] || die "starte dies aus einem Klon von open-agentic-office ($DOCKERFILE fehlt)"
  mkdir -p "$STATE_DIR"
  chmod 700 "$STATE_DIR"
  preflight
  connect

  local gh_token="$GH_TOKEN_ARG"
  if [[ -z "$gh_token" && $NO_GH_TOKEN -eq 0 ]] && command -v gh >/dev/null 2>&1; then
    gh_token=$(gh auth token 2>/dev/null || true)
  fi
  [[ $NO_GH_TOKEN -eq 1 ]] && gh_token=""

  say "OpenAgenticOffice \"$NAME\" auf Dokploy ($DOKPLOY_URL)"
  echo "   App:      dieser Checkout, hochgeladen, damit Dokploy ihn mit $DOCKERFILE baut"
  echo "   Daten:    ein Docker-Volume auf /data: Konten, Etagen, Projekte (~/workspace), Anmeldungen, Team-Schlüssel"
  echo "   Zugang:   nur SSH-Tunnel (das Office ist nie offen erreichbar), auf einem Port des Dokploy-Servers"
  if [[ -n "$gh_token" ]]; then
    echo "   GitHub:   dein GitHub-Token kommt ins Office (private Klone, Issue-/PR-Tafeln, Pushes)"
  else
    echo "   GitHub:   kein Token — private Repos und die Tafeln gehen erst, wenn gh angemeldet ist"
  fi
  if [[ -n "$CLAUDE_TOKEN" || -n "$ANTHROPIC_KEY" ]]; then
    echo "   Claude:   mit dem übergebenen Token angemeldet"
  else
    echo "   Claude:   nicht angemeldet — melde dich im Terminal des ersten Arbeiters an (oder übergib --claude-token)"
  fi

  [[ -f "$KEY_FILE" ]] || ssh-keygen -q -t ed25519 -N '' -C "open-agentic-office-$NAME" -f "$KEY_FILE"
  [[ -f "$CLAIM_FILE" ]] || (umask 077 && random_token >"$CLAIM_FILE")

  ensure_project
  configure_app

  # Dein Schlüssel bekommt eine Shell im Container; der Beanspruchungs-Link zeigt das erzeugte Passwort
  # einmal; Teammitglieder sollen per SSH zum Port des Servers verbinden; die Panels des Office
  # schlagen dieses Skript vor.
  local vars=("OAO_ADMIN_KEYS=$(cat "$KEY_FILE.pub")" "OFFICE_BEANSPRUCH_TOKEN=$(cat "$CLAIM_FILE")"
    "OFFICE_OEFFENTLICHER_HOST=$SSH_HOST:$SSH_PORT" "OFFICE_DEPLOY_BEFEHL=deploy/dokploy.sh$NAME_FLAG")
  [[ -z "$CLAUDE_TOKEN" ]] || vars+=("CLAUDE_CODE_OAUTH_TOKEN=$CLAUDE_TOKEN")
  [[ -z "$ANTHROPIC_KEY" ]] || vars+=("ANTHROPIC_API_KEY=$ANTHROPIC_KEY")
  set_env "${vars[@]}"

  deploy

  if [[ -n "$gh_token" ]]; then
    say "Melde die GitHub CLI an"
    # In der eigenen Konfiguration von gh auf dem Volume gehalten, sodass gh, git, die Tafeln und die Arbeiter es alle benutzen.
    printf '%s' "$gh_token" | remote 'gh auth login --hostname github.com --git-protocol https --with-token &&
      gh auth setup-git --hostname github.com && gh api user --jq "\"    als \" + .login"' || warn "die GitHub CLI ließ sich nicht anmelden"
  fi
  local git_name git_email
  git_name=$(git config user.name 2>/dev/null || true)
  git_email=$(git config user.email 2>/dev/null || true)
  [[ -z "$git_name" ]] || remote "git config --global user.name $(printf '%q' "$git_name")"
  [[ -z "$git_email" ]] || remote "git config --global user.email $(printf '%q' "$git_email")"

  ok "Dein Office läuft auf Dokploy (nur über SSH erreichbar)"
  echo
  echo "   Später öffnen:         deploy/dokploy.sh open$NAME_FLAG"
  echo "   Teammitglied einladen: der Knopf 👥 Team einladen im Office, oder deploy/dokploy.sh invite <github-name>$NAME_FLAG"
  echo "   Aktualisieren:         deploy/dokploy.sh update$NAME_FLAG"
  echo "   Abbauen:               deploy/dokploy.sh destroy$NAME_FLAG"
  echo "   In Dokploy:            $(app_url)"
  echo
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_open() {
  preflight
  require_office
  wait_healthy || die "das Office antwortet nicht — prüfe: deploy/dokploy.sh logs$NAME_FLAG"
  open_office
}

cmd_update() {
  preflight
  require_office
  connect
  deploy
  ok "Aktualisiert (Arbeiter, die liefen, machen dort weiter, wo sie waren)"
}

cmd_restart() {
  preflight
  require_office
  connect
  local was
  was=$(started)
  api POST application.reload "$(mkjson '{applicationId: $v[0], appName: $v[1]}' "$APPLICATION_ID" "$APP_NAME")" >/dev/null ||
    die "das Office ließ sich nicht neu starten"
  say "Starte neu"
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/dokploy.sh logs$NAME_FLAG"
  ok "Neu gestartet"
}

cmd_service() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 && "${POSITIONAL[0]}" =~ ^[0-9]+$ && ${POSITIONAL[0]} -gt 0 && ${POSITIONAL[0]} -lt 65536 ]] ||
    die "Aufruf: deploy/dokploy.sh service <port>   (ein Port von der 🌐 Dienste-Tafel des Office)"
  [[ "${POSITIONAL[0]}" -ne $OFFICE_PORT ]] || die "$OFFICE_PORT ist das Office selbst — nimm: deploy/dokploy.sh open"
  require_office
  service_tunnel "${POSITIONAL[0]}"
}

cmd_status() {
  preflight
  if [[ -z "$APPLICATION_ID" ]]; then
    echo "Es gibt kein Office namens \"$NAME\" auf Dokploy."
    return
  fi
  connect
  local app deployment
  app=$(api GET application.one "applicationId=$APPLICATION_ID") || die "die Anwendung des Office ließ sich nicht lesen"
  deployment=$(latest_deployment) || deployment=""
  echo "Office:      $NAME (Dokploy-Anwendung $APP_NAME$([[ -z "$SERVER_ID" ]] || echo ", auf $(json '.server.name?' <<<"$app")"))"
  echo "Dokploy:     $(app_url)"
  echo "Deployment:  ${deployment#* } (Anwendung: $(json '.applicationStatus' <<<"$app"))"
  echo "SSH:         $SSH_HOST:$SSH_PORT  (Office öffnen mit: deploy/dokploy.sh open$NAME_FLAG)"
  if [[ -f "$KEY_FILE" ]] && office_get /api/health >/dev/null 2>&1; then
    echo "Volume:      $VOLUME_NAME: $(remote "df -h /data | awk 'NR == 2 {print \$3 \" belegt von \" \$2}'")"
    echo "Office:      läuft"
    local team
    team=$(team_members 2>/dev/null | awk '{printf "%s%s", sep, $1; sep=", "}')
    echo "Team:        ${team:-noch niemand eingeladen}"
  else
    echo "Volume:      ${VOLUME_NAME:-keins}"
    echo "Office:      antwortet nicht"
  fi
}

cmd_invite() {
  preflight
  [[ ${#POSITIONAL[@]} -ge 1 && ${#POSITIONAL[@]} -le 2 ]] ||
    die "Aufruf: deploy/dokploy.sh invite <github-name>   oder   deploy/dokploy.sh invite <name> <öffentliche-Schlüsseldatei>"
  local who="${POSITIONAL[0]}" src raw
  valid_member "$who"
  if [[ ${#POSITIONAL[@]} -eq 2 ]]; then
    src="${POSITIONAL[1]}"
    [[ -f "$src" ]] || die "keine solche Datei: $src"
    raw=$(cat "$src")
  else
    src="github.com/$who.keys"
    raw=$(curl -fsS --max-time 10 "https://github.com/$who.keys") || die "https://$src ließ sich nicht abrufen"
  fi
  [[ -n "$raw" ]] || die "keine öffentlichen SSH-Schlüssel in $src gefunden"
  require_office
  local n fp
  # Der Container behält nur gültige Schlüssel und beschränkt jeden darauf, den Tunnel zu öffnen.
  n=$(printf '%s\n' "$raw" | remote "open-agentic-office-team add $who") || die "die Schlüssel von $who aus $src ließen sich nicht hinzufügen"
  ok "$who ist eingeladen ($n Schlüssel aus $src)"
  fp=$(remote "open-agentic-office-team fingerprint")
  echo
  echo "   Schick $who das hier:"
  echo
  echo "     ssh -L 4600:localhost:$OFFICE_PORT ssh://$TEAM_USER@$SSH_HOST:$SSH_PORT"
  echo
  echo "     Lass es laufen, öffne http://localhost:4600 und melde dich mit dem Office-Passwort an."
  echo "     Beim ersten Mal fragt ssh, ob du dem Server vertraust. Sag nur Ja, wenn er"
  echo "     ED25519 key fingerprint $fp zeigt"
  echo
}

cmd_uninvite() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/dokploy.sh uninvite <name>"
  local who="${POSITIONAL[0]}" out rc=0
  valid_member "$who"
  require_office
  out=$(remote "open-agentic-office-team remove $who" 2>&1) || rc=$?
  [[ $rc -eq 66 ]] && die "$who ist nicht eingeladen (siehe: deploy/dokploy.sh team$NAME_FLAG)"
  [[ $rc -eq 0 ]] || die "die Schlüssel ließen sich nicht entfernen: $out"
  ok "Die Schlüssel von $who sind entfernt und offene Tunnel getrennt (andere Teammitglieder verbinden einfach neu)"
  echo "   Sie kennen noch das Office-Passwort. Zum Ändern: deploy/dokploy.sh reset-password$NAME_FLAG"
}

cmd_team() {
  preflight
  require_office
  local list
  list=$(team_members)
  if [[ -z "$list" ]]; then
    echo "Noch niemand ist eingeladen. Lade jemanden ein: deploy/dokploy.sh invite <github-name>$NAME_FLAG"
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

# Die Ausgabe des Containers, von Dokploy (das nicht mehr hält als Docker): die letzten 100 Zeilen,
# dann alles, was danach kommt, bis Strg-C.
cmd_logs() {
  preflight
  require_office
  connect
  local out last="" since=""
  while :; do
    out=$(api GET application.readLogs "applicationId=$APPLICATION_ID${since:+&since=$since}") || out='""'
    # Jede Zeile beginnt mit Dockers Zeitstempel (feste Breite, also sortieren sie sich als Text).
    out=$(mkjson '($v[0] | fromjson | split("\n") | map(select(. != "" and (split(" ")[0] > $v[1]))) | join("\n"))' "$out" "$last" | json '.')
    if [[ -n "$out" ]]; then
      printf '%s\n' "$out"
      last=$(tail -n 1 <<<"$out" | cut -d' ' -f1)
    fi
    since=1m
    sleep 2
  done
}

cmd_reset_password() {
  preflight
  require_office
  connect
  (umask 077 && random_token >"$CLAIM_FILE")
  say "Setze das Office-Passwort zurück"
  set_env "OFFICE_BEANSPRUCH_TOKEN=$(cat "$CLAIM_FILE")"
  local was
  was=$(started)
  remote "/opt/open-agentic-office/open-agentic-office --daten ~/open-agentic-office --passwort-zuruecksetzen >/dev/null" || die "Zurücksetzen fehlgeschlagen"
  # Ein Neustart, damit das Office mit dem neuen Beanspruchungs-Token startet (und das neue Passwort erzeugt).
  api POST application.reload "$(mkjson '{applicationId: $v[0], appName: $v[1]}' "$APPLICATION_ID" "$APP_NAME")" >/dev/null ||
    die "das Office ließ sich nicht neu starten"
  wait_replaced "$was" || die "das Office ist nicht zurückgekommen — prüfe: deploy/dokploy.sh logs$NAME_FLAG"
  ok "Alle sind abgemeldet"
  open_office
}

cmd_down() {
  preflight
  if [[ -z "$PROJECT_ID" && -z "$APPLICATION_ID" ]]; then
    echo "Für \"$NAME\" gibt es nichts zu löschen."
    rm -rf "$STATE_DIR"
    return
  fi
  connect
  # Das Zustandsverzeichnis (SSH-Schlüssel, Beanspruchungs-Token, API-Schlüssel und IDs dieses Office)
  # verschwindet erst, wenn das Office weg ist: exists bricht ab, wenn Dokploy es nicht sagen kann, und
  # ein fehlgeschlagenes Löschen hält hier an.
  local have_app=0 i rc out others
  [[ -n "$APPLICATION_ID" ]] && exists application.one "applicationId=$APPLICATION_ID" && have_app=1
  say "Das löscht das Office \"$NAME\" endgültig aus $DOKPLOY_URL: die Anwendung${VOLUME_NAME:+, das Volume $VOLUME_NAME} und alles darauf."
  echo "   Alles im Office, was nicht auf GitHub gepusht ist, geht verloren."
  if [[ $YES -ne 1 ]]; then
    read -r -p "   Tippe zur Bestätigung den Namen des Office ($NAME): " answer
    [[ "$answer" == "$NAME" ]] || die "abgebrochen"
  fi
  if [[ $have_app -eq 1 ]]; then
    api POST application.delete "$(mkjson '{applicationId: $v[0]}' "$APPLICATION_ID")" >/dev/null || die "die Anwendung des Office ließ sich nicht löschen"
    ok "Anwendung gelöscht (ihr Container ist gestoppt und entfernt)"
  else
    ok "Die Anwendung des Office ist schon gelöscht"
  fi
  APPLICATION_ID=""
  save_ids

  # Dokploy lässt die Volumes einer Anwendung zurück. Es ist in Benutzung, bis der Container weg ist.
  if [[ -n "$VOLUME_NAME" ]]; then
    for ((i = 0; i < 24; i++)); do
      rc=0
      out=$(api POST dockerVolume.removeVolume "$(mkjson '{volumeName: $v[0]} + (if $v[1] == "" then {} else {serverId: $v[1]} end)' "$VOLUME_NAME" "$SERVER_ID")" 2>&1) || rc=$?
      [[ $rc -eq 0 || $rc -eq 4 || $rc -eq 5 || "$out" == *"no such volume"* ]] && break
      sleep 5
    done
    if [[ $rc -eq 0 || $rc -eq 4 || "$out" == *"no such volume"* ]]; then
      ok "Volume $VOLUME_NAME gelöscht"
    elif [[ $rc -eq 5 ]]; then
      warn "dieses Dokploy kann Volumes nicht über seine API löschen. Führe auf dem Server aus: docker volume rm $VOLUME_NAME"
    else
      die "das Volume $VOLUME_NAME ließ sich nicht löschen: ${out:-keine Antwort} (starte dies erneut, um es nochmal zu versuchen)"
    fi
  fi

  # Das Projekt auch, außer du hast etwas anderes hineingelegt.
  if [[ -n "$PROJECT_ID" ]] && exists project.one "projectId=$PROJECT_ID"; then
    others=$(api GET project.one "projectId=$PROJECT_ID" |
      json '[.environments[] as $e | ("applications", "compose", "libsql", "mariadb", "mongo", "mysql", "postgres", "redis") as $k | ($e[$k] // []) | length] | add // 0') ||
      die "das Dokploy-Projekt ließ sich nicht lesen"
    if [[ "$others" -eq 0 ]]; then
      api POST project.remove "$(mkjson '{projectId: $v[0]}' "$PROJECT_ID")" >/dev/null || die "das Dokploy-Projekt ließ sich nicht löschen"
      ok "Dokploy-Projekt $NAME gelöscht"
    else
      warn "das Dokploy-Projekt \"$NAME\" bleibt bestehen: Es enthält $others weitere(n) Dienst(e)"
    fi
  fi
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
  *) die "unbekannter Befehl \"$CMD\" (siehe: deploy/dokploy.sh help)" ;;
esac
