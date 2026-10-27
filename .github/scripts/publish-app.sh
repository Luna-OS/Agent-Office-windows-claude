#!/usr/bin/env bash
# Publishes one GitHub release with the Windows installer and both Mac disk images of a version:
# tag v<version> (1.0, 1.1, ...), title "Agent Office <version>", a download table and the generated
# changelog.
#
#   publish-app.sh <version> <file>...      (needs GH_TOKEN, GITHUB_SHA and GITHUB_REPOSITORY)
set -euo pipefail

version="$1"
shift
tag="v$version"
downloads="https://github.com/$GITHUB_REPOSITORY/releases/download/$tag"

if gh release view "$tag" > /dev/null 2>&1; then
  echo "$tag is already released"
  exit 0
fi

# One row per installer, named after the files themselves, so a renamed app keeps working links.
rows=""
for file in "$@"; do
  name="$(basename "$file")"
  case "$name" in
    *.exe) system="Windows" ;;
    *arm64.dmg) system="macOS mit Apple-Chip (M1, M2, M3, M4 …)" ;;
    *.dmg) system="macOS mit Intel-Prozessor" ;;
    *) continue ;;
  esac
  rows+="| $system | [$name]($downloads/$name) |"$'\n'
done

notes="$(cat <<NOTES
## Download

| System | Datei |
| --- | --- |
$rows
Installieren: [Windows](https://github.com/$GITHUB_REPOSITORY/blob/main/docs/windows-app.md#installieren) · [Mac](https://github.com/$GITHUB_REPOSITORY/blob/main/docs/mac-app.md#installieren)
NOTES
)"

gh release create "$tag" "$@" --target "$GITHUB_SHA" --title "Agent Office $version" \
  --notes "$notes" --generate-notes --latest
