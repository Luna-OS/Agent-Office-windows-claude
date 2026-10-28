#!/usr/bin/env bash
# Stelle dein eigenes OpenAgenticOffice mit einem Befehl auf Azure bereit, nur mit der Azure CLI.
#
#   deploy/azure.sh up        die VM anlegen, das Office installieren, starten und öffnen
#   deploy/azure.sh open      Tunnel zum Office aufbauen und im Browser öffnen
#   deploy/azure.sh pause     die VM freigeben (deallocate), um Geld zu sparen (fragt vorher)
#   deploy/azure.sh resume    wieder starten
#   deploy/azure.sh destroy   alles löschen, was angelegt wurde (fragt vorher)
#
# Alles liegt in einer eigenen Ressourcengruppe (open-agentic-office oder open-agentic-office-<name>), sodass
# destroy ein einziges `az group delete` ist. Das Office ist nie im Internet: Es hört auf dem Loopback
# der VM, und alle erreichen es über einen SSH-Tunnel. `deploy/azure.sh help` zeigt alle Befehle und
# Optionen. Es ist deploy/aws.sh für Azure: dieselben Befehle, dasselbe deploy/provision.sh.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NAME="open-agentic-office"
# 4 vCPU und 16 GiB wie die t3.xlarge von deploy/aws.sh, zu etwa demselben Preis. Nicht die burstbare
# B4s_v2: Eine t3 auf AWS bursted standardmäßig unbegrenzt, während eine VM der B-Serie, der die
# CPU-Guthaben ausgehen (unter einer Stunde mit beschäftigten Arbeitern), auf 40 % gedrosselt wird.
DEFAULT_SIZE="Standard_D4as_v5"
SIZE="$DEFAULT_SIZE"
SIZE_SET=0
DISK_GB=64 # Premium SSD wird nach Stufe abgerechnet, und alles von 33 bis 64 GiB ist dieselbe P6-Stufe
LOCATION_ARG=""
SUBSCRIPTION_ARG=""
APP_REF="main"
APP_REPO=""
PROJECT=""
GH_TOKEN_ARG=""
NO_GH_TOKEN=0
CLAUDE_TOKEN="${CLAUDE_CODE_OAUTH_TOKEN:-}"
ANTHROPIC_KEY=""
YES=0
NO_OPEN=0
EXTRA_ALLOW=()
SSH_USER="azureuser"
TEAM_USER="office"   # die Schlüssel der Teammitglieder melden sich als dieser Benutzer an, der nur zum Office tunneln darf
OFFICE_PORT=4600     # wo das Office auf der VM hört (nur 127.0.0.1)
LOCAL_PORT=4600
LOCAL_PORT_SET=0
SSH_RULE="open-agentic-office-ssh" # die eine eingehende Regel der Netzwerksicherheitsgruppe

usage() {
  cat <<'EOF'
OpenAgenticOffice auf Azure — ein Befehl hoch, ein Befehl runter.

Aufruf: deploy/azure.sh <Befehl> [Optionen]

Das Office ist nie im Internet. Es hört auf dem Loopback der VM, die Firewall öffnet nur SSH,
und alle erreichen das Office über einen SSH-Tunnel auf http://localhost:4600.

Befehle
  up                 Dein Office auf einer Azure-VM anlegen (oder weiterverwenden), installieren,
                     starten und im Browser öffnen. Die erste Seite zeigt das Office-Passwort
                     EINMAL — schreib es auf.
  open               Zu deinem Office tunneln und es im Browser öffnen (Strg-C schließt den Tunnel)
  pause              Die VM freigeben (deallocate), um Geld zu sparen (fragt vorher). Platte, Adresse
                     und alles darauf bleiben; nur Platte und Adresse kosten, solange pausiert ist
  resume             Ein pausiertes Office wieder starten und im Browser öffnen
  destroy            Die Ressourcengruppe des Office und alles darin löschen (du tippst vorher den
                     Office-Namen). `down` tut dasselbe.
  connect            Diesen Computer ein Office verwalten lassen, das auf einem anderen angelegt
                     wurde: legt den SSH-Schlüssel dieses Computers auf die VM und seine IP in die
                     Firewall, sonst nichts

  service <port>     Den Webserver eines Arbeiters von der 🌐 Dienste-Tafel des Office auf
                     http://localhost:<port> öffnen (über das Office; Strg-C schließt den Tunnel)
  invite <gh-user>   Ein Teammitglied mit den SSH-Schlüsseln seines GitHub-Kontos hereintunneln
                     lassen und den einen Befehl ausgeben, den du ihm schickst.
                     Oder: invite <name> <public-key-datei>
  uninvite <name>    Die Schlüssel eines Teammitglieds entfernen und offene Tunnel kappen
  team               Auflisten, wer eingeladen ist
  status             Die VM zeigen, ob das Office läuft und welche IPs per SSH hinein dürfen
  allow <ip|me>      Einer IP (oder einem CIDR) SSH erlauben. "me" = deine aktuelle IP. "anywhere"
                     öffnet SSH für alle IPs — vertretbar, da nur dein Schlüssel und eingeladene
                     Schlüssel angenommen werden
  revoke <ip|me>     Diesen Zugang wieder entziehen
  ssh                Per SSH auf die VM
  logs               Die Logs des Office verfolgen
  resize <größe>     Die VM-Größe ändern, z. B. Standard_D8as_v5 (hält sie für ein paar Minuten
                     an; die Adresse bleibt gleich). `up --size <größe>` tut das auch.
  update             Das neueste open-agentic-office auf der VM installieren und neu starten
  reset-password     Das Passwort vergessen und im Browser einmal ein neues anzeigen

Optionen
  --name <name>             Name der Bereitstellung, mit dem du mehrere Offices betreiben kannst
                            (Standard: open-agentic-office). Jedes liegt in einer eigenen
                            Ressourcengruppe: open-agentic-office-<name>
  --location <region>       Azure-Region für ein neues Office, z. B. westeurope (Standard: die
                            Standard-Location deiner az CLI, sonst eastus). Ein Office bleibt in der
                            Region, in der es angelegt wurde. --region geht auch
  --subscription <id|name>  Azure-Abonnement (Standard: das aktuelle der az CLI)
  --size <vm-größe>         VM-Größe (Standard: Standard_D4as_v5 — 4 vCPU, 16 GiB). --instance-type
                            geht auch
  --disk <GiB>              Größe der OS-Platte, Premium SSD (Standard: 64)
  --allow <ip|cidr>         Mit up oder invite: dieser IP ebenfalls SSH erlauben (wiederholbar).
                            Deine eigene IP ist immer erlaubt.
  --port <n>                Lokaler Port für den Tunnel (Standard: 4600, sonst der nächste freie)
  --project <owner/repo>    Dieses GitHub-Repo zusätzlich als erste Etage des Office klonen. Ohne
                            diese Option öffnet sich das Office im Aufzug, der jedes Repo auflistet,
                            das dein GitHub-Token sieht: wähl dort eines. Projekte liegen in
                            ~/workspace auf der VM
  --app-repo <url>          Das open-agentic-office-Repo, das installiert wird (Standard: das
                            GitHub-Origin dieses Checkouts)
  --app-ref <ref>           Branch oder Tag zum Installieren (Standard: main)
  --github-token <token>    GitHub-Token für private Repos und die Issue-/PR-Tafeln
                            (Standard: dein lokales `gh auth token`)
  --no-github-token         Kein GitHub-Token auf die VM legen
  --claude-token <token>    Claude-Abo-Token von `claude setup-token`
                            (Standard: $CLAUDE_CODE_OAUTH_TOKEN). Ohne einen meldest du dich im
                            Terminal des ersten Arbeiters im Office an.
  --anthropic-api-key <key> Stattdessen einen Anthropic-API-Schlüssel verwenden
  --no-open                 Den Browser nicht öffnen (bei up, resume: auch den Tunnel nicht öffnen)
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

# Das erste Wort, das keine Option ist, ist der Befehl; Optionen dürfen davor oder danach stehen.
CMD=""
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name | --location | --region | --subscription | --size | --instance-type | --disk | --allow | --port | --project | \
      --app-repo | --app-ref | --github-token | --claude-token | --anthropic-api-key)
      [[ $# -ge 2 ]] || die "$1 braucht einen Wert (siehe: deploy/azure.sh help)" ;;
  esac
  case "$1" in
    --name) NAME="$2"; shift 2 ;;
    --location | --region) LOCATION_ARG="$2"; shift 2 ;;
    --subscription) SUBSCRIPTION_ARG="$2"; shift 2 ;;
    --size | --instance-type) SIZE="$2"; SIZE_SET=1; shift 2 ;;
    --disk) DISK_GB="$2"; shift 2 ;;
    --allow) EXTRA_ALLOW+=("$2"); shift 2 ;;
    --port) LOCAL_PORT="$2"; LOCAL_PORT_SET=1; shift 2 ;;
    --project) PROJECT="$2"; shift 2 ;;
    --app-repo) APP_REPO="$2"; shift 2 ;;
    --app-ref) APP_REF="$2"; shift 2 ;;
    --github-token) GH_TOKEN_ARG="$2"; shift 2 ;;
    --no-github-token) NO_GH_TOKEN=1; shift ;;
    --claude-token) CLAUDE_TOKEN="$2"; shift 2 ;;
    --anthropic-api-key) ANTHROPIC_KEY="$2"; shift 2 ;;
    --no-open) NO_OPEN=1; shift ;;
    -y | --yes) YES=1; shift ;;
    -h | --help) usage; exit 0 ;;
    -*) die "unbekannte Option $1 (siehe: deploy/azure.sh help)" ;;
    *)
      if [[ -z "$CMD" ]]; then CMD="$1"; else POSITIONAL+=("$1"); fi
      shift
      ;;
  esac
