#!/usr/bin/env bash
# Prints the version of the next app release: desktop/package.json's major.minor plus one more than the
# highest patch already tagged for it (v…, and the old windows-v… / mac-v… tags), so versions only ever
# go up by one: 0.1.13, 0.1.14, ... Bump desktop/package.json's minor to start over at <minor>.1.
set -euo pipefail
cd "$(dirname "$0")/../.."

base="$(node -p 'require("./desktop/package.json").version.split(".").slice(0, 2).join(".")')"
last="$(git tag -l | sed -nE "s/^(v|windows-v|mac-v)${base//./\\.}\.([0-9]+)$/\2/p" | sort -n | tail -n 1)"
echo "$base.$(( ${last:-0} + 1 ))"
