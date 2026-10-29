#!/usr/bin/env bash
# Puts the Umbrel app Agent Office Vermittler (deploy/umbrel/agent-office-vermittler, the server the
# office runs on, see docs/vermittler.md) into an Umbrel community app store: a git repository with
# an umbrel-app-store.yml at its root, like http://100.78.107.162:8101/nicorenezweers/nrzStore.
#
#   git clone http://100.78.107.162:8101/nicorenezweers/nrzStore
#   deploy/umbrel/publish.sh nrzStore --pr      # a branch of its own, to merge as a pull request
#   deploy/umbrel/publish.sh nrzStore --push    # or straight onto the branch you're on
#
# The app lands in <store>/<store id>-agent-office-vermittler/ (Umbrel wants every app's id, and its
# folder, to start with the store's id), on the newest image the "Umbrel image" workflow pushed to
# ghcr.io/luna-os/agent-office, pinned by its digest, with its version as the app's version, so
# Umbrel offers an update each time you publish a newer one. Run it again to update.
#
# Options:
#   --push            git add, commit and push the store afterwards
#   --pr              the same, on a new branch agent-office-vermittler-<version>, and print the link
#                     that opens the pull request for it (Gitea and GitHub both take /compare/<base>...<branch>)
#   --tag <version>   a particular image (default: the newest x.y.z tag)
#   --image <repo>    another image repository (default: ghcr.io/luna-os/agent-office)
#   --store-id <id>   the store's id, for a store without an umbrel-app-store.yml yet (one is made)
#   --port <port>     the port the Umbrel serves the office on (default 4600)
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
app=agent-office-vermittler
image=''
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
    --tag) tag=${2:?--tag needs a version}; shift ;;
    --image) image=${2:?--image needs a repository}; shift ;;
    --store-id) store_id=${2:?--store-id needs an id}; shift ;;
    --port) port=${2:?--port needs a number}; shift ;;
    -h | --help) usage ;;
    -*) die "unknown option $1 (see --help)" ;;
    *) [[ -z "$store" ]] || die "one store at a time"; store=$1 ;;
  esac
  shift
done
[[ -n "$store" ]] || usage 1
[[ -d "$store" ]] || die "$store isn't a folder: clone your app store first (git clone <its url>)"
[[ "$port" =~ ^[0-9]+$ ]] || die "--port must be a number"
[[ -n "$image" ]] || image=ghcr.io/luna-os/agent-office
name=$(sed -n 's/^name:[[:space:]]*//p' "$here/$app/umbrel-app.yml" | head -1)

# The store's id: from its umbrel-app-store.yml, or made with --store-id.
manifest="$store/umbrel-app-store.yml"
if [[ -f "$manifest" ]]; then
  found=$(sed -n 's/^id:[[:space:]]*["'\'']\{0,1\}\([A-Za-z0-9-]*\).*/\1/p' "$manifest" | head -1)
  [[ -n "$found" ]] || die "$manifest has no id: line"
  [[ -z "$store_id" || "$store_id" == "$found" ]] || die "$manifest says the store's id is $found, not $store_id"
  store_id=$found
else
  [[ -n "$store_id" ]] || die "$store has no umbrel-app-store.yml: pass --store-id <id> to make one"
  printf 'id: "%s"\nname: "%s"\n' "$store_id" "$(basename "$(cd "$store" && pwd)")" >"$manifest"
  echo "==> made $manifest (id: $store_id)"
fi
[[ "$store_id" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "the store's id ($store_id) must be lowercase letters, digits and dashes"
app_id="$store_id-$app"

# The image: the newest x.y.z tag on the registry (or --tag), and its digest, read anonymously.
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
  [[ -n "$tag" ]] || die "found no x.y.z tag of $image: has the \"Umbrel image\" workflow run, and is the package public? (or pass --tag)"
fi
[[ "$tag" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "--tag must look like 0.1.42 (it's the app's version too)"
digest=$(curl -fsSI "${auth[@]}" \
  -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
  "https://$registry/v2/$repo/manifests/$tag" 2>/dev/null | tr -d '\r' | sed -n 's/^docker-content-digest:[[:space:]]*//Ip' | head -1) || true
if [[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  ref="$image:$tag@$digest"
else
  echo "publish.sh: couldn't read the digest of $image:$tag, so it's pinned by its tag only" >&2
  ref="$image:$tag"
fi

# The app's folder, with the store's id, the image, its version and the port.
dest="$store/$app_id"
rm -rf "$dest"
mkdir -p "$dest"
sed -e "s/^id: $app$/id: $app_id/" \
  -e "s/^version: .*/version: \"$tag\"/" \
  -e "s/^port: .*/port: $port/" \
  "$here/$app/umbrel-app.yml" >"$dest/umbrel-app.yml"
sed -e "s|APP_HOST: ${app}_web_1|APP_HOST: ${app_id}_web_1|" \
  -e "s|image: ghcr.io/luna-os/agent-office:latest|image: $ref|" \
  "$here/$app/docker-compose.yml" >"$dest/docker-compose.yml"
grep -q "^id: $app_id$" "$dest/umbrel-app.yml" || die "couldn't set the app's id in $dest/umbrel-app.yml"
grep -q "image: $ref" "$dest/docker-compose.yml" || die "couldn't set the image in $dest/docker-compose.yml"
echo "==> $dest: $name $tag ($ref) on port $port"

if ((push)); then
  base=$(git -C "$store" rev-parse --abbrev-ref HEAD)
  branch="$app-$tag"
  ((pr)) && git -C "$store" checkout -q -B "$branch"
  git -C "$store" add umbrel-app-store.yml "$app_id"
  if git -C "$store" diff --cached --quiet; then
    echo "==> nothing new to commit: the store has this version already"
  else
    git -C "$store" commit -m "$name $tag"
  fi
  if ((pr)); then
    git -C "$store" push -u origin "$branch"
    git -C "$store" checkout -q "$base"
    # The store's page: its remote without credentials or .git (an ssh remote's host and path, over https).
    web=$(git -C "$store" remote get-url origin | sed -E 's#^(https?://)[^/@]+@#\1#; s#^[^/@]+@([^:/]+):#https://\1/#; s#\.git$##')
    echo "==> pushed $branch. Open the pull request: $web/compare/$base...$branch"
    echo "    Once it's merged: on the Umbrel, App Store → ⋯ → Community App Stores → add the store's URL (once), then install or update $name."
  else
    git -C "$store" push
    echo "==> pushed. On the Umbrel: App Store → ⋯ → Community App Stores → add the store's URL (once), then install or update $name."
  fi
else
  echo "==> next: git -C '$store' add -A && git -C '$store' commit -m '$name $tag' && git -C '$store' push"
fi