done
CMD="${CMD:-help}"
# Die Namen von Azure-Ressourcengruppen ignorieren Groß-/Kleinschreibung, also auch der Name des Office.
NAME=$(printf '%s' "$NAME" | tr '[:upper:]' '[:lower:]')

# Daraus werden Ressourcennamen gebaut, und der Name einer Linux-VM darf nicht mit einem Bindestrich beginnen oder enden.
[[ "$NAME" =~ ^[a-zA-Z0-9]([a-zA-Z0-9-]{0,38}[a-zA-Z0-9])?$ ]] ||
  die "--name darf nur Buchstaben, Ziffern und Bindestriche enthalten (nicht am Anfang oder Ende), höchstens 40"
[[ "$LOCAL_PORT" =~ ^[0-9]+$ && $LOCAL_PORT -gt 0 && $LOCAL_PORT -lt 65536 ]] || die "--port muss eine Portnummer sein"
[[ "$DISK_GB" =~ ^[0-9]+$ && $DISK_GB -ge 30 && $DISK_GB -le 4095 ]] || die "--disk muss eine Größe in GiB von 30 bis 4095 sein"
[[ "$SIZE" =~ ^[A-Za-z0-9_]+$ ]] || die "keine VM-Größe: $SIZE (z. B. $DEFAULT_SIZE)"
RESOURCE="open-agentic-office-$NAME"
[[ "$NAME" == "open-agentic-office" ]] && RESOURCE="open-agentic-office"
RG="$RESOURCE"
VM="$RESOURCE"
NSG="$RESOURCE-nsg"
PIP="$RESOURCE-ip"
VNET="$RESOURCE-vnet"
STATE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/open-agentic-office/azure/$NAME"
NAME_FLAG=""
[[ "$NAME" != "open-agentic-office" ]] && NAME_FLAG=" --name $NAME"
# RSA, das jeder Azure-Pfad nimmt (ED25519 wird erst neuer unterstützt, und nicht überall).
KEY_FILE="$STATE_DIR/id_rsa"
KNOWN_HOSTS="$STATE_DIR/known_hosts"
CLAIM_FILE="$STATE_DIR/claim-token"
# Das Abonnement, in dem `up` das Office angelegt hat, damit spätere Befehle ohne --subscription dort suchen.
SUB_FILE="$STATE_DIR/subscription"

need() { command -v "$1" >/dev/null 2>&1 || die "$1 wird gebraucht (${2:-bitte zuerst installieren})"; }
lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }

# Die Azure CLI, auf ein Abonnement festgelegt, ohne Warnungen und ohne das \r, das sie unter
# Windows ausgibt. Nicht für `az account list-locations` oder `az config`, die kein --subscription nehmen.
SUB_ARGS=()
azc() { az "$@" "${SUB_ARGS[@]+"${SUB_ARGS[@]}"}" --only-show-errors | tr -d '\r'; }
# Ein Wert. az gibt für einen fehlenden in einer Liste None aus, für einen einzelnen fehlenden nichts.
azv() { azc "$@" -o tsv | sed 's/^None$//'; }
# Die tsv-Zeilen von az als durch Leerzeichen getrennte Wörter, mit - für einen fehlenden Wert, damit `read` jeden an seinem Platz behält.
words() { awk -F'\t' '{ for (i = 1; i <= NF; i++) { gsub(/ /, "", $i); if ($i == "" || $i == "None") $i = "-" } $1 = $1; print }'; }

preflight() {
  need az "https://learn.microsoft.com/cli/azure/install-azure-cli"
  need ssh
  need curl
  [[ -z "$SUBSCRIPTION_ARG" && -s "$SUB_FILE" ]] && SUBSCRIPTION_ARG=$(cat "$SUB_FILE")
  [[ -n "$SUBSCRIPTION_ARG" ]] && SUB_ARGS=(--subscription "$SUBSCRIPTION_ARG")
  # Ein Token abzuholen erneuert die Anmeldung, also scheitert eine abgelaufene hier und nicht mittendrin.
  azc account get-access-token -o none 2>/dev/null ||
    die "die Azure CLI ist nicht angemeldet${SUBSCRIPTION_ARG:+ beim Abonnement $SUBSCRIPTION_ARG} (versuch: az login)"
  local row
  row=$(azc account show --query '[[id, name]]' -o tsv 2>/dev/null) || die "konnte dein Azure-Abonnement nicht lesen (versuch: az login)"
  SUB_ID="${row%%$'\t'*}"
  SUB_NAME="${row#*$'\t'}"
  # Ab hier per ID, damit jeder Aufruf dasselbe Abonnement trifft, auch wenn sich der Standard ändert.
  SUB_ARGS=(--subscription "$SUB_ID")
}

