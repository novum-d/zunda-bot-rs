#!/usr/bin/env bash
# セーブデータとサーバー設定を圧縮し、GCSへアップロードする。
set -euo pipefail

# バックアップ元は専用データディスク上の共有ディレクトリに固定する。
DATA=/srv/seven-days-data
BUCKET_FILE=${BACKUP_BUCKET_FILE:-/etc/seven-days-backup-bucket}
if [ ! -s "$BUCKET_FILE" ]; then
  printf 'バックアップ先バケット名が見つかりません: %s\n' "$BUCKET_FILE" >&2
  exit 1
fi
BUCKET=$(tr -d '\r\n' <"$BUCKET_FILE")

# 一時ファイルはデータディスク上に作り、アップロード成功後に必ず削除する。
TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
ARCHIVE_DIR="$DATA/.backup-upload"
ARCHIVE="$ARCHIVE_DIR/$TIMESTAMP.tar.gz"
OBJECT="backups/$TIMESTAMP.tar.gz"
mkdir -p "$ARCHIVE_DIR"
trap 'rm -f "$ARCHIVE"; rmdir "$ARCHIVE_DIR" 2>/dev/null || true' EXIT

# ワールドデータと設定は必須対象として圧縮する。
BACKUP_PATHS=(Saves GeneratedWorlds serverconfig.xml)
# Mods は未使用の環境もあるため、存在する場合だけ含める。
if [ -d "$DATA/Mods" ]; then BACKUP_PATHS+=(Mods); fi
tar -C "$DATA" -czf "$ARCHIVE" "${BACKUP_PATHS[@]}"

# Compute Engine のサービスアカウントから短期アクセストークンを取得する。
TOKEN_JSON=$(curl -fsS -H 'Metadata-Flavor: Google' \
  'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token')
ACCESS_TOKEN=$(printf '%s' "$TOKEN_JSON" | sed -n 's/.*"access_token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
if [ -z "$ACCESS_TOKEN" ]; then
  printf 'Compute Engine のアクセストークンを取得できませんでした\n' >&2
  exit 1
fi

# Resumable upload を開始し、返されたURLへ圧縮ファイルを送信する。
UPLOAD_URL=$(curl -fsS -D - -o /dev/null -X POST \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H 'Content-Type: application/json; charset=UTF-8' \
  -H 'X-Upload-Content-Type: application/gzip' \
  "https://storage.googleapis.com/upload/storage/v1/b/$BUCKET/o?uploadType=resumable&name=$OBJECT" \
  | awk 'tolower($1) == "location:" { sub(/\r$/, "", $2); print $2 }')
if [ -z "$UPLOAD_URL" ]; then
  printf 'GCSのアップロードURLを取得できませんでした\n' >&2
  exit 1
fi

curl -fsS -X PUT \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H 'Content-Type: application/gzip' \
  --upload-file "$ARCHIVE" "$UPLOAD_URL" >/dev/null

printf 'Backup uploaded: gs://%s/%s\n' "$BUCKET" "$OBJECT"
