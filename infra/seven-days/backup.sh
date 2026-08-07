#!/usr/bin/env bash
# セーブデータとサーバー設定を時刻付きディレクトリへ退避する。
set -euo pipefail

# バックアップ元は専用データディスク上の共有ディレクトリに固定する。
DATA=/srv/seven-days-data
# UTC時刻を使い、実行環境のタイムゾーンに左右されない名前にする。
DEST="$DATA/backups/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$DEST"

# ワールドデータと設定は必須対象としてコピーする。
cp -a "$DATA/Saves" "$DATA/GeneratedWorlds" "$DATA/serverconfig.xml" "$DEST/"
# Mods は未使用の環境もあるため、存在する場合だけバックアップする。
if [ -d "$DATA/Mods" ]; then cp -a "$DATA/Mods" "$DEST/"; fi

# 運用担当が作成先を確認できるように標準出力へ表示する。
printf 'Backup created: %s\n' "$DEST"