my_ip() {
  local ip
  ip=$(curl -4 -fsS --max-time 10 https://checkip.amazonaws.com 2>/dev/null || curl -4 -fsS --max-time 10 https://api.ipify.org) || return 1
  printf '%s' "$ip" | tr -d '[:space:]'
}

to_cidr() {
  local v="$1"
  [[ "$v" == "me" ]] && { v=$(my_ip) || die "konnte deine öffentliche IP nicht ermitteln"; }
  [[ "$v" == "anywhere" ]] && v="0.0.0.0/0"
  [[ "$v" == */* ]] || v="$v/32"
  [[ "$v" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || die "keine IPv4-Adresse und kein CIDR: $1"
  echo "$v"
}

random_token() { od -An -N24 -tx1 /dev/urandom | tr -d ' \n'; }

open_url() {
  local url="$1"
  if [[ $NO_OPEN -eq 1 ]]; then return; fi
  if command -v open >/dev/null 2>&1; then open "$url"
  elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$url" >/dev/null 2>&1 &
  elif command -v wslview >/dev/null 2>&1; then wslview "$url"
  elif command -v cmd.exe >/dev/null 2>&1; then MSYS2_ARG_CONV_EXCL='*' cmd.exe /c start "" "$url" # Git Bash würde aus /c ein C:/ machen
  fi
}

confirm() {
  [[ $YES -eq 1 ]] && return
  local answer
  read -r -p "   Fortfahren? [j/N] " answer
  [[ "$answer" =~ ^[JjYy] ]] || die "abgebrochen"
}

# --- Azure-Abfragen -----------------------------------------------------------------------------

# Setzt LOCATION auf die Region der Ressourcengruppe des Office ('' wenn es noch keine gibt), nachdem
# sichergestellt ist, dass dieses Skript sie angelegt hat: destroy löscht die ganze Gruppe.
load_group() {
  local exists row tag
  LOCATION=""
  exists=$(azv group exists -n "$RG") || die "konnte die Ressourcengruppe $RG nicht nachschlagen (siehe oben)"
  [[ "$exists" == "true" ]] || return 0
  row=$(azc group show -n "$RG" --query '[[location, tags."open-agentic-office"]]' -o tsv | words) ||
    die "konnte die Ressourcengruppe $RG nicht lesen (siehe oben)"
  read -r LOCATION tag <<<"$row"
  [[ "$tag" == "$NAME" ]] ||
    die "die Ressourcengruppe $RG wurde nicht von deploy/azure.sh angelegt (sie hat kein Tag open-agentic-office=$NAME) — wähl einen anderen --name"
}

require_group() {
  load_group
  [[ -n "$LOCATION" ]] || die "kein Office namens \"$NAME\" im Abonnement $SUB_NAME — führ aus: deploy/azure.sh up$NAME_FLAG"
}

default_location() {
  local l="${AZURE_DEFAULTS_LOCATION:-}"
  [[ -n "$l" ]] || l=$(az config get defaults.location --query value -o tsv 2>/dev/null | tr -d '\r' || true)
  echo "${l:-eastus}"
}

# "West Europe" und "westeurope" sind dieselbe Region; Azures Name dafür ist der zweite.
normalize_location() { lower "$1" | tr -d ' '; }

# Die VM-ID des Office oder nichts, wenn es keine hat. Eine Liste statt `vm show`, damit jeder andere
# Fehler das Skript stoppt, statt als "keine VM" zu gelten.
find_vm() { azv vm list -g "$RG" --query "[?name=='$VM'].id | [0]"; }
vm_size() { azv vm show -g "$RG" -n "$VM" --query hardwareProfile.vmSize; }

require_vm_exists() {
  VM_ID=$(find_vm)
  [[ -n "$VM_ID" ]] || die "das Office \"$NAME\" hat keine VM — führ aus: deploy/azure.sh up$NAME_FLAG"
}

# Eine Netzwerkressource des Office nach Art (nsg, public-ip) und Name: ihre ID oder nichts. Auch hier
# stoppt ein Fehler das Skript: Ihn für "fehlt" zu halten würde neu anlegen und damit zurücksetzen, was da ist.
find_resource() { azv network "$1" list -g "$RG" --query "[?name=='$2'].id | [0]"; }

# running, starting, stopping, stopped (aus, wird aber noch berechnet), deallocating oder deallocated (pausiert).
vm_power() {
  azv vm get-instance-view -g "$RG" -n "$VM" --query "instanceView.statuses[?starts_with(code, 'PowerState/')].code | [0]" 2>/dev/null |
    sed 's#^PowerState/##' || true
}

# Der Energiezustand, sobald er sich nicht mehr ändert (ein laufender Start, Stopp oder Deallocate).
# Gibt nach 10 Minuten auf, oder nach 30 Sekunden, in denen Azure nichts sagt.
settled_power() {
  local p="" i blank=0
  for ((i = 0; i < 120; i++)); do
    p=$(vm_power)
    case "$p" in
      starting | stopping | deallocating) ;;
      "")
        blank=$((blank + 1))
        [[ $blank -lt 6 ]] || break
        ;;
      *) break ;;
    esac
    [[ $i -eq 0 ]] && say "Warte auf die VM, die ${p:-beschäftigt} ist" >&2
    sleep 5
  done
  echo "$p"
}

# Eine pausierte (deallocated oder stopped) VM starten; eine laufende bleibt unberührt.
start_vm() {
  [[ "$(settled_power)" == "running" ]] && return 0
  say "Starte $VM"
  azc vm start -g "$RG" -n "$VM" -o none
}

public_ip() { azv network public-ip show -g "$RG" -n "$PIP" --query ipAddress 2>/dev/null || true; }

# "<name> <arch> <generationen> <ohne Trusted Launch> <Premium SSD> <Temp-Platte MB> <Platten-Controller>"
# für eine Größe, die dieses Abonnement in $LOCATION nutzen kann (- wo Azure es nicht sagt), oder
# nichts, wenn nicht. Scheitert, wenn Azure die Größen gar nicht auflisten kann.
size_row() {
  local q="[].[name" c out
  for c in CpuArchitectureType HyperVGenerations TrustedLaunchDisabled PremiumIO MaxResourceVolumeMB DiskControllerTypes; do
    q+=", capabilities[?name=='$c'] | [0].value"
  done
  out=$(azc vm list-skus -l "$LOCATION" --resource-type virtualMachines --size "$1" -o tsv --query "$q]") || return 1
  printf '%s\n' "$out" | awk -F'\t' -v w="$1" 'tolower($1) == tolower(w) { print; exit }' | words
}

# Setzt SIZE auf die eigene Schreibweise der Größe und SIZE_ARCH, SIZE_GENS, SIZE_NO_TL, SIZE_PREMIUM,
# SIZE_TEMP und SIZE_CONTROLLERS auf das, was sie hat (siehe size_row).
size_info() {
  local want="$1" row
  [[ "$want" =~ ^[A-Za-z0-9_]+$ ]] || die "keine VM-Größe: $want (z. B. $DEFAULT_SIZE)"
  say "Prüfe, ob $want in $LOCATION verfügbar ist"
  row=$(size_row "$want") || die "konnte die VM-Größen in $LOCATION nicht auflisten (siehe oben)"
  [[ -n "$row" ]] || die "$want ist für dein Abonnement in $LOCATION nicht verfügbar. Prüf den Namen der Region
   (az account list-locations -o table) oder wähl eine andere --size oder --location. Die Größen dort:
   az vm list-skus -l $LOCATION --resource-type virtualMachines --size Standard_D4 -o table"
  read -r SIZE SIZE_ARCH SIZE_GENS SIZE_NO_TL SIZE_PREMIUM SIZE_TEMP SIZE_CONTROLLERS <<<"$row"
}

# Ob eine Größe eine lokale Temp-Platte hat: Azure ändert die Größe nur zwischen Größen, die beide eine haben oder beide keine.
temp_disk() { [[ "$1" != "-" && "$1" != "0" ]]; }

# Ob die Größe in SIZE einen Platten-Controller (SCSI oder NVMe) nimmt. Größen, die nichts sagen, nehmen SCSI.
size_takes() {
  if [[ "$SIZE_CONTROLLERS" == "-" ]]; then [[ "$1" == "SCSI" ]]; else [[ ",$SIZE_CONTROLLERS," == *",$1,"* ]]; fi
}

# Deallocate -> Größe ändern -> starten. Platte, Adresse und alles auf der VM bleiben. Die Änderungen,
# die Azure bekanntermaßen ablehnt, werden vor dem Anhalten abgefangen, und wenn Azure sie nicht als
# neue Größe starten kann (gerade kein Platz für diese Größe in der Region), geht es auf die alte
# zurück. Ein pausiertes Office bleibt pausiert, in der neuen Größe: dann ist RESIZED_PAUSED 1.
resize_vm() {
  local have was paused=0 image security disk controller h_arch h_temp had_temp=0 has_temp=0
  RESIZED_PAUSED=0
  have=$(vm_size)
  was=$(settled_power)
  if [[ "$was" == "deallocated" || "$was" == "stopped" ]]; then paused=1; fi
  if [[ "$(lower "$have")" == "$(lower "$1")" ]]; then
    ok "Schon eine $have"
    RESIZED_PAUSED=$paused
    return
  fi
  size_info "$1"
  read -r image security disk controller <<<"$(azc vm show -g "$RG" -n "$VM" -o tsv --query \
    '[[storageProfile.imageReference.sku, securityProfile.securityType, storageProfile.osDisk.managedDisk.storageAccountType, storageProfile.diskControllerType]]' |
    words)"
  case "$image" in
    *arm64*) h_arch=Arm64 ;;
    *) h_arch=x64 ;;
  esac
  [[ "$SIZE_ARCH" == "$h_arch" ]] || die "die CPU-Architektur lässt sich nicht wechseln ($have ist $h_arch, $SIZE ist $SIZE_ARCH) — nimm stattdessen destroy + up"
  if [[ "$image" == *gen1* ]]; then
    [[ "$SIZE_GENS" == *V1* ]] || die "$SIZE kann das Gen1-Image dieser VM nicht ausführen — nimm stattdessen destroy + up"
  else
    [[ "$SIZE_GENS" == *V2* ]] || die "$SIZE kann das Gen2-Image dieser VM nicht ausführen — nimm stattdessen destroy + up"
  fi
  [[ "$security" == "TrustedLaunch" && "$SIZE_NO_TL" == "True" ]] &&
    die "$SIZE unterstützt kein Trusted Launch, das diese VM benutzt — wähl eine andere Größe oder destroy + up"
  [[ "$disk" == Premium* && "$SIZE_PREMIUM" != "True" ]] &&
    die "$SIZE kann die Premium SSD dieser VM nicht aufnehmen — wähl eine Größe mit einem s nach der Zahl, wie $DEFAULT_SIZE"
  [[ "$controller" == "-" ]] && controller="SCSI"
  size_takes "$controller" || die "$SIZE nimmt den Platten-Controller $controller dieser VM nicht — wähl eine andere Größe oder destroy + up"
  h_temp=$(size_row "$have" | awk '{ print $6 }') || h_temp=""
  if [[ -n "$h_temp" ]]; then
    temp_disk "$h_temp" && had_temp=1
    temp_disk "$SIZE_TEMP" && has_temp=1
    [[ $had_temp -eq $has_temp ]] ||
      die "Azure ändert die Größe nur zwischen Größen, die beide eine lokale Temp-Platte haben oder beide keine, und nur eine von $have und $SIZE hat eine — wähl eine andere Größe oder destroy + up"
  fi
  if [[ $paused -eq 1 ]]; then
    say "Ändere die pausierte $VM von $have auf $SIZE. Sie bleibt pausiert."
  else
    say "Ändere $VM von $have auf $SIZE. Das Office ist ein paar Minuten offline;"
    echo "   laufende Arbeiter stoppen und kommen schlafend zurück (R an ihrem Schreibtisch setzt sie fort)."
  fi
  confirm
  if [[ "$was" != "deallocated" ]]; then
    say "Halte an"
    azc vm deallocate -g "$RG" -n "$VM" -o none
  fi
  if ! azc vm resize -g "$RG" -n "$VM" --size "$SIZE" -o none; then
    warn "Azure wollte keine $SIZE daraus machen (siehe oben)"
    [[ $paused -eq 1 ]] && die "das Office ist noch eine $have und noch pausiert"
    say "Starte es wieder als $have"
    azc vm start -g "$RG" -n "$VM" -o none || true
    die "das Office ist noch eine $have"
  fi
  if [[ $paused -eq 1 ]]; then
    RESIZED_PAUSED=1
    ok "Jetzt eine $SIZE, und noch pausiert"
    return
  fi
  say "Starte als $SIZE"
  if ! azc vm start -g "$RG" -n "$VM" -o none; then
    warn "Azure konnte es nicht als $SIZE starten (siehe oben; oft ist gerade kein Platz für diese Größe in der Region)"
    say "Gehe zurück auf $have"
    azc vm deallocate -g "$RG" -n "$VM" -o none || true
    if azc vm resize -g "$RG" -n "$VM" --size "$have" -o none && azc vm start -g "$RG" -n "$VM" -o none; then
      die "das Office läuft wieder als $have. Versuch $SIZE später noch einmal, oder eine andere Größe"
    fi
    die "…und auch das ist gescheitert (siehe oben). Versuch es erneut mit: deploy/azure.sh resize $have$NAME_FLAG"
  fi
  ok "Jetzt eine $SIZE"
}

# Befehle, die per SSH hineingehen, brauchen den Schlüssel, den `up` angelegt hat. Zuerst geprüft, da ein gescheitertes ssh stumm sein kann.
require_key() {
  [[ -f "$KEY_FILE" ]] || die "der SSH-Schlüssel für das Office \"$NAME\" liegt nicht auf diesem Computer ($KEY_FILE).
   Lass diesen Computer herein mit: deploy/azure.sh connect$NAME_FLAG (oder kopier den Ordner von dem Computer, der das Office angelegt hat)"
}

require_vm() {
  require_key
  require_group
  require_vm_exists
  case "$(vm_power)" in
    stopped | stopping | deallocated | deallocating) die "das Office ist pausiert — starte es mit: deploy/azure.sh resume$NAME_FLAG" ;;
  esac
  IP=$(public_ip)
  [[ -n "$IP" ]] || die "die VM des Office hat keine öffentliche IP ($PIP) — führ aus: deploy/azure.sh up$NAME_FLAG"
}

SSH_OPTS=(-i "$KEY_FILE" -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$KNOWN_HOSTS"
  -o ConnectTimeout=8 -o ServerAliveInterval=15 -o LogLevel=ERROR)

remote() {
  require_key
  # shellcheck disable=SC2029 # der Befehl soll hier expandiert und dort ausgeführt werden
  ssh "${SSH_OPTS[@]}" "$SSH_USER@$IP" "$@"
}

# Die Adressen, die SSH erreichen dürfen, eine pro Zeile (keine, wenn die Regel nicht da ist). Azure
# hält eine einzelne in sourceAddressPrefix und mehrere in sourceAddressPrefixes. Jeder andere Fehler
# stoppt das Skript: Für eine leere Liste gehalten, ließe er die nächste Änderung die Adressen aller anderen verwerfen.
allowed_cidrs() {
  local out
  out=$(azc network nsg show -g "$RG" -n "$NSG" -o tsv \
    --query "securityRules[?name=='$SSH_RULE'] | [0].[sourceAddressPrefix, sourceAddressPrefixes][]") ||
    die "konnte die Firewall-Regeln von $NSG nicht lesen (siehe oben)"
  printf '%s\n' "$out" | sed '/^$/d;/^None$/d'
}

# Macht die eine eingehende Regel der NSG zu SSH (22) von genau diesen Adressen. Es wird nur SSH
# geöffnet; das Office selbst erreichst du über den Tunnel. Ohne Adressen ist SSH für alle geschlossen.
set_ssh_sources() {
  if [[ $# -eq 0 ]]; then
    azc network nsg rule delete -g "$RG" --nsg-name "$NSG" -n "$SSH_RULE" -o none
  else
    # create ist ein PUT: Es schreibt die ganze Regel, ob sie schon da ist oder nicht.
    azc network nsg rule create -g "$RG" --nsg-name "$NSG" -n "$SSH_RULE" --priority 1000 \
      --direction Inbound --access Allow --protocol Tcp --source-address-prefixes "$@" --source-port-ranges '*' \
      --destination-address-prefixes '*' --destination-port-ranges 22 \
      --description "OpenAgenticOffice: SSH nur von erlaubten IPs" -o none
  fi
}

# Ist dieses CIDR in der Liste (eines pro Zeile)? 203.0.113.7 und 203.0.113.7/32 sind dasselbe.
has_cidr() { printf '%s\n' "$1" | awk -v c="${2%/32}" '{ sub(/\/32$/, "") } $0 == c { found = 1 } END { exit !found }'; }

# Fügt die angegebenen CIDRs zu den Adressen hinzu, die per SSH hinein dürfen (mit - zuerst: nimmt sie weg).
change_ssh_sources() {
  local remove=0 have want c list=()
  [[ "${1:-}" == "-" ]] && { remove=1; shift; }
  have=$(allowed_cidrs)
  want="$have"
  for c in "$@"; do
    if [[ $remove -eq 1 ]]; then
      want=$(printf '%s\n' "$want" | awk -v c="${c%/32}" '{ x = $0; sub(/\/32$/, "", x) } x != c')
    elif ! has_cidr "$want" "$c"; then
      want=$(printf '%s\n%s' "$want" "$c")
    fi
  done
  [[ "$want" == "$have" ]] && return 0
  while IFS= read -r c; do
    if [[ -n "$c" ]]; then list+=("$c"); fi
  done <<<"$want"
  set_ssh_sources "${list[@]+"${list[@]}"}"
}

office_get() { remote "curl -fs --max-time 4 http://127.0.0.1:$OFFICE_PORT$1"; }

ip_int() {
  local IFS=.
  # shellcheck disable=SC2086 # an den Punkten aufteilen
  set -- $1
  echo $((($1 << 24) | ($2 << 16) | ($3 << 8) | $4))
}

# Ob die IPv4-Adresse $1 in einem der CIDRs auf stdin liegt.
ip_allowed() {
  local ip c n bits mask
  ip=$(ip_int "$1")
  while IFS= read -r c; do
    if [[ "$c" == "*" || "$c" == "Internet" ]]; then return 0; fi
    [[ "$c" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || continue
    n=$(ip_int "${c%/*}")
    bits=${c#*/}
    mask=$((bits == 0 ? 0 : (0xFFFFFFFF << (32 - bits)) & 0xFFFFFFFF))
    if (((ip & mask) == (n & mask))); then return 0; fi
  done
  return 1
}

# SSH, das sich nicht verbindet: stoppen und es sagen, wenn die IP dieses Computers keine ist, die die Firewall hereinlässt.
ssh_blocked_hint() {
  local my list
  my=$(my_ip 2>/dev/null) || return 0
  [[ "$my" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]] || return 0
  list=$(allowed_cidrs 2>/dev/null) || return 0
  printf '%s\n' "$list" | ip_allowed "$my" && return 0
  die "SSH antwortet nur den IPs, die du erlaubt hast, und die dieses Computers ($my) gehört nicht dazu. Lass ihn herein mit: deploy/azure.sh allow me$NAME_FLAG"
}

# Wartet auf SSH bei einer VM, die (neu) startet.
wait_for_ssh() {
  local i
  say "Warte auf SSH"
  for ((i = 0; i < 60; i++)); do
    remote true 2>/dev/null && return 0
    sleep 5
  done
  remote true || die "SSH kam auf $IP nie hoch"
}

wait_healthy() {
  local i rc
  for ((i = 0; i < 30; i++)); do
    rc=0
    remote "for i in \$(seq 90); do curl -fs --max-time 4 http://127.0.0.1:$OFFICE_PORT/api/health >/dev/null && exit 0; sleep 2; done; exit 1" \
      2>/dev/null || rc=$?
    [[ $rc -eq 255 ]] || return "$rc" # 255: ssh selbst ist gescheitert, die VM bootet noch…
    [[ $i -eq 0 ]] && ssh_blocked_hint # …oder die IP dieses Computers ist nicht erlaubt
    sleep 5
  done
  return 1
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
  die "kein freier lokaler Port ab $LOCAL_PORT (wähl einen mit --port)"
}

# localhost:<port> zum Office auf der VM weiterleiten, den Browser öffnen und bis Strg-C halten.
tunnel() {
  local path="$1" port pid i up=0
  port=$(pick_port) || exit 1
  ssh "${SSH_OPTS[@]}" -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$IP" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "konnte den SSH-Tunnel zu $IP nicht öffnen"
    curl -fs --max-time 2 "http://localhost:$port/api/health" >/dev/null 2>&1 && { up=1; break; }
    sleep 0.5
  done
  if [[ $up -ne 1 ]]; then
    kill "$pid" 2>/dev/null
    die "der Tunnel ist offen, aber das Office antwortet nicht darüber — prüf: deploy/azure.sh logs$NAME_FLAG"
  fi
  ok "Dein Office: http://localhost:$port$path"
  echo "   (per SSH nach $IP getunnelt — lass das laufen, solange du es benutzt; Strg-C schließt es)"
  open_url "http://localhost:$port$path"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn neu mit: deploy/azure.sh open$NAME_FLAG"
}

# Der Server eines Arbeiters von der 🌐 Dienste-Tafel: localhost:<port> tunnelt zum Office, das ihn
# über diesen Port weiterreicht (siehe das Relay im Server), also muss der lokale Port zu dem des Dienstes passen.
service_tunnel() {
  local port="$1" pid i up=0
  port_busy "$port" && die "localhost:$port ist auf diesem Computer schon belegt — beende zuerst, was dort läuft"
  ssh "${SSH_OPTS[@]}" -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$IP" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "konnte den SSH-Tunnel zu $IP nicht öffnen"
    port_busy "$port" && { up=1; break; }
    sleep 0.5
  done
  [[ $up -eq 1 ]] || { kill "$pid" 2>/dev/null; die "der Tunnel kam nicht hoch"; }
  ok "Der Server des Arbeiters: http://localhost:$port"
  echo "   (über das Office auf $IP — melde dich mit dem Office-Passwort an, falls gefragt; Strg-C schließt es)"
  open_url "http://localhost:$port"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn neu mit: deploy/azure.sh service $port$NAME_FLAG"
}

open_office() {
  local claimable path="/"
  claimable=$(office_get /api/beanspruchen 2>/dev/null || true)
  if [[ "$claimable" == *'"claimable":true'* && -f "$CLAIM_FILE" ]]; then
    path="/beanspruchen?t=$(cat "$CLAIM_FILE")"
    say "Öffne die einmalige Passwort-Seite — schreib das Passwort auf, es wird nie wieder angezeigt"
  fi
  tunnel "$path"
}

valid_member() { [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,38}$ ]] || die "Namen bestehen aus Buchstaben, Ziffern, Punkten, Bindestrichen und Unterstrichen: $1"; }

