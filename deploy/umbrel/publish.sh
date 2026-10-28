#!/usr/bin/env bash
# Stellt die OpenAgenticOffice-Umbrel-App (deploy/umbrel/open-agentic-office) in einen Umbrel-Community-App-Store:
# ein Git-Repository mit einer umbrel-app-store.yml im Wurzelordner, etwa
# http://100.78.107.162:8101/nicorenezweers/nrzStore.
#
#   git clone http://100.78.107.162:8101/nicorenezweers/nrzStore
#   deploy/umbrel/publish.sh nrzStore --pr      # ein eigener Zweig, den du als Pull Request mergst
#   deploy/umbrel/publish.sh nrzStore --push    # oder direkt auf den aktuellen Zweig
#
# Die App landet in <store>/<store-id>-open-agentic-office/ (Umbrel verlangt, dass die ID jeder App und
# ihr Ordner mit der ID des Stores beginnen), mit dem neuesten Image, das der Workflow "Umbrel image"
# nach ghcr.io/luna-os/open-agentic-office geschoben hat, per Digest festgenagelt, und mit seiner Version
# als Version der App, damit Umbrel bei jeder neueren Veröffentlichung ein Update anbietet. Noch einmal
# ausgeführt aktualisiert es.
#
# Optionen:
#   --push            den Store danach mit git add, commit und push hochladen
#   --pr              dasselbe auf einem neuen Zweig open-agentic-office-<version>, und den Link ausgeben, der
#                     den Pull Request dafür öffnet (Gitea und GitHub nehmen /compare/<basis>...<zweig>)
#   --tag <version>   ein bestimmtes Image (Vorgabe: das neueste Tag x.y.z)
#   --image <repo>    ein anderes Image-Repository (Vorgabe: ghcr.io/luna-os/open-agentic-office)
#   --store-id <id>   die ID des Stores, für einen Store ohne umbrel-app-store.yml (es wird eine angelegt)
#   --port <port>     der Port, auf dem Umbrel das Office anbietet (Vorgabe 4600)
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
image=ghcr.io/luna-os/open-agentic-office
tag=''
store=''
store_id=''
port=4600
push=0
pr=0

die() { echo "publish.sh: $*" >&2; exit 1; }
usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'; exit "${1:-0}"; }

while (($#)); do
  case "$1" in
    --push) push=1 ;;
    --pr) push=1 pr=1 ;;
    --tag) tag=${2:?--tag braucht eine Version}; shift ;;
    --image) image=${2:?--image braucht ein Repository}; shift ;;
    --store-id) store_id=${2:?--store-id braucht eine ID}; shift ;;
    --port) port=${2:?--port braucht eine Zahl}; shift ;;
    -h | --help) usage ;;
    -*) die "unbekannte Option $1 (siehe --help)" ;;
    *) [[ -z "$store" ]] || die "immer nur ein Store"; store=$1 ;;
  esac
  shift
done
[[ -n "$store" ]] || usage 1
[[ -d "$store" ]] || die "$store ist kein Ordner: klone zuerst deinen App-Store (git clone <seine URL>)"
[[ "$port" =~ ^[0-9]+$ ]] || die "--port muss eine Zahl sein"

# Die ID des Stores: aus seiner umbrel-app-store.yml oder per --store-id angelegt.
manifest="$store/umbrel-app-store.yml"
if [[ -f "$manifest" ]]; then
  found=$(sed -n 's/^id:[[:space:]]*["'\'']\{0,1\}\([A-Za-z0-9-]*\).*/\1/p' "$manifest" | head -1)
  [[ -n "$found" ]] || die "$manifest hat keine id:-Zeile"
  [[ -z "$store_id" || "$store_id" == "$found" ]] || die "$manifest sagt, die ID des Stores sei $found, nicht $store_id"
  store_id=$found
else
  [[ -n "$store_id" ]] || die "$store hat keine umbrel-app-store.yml: gib --store-id <id> an, um eine anzulegen"
  printf 'id: "%s"\nname: "%s"\n' "$store_id" "$(basename "$(cd "$store" && pwd)")" >"$manifest"
  echo "==> angelegt: $manifest (id: $store_id)"
