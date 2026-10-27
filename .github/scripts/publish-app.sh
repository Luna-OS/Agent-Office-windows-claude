#!/usr/bin/env bash
# Puts a desktop app's installers into the one GitHub release of this version (v0.1.<commits on main>),
# "Agent Office 0.1.<n>", which windows-app.yml and mac-app.yml share: whichever finishes first creates
# it, with a download table and the generated changelog, and the other adds its files to it.
#
#   publish-app.sh <version> <file>...      (needs GH_TOKEN and GITHUB_SHA)
set -euo pipefail

version="$1"
shift
tag="v$version"
downloads="https://github.com/$GITHUB_REPOSITORY/releases/download/$tag"

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

# The previous release of this kind, so the changelog only lists what's new since then.
previous="$(gh release list --limit 100 --json tagName --jq '.[].tagName' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | grep -vx "$tag" | sort -V | tail -n 1 || true)"

if ! gh release view "$tag" > /dev/null 2>&1; then
  if gh release create "$tag" "$@" --target "$GITHUB_SHA" --title "Agent Office $version" \
    --notes "$notes" --generate-notes ${previous:+--notes-start-tag "$previous"} --latest; then
    exit 0
  fi
  # The other app's workflow may have created it in the meantime.
  gh release view "$tag" > /dev/null
fi
gh release upload "$tag" "$@" --clobber