# Die Schlüssel der Teammitglieder verwaltet auf der VM open-agentic-office-team (von provision.sh installiert).
require_team() {
  remote "test -x /usr/local/bin/open-agentic-office-team" 2>/dev/null || die "dieses Office ist älter als der Team-Zugang — führ aus: deploy/azure.sh up$NAME_FLAG"
}

team_members() { remote "open-agentic-office-team list"; } # "<name> <Anzahl Schlüssel>" pro Zeile

# --- Befehle ---------------------------------------------------------------------------------------

github_https() {
  # git@github.com:owner/repo(.git) | https://github.com/owner/repo(.git) | owner/repo -> https-URL
  local v="$1"
  v="${v%.git}"
  v="${v#git@github.com:}"
  v="${v#https://github.com/}"
  v="${v#ssh://git@github.com/}"
  [[ "$v" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || return 1
  echo "https://github.com/$v"
}

cmd_up() {
  preflight
  need ssh-keygen

  # Was installiert wird. Das Office startet ohne Projekt (nie mit dem Checkout, in dem dieses Skript
  # liegt): alle wählen ihres im Aufzug, außer --project nennt ein erstes.
  if [[ -z "$APP_REPO" ]]; then
    APP_REPO=$(github_https "$(git -C "$SCRIPT_DIR/.." remote get-url origin 2>/dev/null || true)" || echo "https://github.com/Luna-OS/OpenAgenticOffice")
  fi
  local project_repo=""
  if [[ -n "$PROJECT" ]]; then
    project_repo=$(github_https "$PROJECT") || die "--project muss ein GitHub-Repo sein (owner/name oder URL), bekommen: $PROJECT"
  fi

  local gh_token="$GH_TOKEN_ARG"
  if [[ -z "$gh_token" && $NO_GH_TOKEN -eq 0 ]] && command -v gh >/dev/null 2>&1; then
    gh_token=$(gh auth token 2>/dev/null || true)
  fi
  [[ $NO_GH_TOKEN -eq 1 ]] && gh_token=""

  local my a c
  my=$(my_ip) || die "konnte deine öffentliche IP nicht ermitteln"
  local cidrs=("$my/32")
  for a in "${EXTRA_ALLOW[@]+"${EXTRA_ALLOW[@]}"}"; do
    c=$(to_cidr "$a")
    cidrs+=("$c")
  done

  # Wo: Ein neues Office kommt nach --location; ein bestehendes bleibt in der Region seiner Ressourcengruppe.
  load_group
  local new_group=0
  if [[ -z "$LOCATION" ]]; then
    new_group=1
    LOCATION=$(normalize_location "${LOCATION_ARG:-$(default_location)}")
    [[ "$LOCATION" =~ ^[a-z0-9]+$ ]] || die "keine Azure-Region: $LOCATION (siehe: az account list-locations -o table)"
  elif [[ -n "$LOCATION_ARG" && "$(normalize_location "$LOCATION_ARG")" != "$LOCATION" ]]; then
    die "das Office \"$NAME\" liegt in $LOCATION. Um es zu verschieben, lösch es zuerst (oder wähl einen anderen --name)"
  fi

  VM_ID=""
  if [[ $new_group -eq 0 ]]; then VM_ID=$(find_vm); fi
  local have_size="" resize=0 machine
  if [[ -z "$VM_ID" ]]; then
    size_info "$SIZE"
    # Die az CLI schaltet Trusted Launch für Azures Gen2-Ubuntu-Images ein (auch Arm), also braucht
    # eine Größe ohne es das Gen1-Image, und die Ampere-Arm-Größen (Bpsv2) haben keins.
    if [[ "$SIZE_NO_TL" == "True" && "$SIZE_GENS" != *V1* ]]; then
      [[ "$SIZE_ARCH" == "Arm64" ]] && die "$SIZE ist eine Arm-Größe ohne Trusted Launch. Für Arm wähl eine Cobalt-Größe wie Standard_D4ps_v6"
      die "$SIZE unterstützt weder Trusted Launch noch Gen1-Images — wähl eine andere Größe, etwa den Standard $DEFAULT_SIZE"
    fi
    machine="$SIZE, ${DISK_GB} GiB Platte, Ubuntu 24.04"
  else
    have_size=$(vm_size)
    machine="die vorhandene VM ($have_size)"
    if [[ $SIZE_SET -eq 1 && "$(lower "$have_size")" != "$(lower "$SIZE")" ]]; then
      resize=1
      machine="die vorhandene VM, von $have_size auf $SIZE geändert"
    fi
  fi

  say "OpenAgenticOffice \"$NAME\" in $LOCATION (Abonnement $SUB_NAME)"
  echo "   Maschine: $machine"
  echo "   App:      $APP_REPO @ $APP_REF"
  echo "   Projekte: ${project_repo:+$project_repo, dann }im Aufzug des Office wählen (geklont nach ~/workspace)"
  echo "   Zugang:   nur SSH-Tunnel (das Office ist nie offen); SSH von ${cidrs[*]}"
  if [[ -n "$gh_token" ]]; then
    echo "   GitHub:   dein GitHub-Token kommt auf die VM (private Klone, Issue-/PR-Tafeln, Pushes)"
  else
    echo "   GitHub:   kein Token — private Repos und die Tafeln funktionieren nicht"
  fi
  if [[ -n "$CLAUDE_TOKEN" || -n "$ANTHROPIC_KEY" ]]; then
    echo "   Claude:   mit dem übergebenen Token angemeldet"
  else
    echo "   Claude:   nicht angemeldet — melde dich im Terminal des ersten Arbeiters an (oder gib --claude-token an)"
  fi

  mkdir -p "$STATE_DIR"
  chmod 700 "$STATE_DIR"
  echo "$SUB_ID" >"$SUB_FILE"
  local new_key=0
  if [[ ! -f "$KEY_FILE" ]]; then
    ssh-keygen -q -t rsa -b 4096 -N '' -C "$RESOURCE" -f "$KEY_FILE"
    new_key=1
  fi

  # Eine Ressourcengruppe hält alles, mit einem Tag versehen, damit destroy weiß, dass sie dem Office gehört.
  if [[ $new_group -eq 1 ]]; then
    azc group create -n "$RG" -l "$LOCATION" --tags "open-agentic-office=$NAME" -o none
    ok "Ressourcengruppe $RG"
  fi

  # Netzwerksicherheitsgruppe: nur die erlaubten IPs erreichen Port 22 (ssh). Sonst ist nichts offen.
  local nsg_id pip_id
  nsg_id=$(find_resource nsg "$NSG")
  if [[ -z "$nsg_id" ]]; then
    azc network nsg create -g "$RG" -n "$NSG" -l "$LOCATION" --tags "open-agentic-office=$NAME" -o none
    ok "Netzwerksicherheitsgruppe $NSG"
  fi
  change_ssh_sources "${cidrs[@]}"
  ok "SSH erlaubt von ${cidrs[*]}"

  # Eine feste Adresse, damit die Adresse des Office Pausen und Größenänderungen übersteht.
  pip_id=$(find_resource public-ip "$PIP")
  if [[ -z "$pip_id" ]]; then
    # Azure trennt eine Verbindung, die --idle-timeout Minuten (standardmäßig 4) still ist, und der
    # Tunnel eines Teammitglieds sendet keine Keepalives.
    azc network public-ip create -g "$RG" -n "$PIP" -l "$LOCATION" --sku Standard --allocation-method Static --version IPv4 \
      --idle-timeout 30 --tags "open-agentic-office=$NAME" -o none
  fi
  IP=$(public_ip)
  [[ -n "$IP" ]] || die "die öffentliche IP $PIP hat keine Adresse"
  ok "Statische IP $IP"

  # Die VM.
  if [[ -z "$VM_ID" ]]; then
    # Ubuntu 24.04. Die az CLI schaltet Trusted Launch (Secure Boot + vTPM) für Gen2-x64-Images von
    # selbst ein. Größen ohne es (oder ohne Gen2) nehmen das Gen1-Image und Arm-Größen das Arm-Image.
    local image="Canonical:ubuntu-24_04-lts:server:latest" disk_sku="Premium_LRS"
    if [[ "$SIZE_ARCH" == "Arm64" ]]; then
      image="Canonical:ubuntu-24_04-lts:server-arm64:latest"
    elif [[ "$SIZE_GENS" != *V2* || ("$SIZE_NO_TL" == "True" && "$SIZE_GENS" == *V1*) ]]; then
      image="Canonical:ubuntu-24_04-lts:server-gen1:latest"
    fi
    [[ "$SIZE_PREMIUM" == "True" ]] || disk_sku="StandardSSD_LRS"
    say "Lege die VM an ($SIZE) — ein bis zwei Minuten"
    azc vm create -g "$RG" -n "$VM" -l "$LOCATION" --image "$image" --size "$SIZE" \
      --admin-username "$SSH_USER" --authentication-type ssh --ssh-key-values "$KEY_FILE.pub" \
      --vnet-name "$VNET" --subnet default --nsg "$NSG" --nsg-rule NONE --public-ip-address "$PIP" \
      --os-disk-size-gb "$DISK_GB" --storage-sku "$disk_sku" --tags "open-agentic-office=$NAME" -o none ||
      die "konnte die VM nicht anlegen (Azures Begründung steht oben). Bei einem Kontingent frag mehr vCPUs
   für die Familie dieser Größe an (Azure-Portal → Kontingente → Compute), oder führ up mit einer anderen --size erneut aus.
   Eine andere Region (--location) braucht einen frischen Start: vorher deploy/azure.sh destroy$NAME_FLAG"
  elif [[ $resize -eq 1 ]]; then
    resize_vm "$SIZE"
    if [[ $RESIZED_PAUSED -eq 1 ]]; then start_vm; fi
  elif [[ "$(settled_power)" != "running" ]]; then
    start_vm
  else
    say "Verwende $VM weiter ($have_size)"
  fi
  ok "Die VM $VM läuft auf $IP"

  # Ein Office, das auf einem anderen Computer angelegt wurde (oder dessen Schlüssel verloren ging): den Schlüssel dieses Computers auch darauf legen.
  if [[ $new_key -eq 1 && -n "$VM_ID" ]]; then add_key; fi
  wait_for_ssh

  [[ -f "$CLAIM_FILE" ]] || (umask 077 && random_token >"$CLAIM_FILE")

  say "Richte ein (git, gh, Claude Code, open-agentic-office) — beim ersten Mal ein paar Minuten"
  local git_name git_email
  git_name=$(git config user.name 2>/dev/null || true)
  git_email=$(git config user.email 2>/dev/null || true)
  {
    printf 'export APP_REPO=%q APP_REF=%q PROJECT_REPO=%q\n' "$APP_REPO" "$APP_REF" "$project_repo"
    printf 'export CLAIM_TOKEN=%q PUBLIC_HOST=%q GH_TOKEN=%q CLAUDE_CODE_OAUTH_TOKEN=%q ANTHROPIC_API_KEY=%q\n' "$(cat "$CLAIM_FILE")" "$IP" "$gh_token" "$CLAUDE_TOKEN" "$ANTHROPIC_KEY"
    # Wie das Office dieses Skript in den Befehlen nennt, die es vorschlägt (mit --name für ein zweites Office).
    printf 'export GIT_NAME=%q GIT_EMAIL=%q DEPLOY_SCRIPT=%q\n' "$git_name" "$git_email" "deploy/azure.sh$NAME_FLAG"
    cat "$SCRIPT_DIR/provision.sh"
  } | remote 'bash -s' || die "die Einrichtung ist gescheitert (führ \"deploy/azure.sh up$NAME_FLAG\" erneut aus; es macht dort weiter, wo es aufgehört hat)"

  say "Warte, bis das Office antwortet"
  wait_healthy || die "das Office kam nicht hoch — prüf: deploy/azure.sh logs$NAME_FLAG"
  ok "Dein Office läuft auf $IP (nur über SSH erreichbar)"
  echo
  echo "   Später öffnen:        deploy/azure.sh open$NAME_FLAG"
  echo "   Teammitglied:         der Knopf 👥 Team einladen im Office oder deploy/azure.sh invite <github-name>$NAME_FLAG"
  echo "   Pausieren/Fortsetzen: deploy/azure.sh pause$NAME_FLAG   /   deploy/azure.sh resume$NAME_FLAG"
  echo "   Abbauen:              deploy/azure.sh destroy$NAME_FLAG"
  echo
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

# Legt den öffentlichen Schlüssel dieses Computers auf die VM (über den VM-Agenten von Azure, also ohne SSH).
add_key() {
  say "Lege den SSH-Schlüssel dieses Computers auf die VM"
  azc vm user update -g "$RG" -n "$VM" --username "$SSH_USER" --ssh-key-value "$KEY_FILE.pub" -o none ||
    die "konnte den SSH-Schlüssel ($KEY_FILE.pub) nicht auf die VM legen (siehe oben)"
}

# Ein Office, das auf einem anderen Computer angelegt wurde: dieser bekommt einen Schlüssel auf der VM und seine IP in der Firewall.
# Anders als up richtet es nichts ein, sodass das Office seine GitHub- und Claude-Anmeldungen behält.
cmd_connect() {
  preflight
  need ssh-keygen
  require_group
  require_vm_exists
  mkdir -p "$STATE_DIR"
  chmod 700 "$STATE_DIR"
  echo "$SUB_ID" >"$SUB_FILE"
  local my
  my=$(my_ip) || die "konnte deine öffentliche IP nicht ermitteln"
  change_ssh_sources "$my/32"
  ok "SSH erlaubt von $my/32"
  start_vm # der VM-Agent, der den Schlüssel einträgt, läuft nur auf einer laufenden VM
  IP=$(public_ip)
  [[ -n "$IP" ]] || die "die VM des Office hat keine öffentliche IP ($PIP) — führ aus: deploy/azure.sh up$NAME_FLAG"
  if [[ -f "$KEY_FILE" ]] && remote true 2>/dev/null; then
    ok "Dieser Computer konnte schon per SSH hinein"
  else
    [[ -f "$KEY_FILE" ]] || ssh-keygen -q -t rsa -b 4096 -N '' -C "$RESOURCE" -f "$KEY_FILE"
    add_key
    wait_for_ssh
  fi
  ok "Verbunden mit dem Office \"$NAME\" auf $IP — öffne es mit: deploy/azure.sh open$NAME_FLAG"
}

cmd_open() {
  preflight
  require_vm
  wait_healthy || die "das Office antwortet nicht — prüf: deploy/azure.sh logs$NAME_FLAG"
  open_office
}

cmd_service() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 && "${POSITIONAL[0]}" =~ ^[0-9]+$ && ${POSITIONAL[0]} -gt 0 && ${POSITIONAL[0]} -lt 65536 ]] ||
    die "Aufruf: deploy/azure.sh service <port>   (ein Port von der 🌐 Dienste-Tafel des Office)"
  [[ "${POSITIONAL[0]}" -ne $OFFICE_PORT ]] || die "$OFFICE_PORT ist das Office selbst — nimm: deploy/azure.sh open"
  require_vm
  service_tunnel "${POSITIONAL[0]}"
}