fi
[[ "$store_id" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "die ID des Stores ($store_id) darf nur aus Kleinbuchstaben, Ziffern und Bindestrichen bestehen"
app_id="$store_id-open-agentic-office"

# Das Image: das neueste Tag x.y.z in der Registry (oder --tag) und sein Digest, anonym gelesen.
registry=${image%%/*}
repo=${image#*/}
token=''
if [[ "$registry" == ghcr.io ]]; then
  token=$(curl -fsS "https://ghcr.io/token?service=ghcr.io&scope=repository:$repo:pull" 2>/dev/null | sed -n 's/.*"token":"\([^"]*\)".*/\1/p') || true
fi
auth=()
[[ -n "$token" ]] && auth=(-H "Authorization: Bearer $token")
if [[ -z "$tag" ]]; then
  tag=$(curl -fsS "${auth[@]}" "https://$registry/v2/$repo/tags/list" 2>/dev/null | grep -o '"[0-9]\+\.[0-9]\+\.[0-9]\+"' | tr -d '"' | sort -V | tail -1) || true
  [[ -n "$tag" ]] || die "kein Tag x.y.z von $image gefunden: ist der Workflow \"Umbrel image\" gelaufen, und ist das Paket öffentlich? (oder gib --tag an)"
fi
[[ "$tag" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "--tag muss wie 0.1.42 aussehen (es ist auch die Version der App)"
digest=$(curl -fsSI "${auth[@]}" \
  -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
  "https://$registry/v2/$repo/manifests/$tag" 2>/dev/null | tr -d '\r' | sed -n 's/^docker-content-digest:[[:space:]]*//Ip' | head -1) || true
if [[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  ref="$image:$tag@$digest"
else
  echo "publish.sh: Der Digest von $image:$tag ließ sich nicht lesen, darum ist nur das Tag festgenagelt" >&2
  ref="$image:$tag"
fi

# Der Ordner der App, mit der ID des Stores, dem Image, seiner Version und dem Port.
dest="$store/$app_id"
rm -rf "$dest"
mkdir -p "$dest"
sed -e "s/^id: open-agentic-office$/id: $app_id/" \
  -e "s/^version: .*/version: \"$tag\"/" \
  -e "s/^port: .*/port: $port/" \
  "$here/open-agentic-office/umbrel-app.yml" >"$dest/umbrel-app.yml"
sed -e "s|APP_HOST: open-agentic-office_web_1|APP_HOST: ${app_id}_web_1|" \
  -e "s|image: ghcr.io/luna-os/open-agentic-office:latest|image: $ref|" \
  "$here/open-agentic-office/docker-compose.yml" >"$dest/docker-compose.yml"
grep -q "^id: $app_id$" "$dest/umbrel-app.yml" || die "die ID der App ließ sich nicht setzen in $dest/umbrel-app.yml"
grep -q "image: $ref" "$dest/docker-compose.yml" || die "das Image ließ sich nicht setzen in $dest/docker-compose.yml"
echo "==> $dest: OpenAgenticOffice $tag ($ref) on port $port"

if ((push)); then
  base=$(git -C "$store" rev-parse --abbrev-ref HEAD)
  branch="open-agentic-office-$tag"
  ((pr)) && git -C "$store" checkout -q -B "$branch"
  git -C "$store" add umbrel-app-store.yml "$app_id"
  if git -C "$store" diff --cached --quiet; then
    echo "==> nichts Neues zu committen: der Store hat diese Version schon"
  else
    git -C "$store" commit -m "OpenAgenticOffice $tag"
  fi
  if ((pr)); then
    git -C "$store" push -u origin "$branch"
    git -C "$store" checkout -q "$base"
    # Die Seite des Stores: sein Remote ohne Zugangsdaten oder .git (Host und Pfad eines ssh-Remotes, über https).
    web=$(git -C "$store" remote get-url origin | sed -E 's#^(https?://)[^/@]+@#\1#; s#^[^/@]+@([^:/]+):#https://\1/#; s#\.git$##')
    echo "==> $branch hochgeladen. Öffne den Pull Request: $web/compare/$base...$branch"
    echo "    Ist er gemergt: auf dem Umbrel App Store → ⋯ → Community App Stores → URL des Stores hinzufügen (einmalig), dann OpenAgenticOffice installieren oder aktualisieren."
  else
    git -C "$store" push
    echo "==> hochgeladen. Auf dem Umbrel: App Store → ⋯ → Community App Stores → URL des Stores hinzufügen (einmalig), dann OpenAgenticOffice installieren oder aktualisieren."
  fi
else
  echo "==> als Nächstes: git -C '$store' add -A && git -C '$store' commit -m 'OpenAgenticOffice $tag' && git -C '$store' push"
fi
