#!/usr/bin/env bash
# Prints the version of the next app release, one tenth above the last: 1.0, 1.1, ... 1.9, 2.0, ...
# The first release (no v<major>.<minor> tag yet) is 1.0. The apps themselves get <version>.0.
set -euo pipefail
cd "$(dirname "$0")/../.."

last="$(git tag -l 'v*' | sed -nE 's/^v([0-9]+)\.([0-9])$/\1 \2/p' | awk '{ print $1 * 10 + $2 }' | sort -n | tail -n 1)"
next=$(( ${last:-9} + 1 ))
echo "$(( next / 10 )).$(( next % 10 ))"