cmd_status() {
  preflight
  load_group
  if [[ -z "$LOCATION" ]]; then
    echo "Kein Office namens \"$NAME\" im Abonnement $SUB_NAME."
    return
  fi
  echo "Office:    $NAME ($LOCATION, Ressourcengruppe $RG)"
  VM_ID=$(find_vm)
  if [[ -z "$VM_ID" ]]; then
    echo "VM:        noch keine (anlegen mit: deploy/azure.sh up$NAME_FLAG)"
    return
  fi
  local state
  state=$(vm_power)
  IP=$(public_ip)
  echo "VM:        $VM $(vm_size) ${state:-unbekannt}"
  echo "Adresse:   ${IP:-keine}  (Office öffnen mit: deploy/azure.sh open$NAME_FLAG)"
  if [[ "$state" =~ ^(stopped|stopping)$ ]]; then
    echo "Office:    heruntergefahren, aber Azure berechnet eine angehaltene VM weiter (deploy/azure.sh pause$NAME_FLAG gibt sie frei; resume startet sie)"
  elif [[ "$state" =~ ^(deallocated|deallocating)$ ]]; then
    echo "Office:    pausiert (starte es mit: deploy/azure.sh resume$NAME_FLAG)"
  elif [[ -n "$IP" && -f "$KEY_FILE" ]] && office_get /api/health >/dev/null 2>&1; then
    echo "Office:    läuft"
    local team
    team=$(team_members 2>/dev/null | awk '{printf "%s%s", sep, $1; sep=", "}') || team="(konnte nicht aufgelistet werden)"
    echo "Team:      ${team:-noch niemand eingeladen}"
  else
    echo "Office:    antwortet nicht"
  fi
  local from
  from=$(allowed_cidrs 2>/dev/null | tr '\n' ' ') || from="(konnte die Firewall nicht lesen)"
  echo "SSH von:   ${from:-niemand}"
}

