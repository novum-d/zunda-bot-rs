#!/usr/bin/env bash
set -euo pipefail
DATA=/srv/seven-days-data
DEST="$DATA/backups/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$DEST"
cp -a "$DATA/Saves" "$DATA/GeneratedWorlds" "$DATA/serverconfig.xml" "$DEST/"
if [ -d "$DATA/Mods" ]; then cp -a "$DATA/Mods" "$DEST/"; fi
printf 'Backup created: %s\n' "$DEST"
