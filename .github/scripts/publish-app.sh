#!/usr/bin/env bash
# Publishes one GitHub release with the Windows installer and both Mac disk images of a version:
# tag v<version>, title "Agent Office <version>", a download table and the generated changelog.
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

notes="$(cat <<NOTES
## Download

| System | Datei |
| --- | --- |
| Windows | [Agent-Office-Setup-$version.exe]($downloads/Agent-Office-Setup-$version.exe) |
| macOS mit Apple-Chip (M1, M2, M3, M4 …) | [Agent-Office-$version-mac-arm64.dmg]($downloads/Agent-Office-$version-mac-arm64.dmg) |
| macOS mit Intel-Prozessor | [Agent-Office-$version-mac-x64.dmg]($downloads/Agent-Office-$version-mac-x64.dmg) |

Installieren: [Windows](https://github.com/$GITHUB_REPOSITORY/blob/main/docs/windows-app.md#installieren) · [Mac](https://github.com/$GITHUB_REPOSITORY/blob/main/docs/mac-app.md#installieren)
NOTES
)"

gh release create "$tag" "$@" --target "$GITHUB_SHA" --title "Agent Office $version" \
  --notes "$notes" --generate-notes --latest