cmd_allow() {
  preflight
  [[ ${#POSITIONAL[@]} -gt 0 ]] || die "Aufruf: deploy/azure.sh allow <ip|cidr|me> [...]"
  require_group
  local c cidrs=()
  for c in "${POSITIONAL[@]}"; do
    c=$(to_cidr "$c")
    cidrs+=("$c")
  done
  change_ssh_sources "${cidrs[@]}"
  for c in "${cidrs[@]}"; do ok "Erlaubt: $c"; done
}

cmd_revoke() {
  preflight
  [[ ${#POSITIONAL[@]} -gt 0 ]] || die "Aufruf: deploy/azure.sh revoke <ip|cidr|me> [...]"
  require_group
  local c cidrs=()
  for c in "${POSITIONAL[@]}"; do
    c=$(to_cidr "$c")
    cidrs+=("$c")
  done
  change_ssh_sources - "${cidrs[@]}"
  for c in "${cidrs[@]}"; do ok "Entzogen: $c"; done
  [[ -n "$(allowed_cidrs)" ]] || warn "Jetzt darf keine IP mehr per SSH hinein, du eingeschlossen. Lass deine wieder herein mit: deploy/azure.sh allow me$NAME_FLAG"
}

cmd_invite() {
  preflight
  [[ ${#POSITIONAL[@]} -ge 1 && ${#POSITIONAL[@]} -le 2 ]] ||
    die "Aufruf: deploy/azure.sh invite <github-name>   oder   deploy/azure.sh invite <name> <public-key-datei>"
  local who="${POSITIONAL[0]}" src raw
  valid_member "$who"
  if [[ ${#POSITIONAL[@]} -eq 2 ]]; then
    src="${POSITIONAL[1]}"
    [[ -f "$src" ]] || die "keine solche Datei: $src"
    raw=$(cat "$src")
  else
    src="github.com/$who.keys"
    raw=$(curl -fsS --max-time 10 "https://github.com/$who.keys") || die "konnte https://$src nicht abrufen"
  fi
  [[ -n "$raw" ]] || die "keine öffentlichen SSH-Schlüssel in $src gefunden"
  require_vm
  require_team
  local n
  # Die VM behält nur gültige Schlüssel und beschränkt jeden darauf, den Tunnel zu öffnen.
  n=$(printf '%s\n' "$raw" | remote "open-agentic-office-team add $who") || die "konnte die Schlüssel von $who aus $src nicht hinzufügen"
  ok "$who ist eingeladen ($n Schlüssel aus $src)"

  local a cidrs=() fp
  for a in "${EXTRA_ALLOW[@]+"${EXTRA_ALLOW[@]}"}"; do
    a=$(to_cidr "$a")
    cidrs+=("$a")
  done
  if [[ ${#cidrs[@]} -gt 0 ]]; then
    change_ssh_sources "${cidrs[@]}"
    ok "SSH erlaubt von ${cidrs[*]}"
  fi
  fp=$(remote "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub" | awk '{print $2}')
  echo
  echo "   Schick $who das hier:"
  echo
  echo "     ssh -L 4600:localhost:$OFFICE_PORT $TEAM_USER@$IP"
  echo
  echo "     Lass es laufen, öffne http://localhost:4600 und melde dich mit dem Office-Passwort an."
  echo "     Beim ersten Mal fragt ssh, ob du dem Server vertraust. Sag nur Ja, wenn er"
  echo "     ED25519 key fingerprint $fp zeigt"
  echo
  if ! has_cidr "$(allowed_cidrs)" "0.0.0.0/0"; then
    echo "   SSH antwortet nur erlaubten IPs, also führ außerdem aus: deploy/azure.sh allow <ihre-ip>$NAME_FLAG"
    echo "   (oder \"allow anywhere\" — SSH nimmt nur deinen Schlüssel und eingeladene Schlüssel an)"
  fi
}

cmd_uninvite() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/azure.sh uninvite <name>"
  local who="${POSITIONAL[0]}" out rc=0
  valid_member "$who"
  require_vm
  require_team
  out=$(remote "open-agentic-office-team remove $who" 2>&1) || rc=$?
  [[ $rc -eq 66 ]] && die "$who ist nicht eingeladen (siehe: deploy/azure.sh team$NAME_FLAG)"
  [[ $rc -eq 0 ]] || die "konnte die Schlüssel nicht entfernen: $out"
  ok "Die Schlüssel von $who sind entfernt und offene Tunnel gekappt (andere Teammitglieder verbinden sich einfach neu)"
  echo "   Sie kennen das Office-Passwort noch. Zum Ändern: deploy/azure.sh reset-password$NAME_FLAG"
}

cmd_team() {
  preflight
  require_vm
  require_team
  local list
  list=$(team_members)
  if [[ -z "$list" ]]; then
    echo "Noch niemand ist eingeladen. Lade jemanden ein: deploy/azure.sh invite <github-name>$NAME_FLAG"
    return
  fi
  echo "$list" | awk '{printf "%s  (%d Schlüssel)\n", $1, $2}'
}

cmd_ssh() {
  preflight
  require_vm
  exec ssh "${SSH_OPTS[@]}" -t "$SSH_USER@$IP" "${POSITIONAL[@]+"${POSITIONAL[@]}"}"
}

cmd_logs() {
  preflight
  require_vm
  exec ssh "${SSH_OPTS[@]}" -t "$SSH_USER@$IP" 'sudo journalctl -u open-agentic-office -n 100 -f'
}

cmd_resize() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/azure.sh resize <vm-größe>   (z. B. Standard_D8as_v5, Standard_B4s_v2)"
  require_key
  require_group
  require_vm_exists
  resize_vm "${POSITIONAL[0]}"
  if [[ $RESIZED_PAUSED -eq 1 ]]; then
    echo "   Starte es mit: deploy/azure.sh resume$NAME_FLAG"
    return
  fi
  IP=$(public_ip)
  say "Warte, bis das Office antwortet"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/azure.sh logs$NAME_FLAG"
  ok "Dein Office ist zurück — öffne es mit: deploy/azure.sh open$NAME_FLAG"
}

cmd_pause() {
  preflight
  require_group
  require_vm_exists
  local state
  state=$(settled_power)
  if [[ "$state" != "deallocated" ]]; then
    say "Pausiere Office \"$NAME\" ($VM). Laufende Arbeiter stoppen und kommen schlafend zurück,"
    echo "   wenn du fortsetzt (R an ihrem Schreibtisch). Offene Tunnel, auch die von Teammitgliedern, werden gekappt."
    confirm
    # Deallocate, nicht nur anhalten: Eine angehaltene VM behält ihre Hardware, und Azure rechnet sie weiter ab.
    say "Gebe die VM frei (deallocate)"
    azc vm deallocate -g "$RG" -n "$VM" -o none
  fi
  ok "Pausiert. Platte und Adresse bleiben (und nur sie kosten, bis du fortsetzt)"
  echo "   Wieder starten mit: deploy/azure.sh resume$NAME_FLAG"
}

cmd_resume() {
  preflight
  require_key
  require_group
  require_vm_exists
  start_vm
  IP=$(public_ip)
  [[ -n "$IP" ]] || die "die VM des Office hat keine öffentliche IP ($PIP) — führ aus: deploy/azure.sh up$NAME_FLAG"
  say "Warte, bis das Office antwortet"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/azure.sh logs$NAME_FLAG"
  ok "Dein Office ist zurück (die Arbeiter machen dort weiter, wo sie aufgehört haben)"
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_update() {
  preflight
  require_vm
  say "Aktualisiere open-agentic-office auf $IP"
  remote "set -e
    open-agentic-office-update
    sudo systemctl restart open-agentic-office" || die "die Aktualisierung ist gescheitert"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/azure.sh logs$NAME_FLAG"
  ok "Aktualisiert und neu gestartet (die Arbeiter laufen dabei weiter)"
}

cmd_reset_password() {
  preflight
  require_vm
  (umask 077 && random_token >"$CLAIM_FILE")
  say "Setze das Office-Passwort zurück"
  remote "set -e
    sudo sed -i 's/^OFFICE_BEANSPRUCH_TOKEN=.*/OFFICE_BEANSPRUCH_TOKEN=\"$(cat "$CLAIM_FILE")\"/' /etc/open-agentic-office/env
    sudo systemctl stop open-agentic-office
    sudo -u \"\$(systemctl show -p User --value open-agentic-office)\" /opt/open-agentic-office/open-agentic-office --daten \"\$(cat /etc/open-agentic-office/home)\" --passwort-zuruecksetzen >/dev/null
    sudo systemctl start open-agentic-office" || die "das Zurücksetzen ist gescheitert"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/azure.sh logs$NAME_FLAG"
  ok "Alle sind abgemeldet"
  open_office
}

# Löscht die Dateien dieses Computers zum Office (SSH-Schlüssel, Beanspruchungs-Link), außer sie gehören
# zu einem Office gleichen Namens in einem anderen Abonnement.
forget_office() {
  if [[ -s "$SUB_FILE" && "$(cat "$SUB_FILE")" != "$SUB_ID" ]]; then
    echo "(Die Dateien dieses Computers zu \"$NAME\" in $STATE_DIR gehören zum Abonnement $(cat "$SUB_FILE"), also bleiben sie.)"
    return
  fi
  rm -rf "$STATE_DIR"
}

cmd_down() {
  preflight
  load_group
  if [[ -z "$LOCATION" ]]; then
    echo "Für \"$NAME\" im Abonnement $SUB_NAME gibt es nichts zu löschen."
    forget_office
    return
  fi
  say "Das löscht das Office \"$NAME\" endgültig: die Ressourcengruppe $RG in $LOCATION (Abonnement $SUB_NAME)"
  echo "   und alles darin:"
  azc resource list -g "$RG" -o tsv --query '[].[type, name]' | awk -F'\t' '{ printf "     %s  %s\n", $2, $1 }' || true
  echo "   Alles auf der VM, das nicht auf GitHub gepusht ist, geht verloren."
  if [[ $YES -ne 1 ]]; then
    local answer
    read -r -p "   Tippe zur Bestätigung den Office-Namen ($NAME): " answer
    [[ "$answer" == "$NAME" ]] || die "abgebrochen"
  fi
  say "Lösche $RG — das dauert ein paar Minuten"
  azc group delete -n "$RG" --yes -o none || die "konnte $RG nicht löschen (siehe oben) — führ destroy in einer Minute erneut aus"
  forget_office
  ok "Alles weg"
}

case "$CMD" in
  up) cmd_up ;;
  open) cmd_open ;;
  service) cmd_service ;;
  status) cmd_status ;;
  invite) cmd_invite ;;
  uninvite) cmd_uninvite ;;
  team) cmd_team ;;
  allow) cmd_allow ;;
  revoke) cmd_revoke ;;
  ssh) cmd_ssh ;;
  logs) cmd_logs ;;
  resize) cmd_resize ;;
  pause) cmd_pause ;;
  resume) cmd_resume ;;
  update) cmd_update ;;
  reset-password) cmd_reset_password ;;
  destroy | down) cmd_down ;;
  connect) cmd_connect ;;
  help | -h | --help) usage ;;
  *) die "unbekannter Befehl \"$CMD\" (siehe: deploy/azure.sh help)" ;;
esac
